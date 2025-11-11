-- Migration: 003_payment_channels
-- Description: Payment channel system for instant micropayments
-- Date: 2025-11-11

-- ============================================================================
-- PAYMENT CHANNELS TABLE
-- ============================================================================

CREATE TABLE IF NOT EXISTS payment_channels (
    -- Identity
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    channel_id VARCHAR(66) UNIQUE NOT NULL,
    
    -- Parties
    party_a_paymail VARCHAR(255) NOT NULL,
    party_b_paymail VARCHAR(255) NOT NULL,
    
    -- Balances (in satoshis)
    initial_balance_a BIGINT NOT NULL CHECK (initial_balance_a >= 0),
    initial_balance_b BIGINT NOT NULL CHECK (initial_balance_b >= 0),
    current_balance_a BIGINT NOT NULL CHECK (current_balance_a >= 0),
    current_balance_b BIGINT NOT NULL CHECK (current_balance_b >= 0),
    
    -- State
    status VARCHAR(20) NOT NULL DEFAULT 'Open',
        -- Valid values: 'Open', 'Active', 'Closing', 'Closed', 'Disputed'
    sequence_number BIGINT NOT NULL DEFAULT 0 CHECK (sequence_number >= 0),
    
    -- Timing
    opened_at TIMESTAMP NOT NULL DEFAULT NOW(),
    closed_at TIMESTAMP,
    last_payment_at TIMESTAMP,
    
    -- Settlement
    settlement_txid VARCHAR(64),
    timeout_blocks INT DEFAULT 144 CHECK (timeout_blocks > 0),
    
    -- Metadata
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    
    -- Constraints
    CONSTRAINT balance_conservation CHECK (
        current_balance_a + current_balance_b = 
        initial_balance_a + initial_balance_b
    ),
    CONSTRAINT different_parties CHECK (party_a_paymail != party_b_paymail),
    CONSTRAINT valid_status CHECK (
        status IN ('Open', 'Active', 'Closing', 'Closed', 'Disputed')
    )
);

COMMENT ON TABLE payment_channels IS 'Payment channels for instant off-chain micropayments';
COMMENT ON COLUMN payment_channels.channel_id IS 'Unique identifier for the channel (hash of parties + timestamp)';
COMMENT ON COLUMN payment_channels.sequence_number IS 'Monotonically increasing counter for state updates';
COMMENT ON COLUMN payment_channels.timeout_blocks IS 'Number of blocks before forced settlement (default ~24 hours)';

-- ============================================================================
-- CHANNEL STATES TABLE (Audit Trail)
-- ============================================================================

CREATE TABLE IF NOT EXISTS channel_states (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    channel_id VARCHAR(66) NOT NULL REFERENCES payment_channels(channel_id) ON DELETE CASCADE,
    
    -- State snapshot
    sequence_number BIGINT NOT NULL CHECK (sequence_number >= 0),
    balance_a BIGINT NOT NULL CHECK (balance_a >= 0),
    balance_b BIGINT NOT NULL CHECK (balance_b >= 0),
    
    -- Cryptographic proof (for Phase 5 blockchain integration)
    state_hash VARCHAR(64),
    signature_a TEXT,
    signature_b TEXT,
    
    -- Metadata
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    
    -- Ensure unique sequence per channel
    UNIQUE(channel_id, sequence_number)
);

COMMENT ON TABLE channel_states IS 'Audit trail of all channel state changes';
COMMENT ON COLUMN channel_states.sequence_number IS 'State version number, must be unique per channel';

-- ============================================================================
-- CHANNEL PAYMENTS TABLE
-- ============================================================================

CREATE TABLE IF NOT EXISTS channel_payments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    channel_id VARCHAR(66) NOT NULL REFERENCES payment_channels(channel_id) ON DELETE CASCADE,
    
    -- Payment details
    from_paymail VARCHAR(255) NOT NULL,
    to_paymail VARCHAR(255) NOT NULL,
    amount_satoshis BIGINT NOT NULL CHECK (amount_satoshis > 0),
    
    -- Context
    sequence_number BIGINT NOT NULL CHECK (sequence_number > 0),
    memo TEXT,
    
    -- State after this payment
    balance_a_after BIGINT NOT NULL CHECK (balance_a_after >= 0),
    balance_b_after BIGINT NOT NULL CHECK (balance_b_after >= 0),
    
    -- Performance tracking
    processing_time_ms INT,
    
    -- Metadata
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    
    CONSTRAINT different_payment_parties CHECK (from_paymail != to_paymail)
);

COMMENT ON TABLE channel_payments IS 'Individual payment records through payment channels';
COMMENT ON COLUMN channel_payments.sequence_number IS 'Links to channel state sequence number';
COMMENT ON COLUMN channel_payments.processing_time_ms IS 'Payment processing latency in milliseconds';

-- ============================================================================
-- INDEXES FOR PERFORMANCE
-- ============================================================================

-- Channel lookups
CREATE INDEX IF NOT EXISTS idx_channels_party_a ON payment_channels(party_a_paymail);
CREATE INDEX IF NOT EXISTS idx_channels_party_b ON payment_channels(party_b_paymail);
CREATE INDEX IF NOT EXISTS idx_channels_status ON payment_channels(status);
CREATE INDEX IF NOT EXISTS idx_channels_opened_at ON payment_channels(opened_at DESC);
CREATE INDEX IF NOT EXISTS idx_channels_updated_at ON payment_channels(updated_at DESC);

-- State lookups
CREATE INDEX IF NOT EXISTS idx_channel_states_channel ON channel_states(channel_id);
CREATE INDEX IF NOT EXISTS idx_channel_states_sequence ON channel_states(channel_id, sequence_number DESC);

-- Payment lookups
CREATE INDEX IF NOT EXISTS idx_channel_payments_channel ON channel_payments(channel_id);
CREATE INDEX IF NOT EXISTS idx_channel_payments_created ON channel_payments(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_channel_payments_from ON channel_payments(from_paymail);
CREATE INDEX IF NOT EXISTS idx_channel_payments_to ON channel_payments(to_paymail);

-- ============================================================================
-- DATABASE FUNCTION: Process Payment Atomically
-- ============================================================================

CREATE OR REPLACE FUNCTION process_channel_payment(
    p_channel_id VARCHAR(66),
    p_from_paymail VARCHAR(255),
    p_to_paymail VARCHAR(255),
    p_amount BIGINT,
    p_memo TEXT DEFAULT NULL
) RETURNS JSON AS $$
DECLARE
    v_channel payment_channels;
    v_new_balance_a BIGINT;
    v_new_balance_b BIGINT;
    v_new_sequence BIGINT;
    v_payment_id UUID;
BEGIN
    -- Lock the channel row for update
    SELECT * INTO v_channel
    FROM payment_channels
    WHERE channel_id = p_channel_id
    FOR UPDATE;
    
    -- Validate channel exists
    IF NOT FOUND THEN
        RAISE EXCEPTION 'Channel not found';
    END IF;
    
    -- Validate channel status
    IF v_channel.status NOT IN ('Open', 'Active') THEN
        RAISE EXCEPTION 'Channel is not active (status: %)', v_channel.status;
    END IF;
    
    -- Validate parties
    IF p_from_paymail NOT IN (v_channel.party_a_paymail, v_channel.party_b_paymail) THEN
        RAISE EXCEPTION 'Sender is not a party to this channel';
    END IF;
    
    IF p_to_paymail NOT IN (v_channel.party_a_paymail, v_channel.party_b_paymail) THEN
        RAISE EXCEPTION 'Recipient is not a party to this channel';
    END IF;
    
    IF p_from_paymail = p_to_paymail THEN
        RAISE EXCEPTION 'Cannot pay yourself';
    END IF;
    
    -- Validate amount
    IF p_amount <= 0 THEN
        RAISE EXCEPTION 'Amount must be positive';
    END IF;
    
    -- Calculate new balances
    IF p_from_paymail = v_channel.party_a_paymail THEN
        v_new_balance_a := v_channel.current_balance_a - p_amount;
        v_new_balance_b := v_channel.current_balance_b + p_amount;
    ELSE
        v_new_balance_a := v_channel.current_balance_a + p_amount;
        v_new_balance_b := v_channel.current_balance_b - p_amount;
    END IF;
    
    -- Validate balances
    IF v_new_balance_a < 0 THEN
        RAISE EXCEPTION 'Insufficient balance (needed: %, available: %)', 
            p_amount, v_channel.current_balance_a;
    END IF;
    
    IF v_new_balance_b < 0 THEN
        RAISE EXCEPTION 'Insufficient balance';
    END IF;
    
    -- Increment sequence
    v_new_sequence := v_channel.sequence_number + 1;
    
    -- Update channel
    UPDATE payment_channels
    SET current_balance_a = v_new_balance_a,
        current_balance_b = v_new_balance_b,
        sequence_number = v_new_sequence,
        last_payment_at = NOW(),
        updated_at = NOW(),
        status = 'Active'
    WHERE channel_id = p_channel_id;
    
    -- Record payment
    INSERT INTO channel_payments (
        channel_id,
        from_paymail,
        to_paymail,
        amount_satoshis,
        sequence_number,
        memo,
        balance_a_after,
        balance_b_after
    ) VALUES (
        p_channel_id,
        p_from_paymail,
        p_to_paymail,
        p_amount,
        v_new_sequence,
        p_memo,
        v_new_balance_a,
        v_new_balance_b
    ) RETURNING id INTO v_payment_id;
    
    -- Record state snapshot
    INSERT INTO channel_states (
        channel_id,
        sequence_number,
        balance_a,
        balance_b
    ) VALUES (
        p_channel_id,
        v_new_sequence,
        v_new_balance_a,
        v_new_balance_b
    );
    
    -- Return result
    RETURN json_build_object(
        'payment_id', v_payment_id,
        'sequence_number', v_new_sequence,
        'balance_a', v_new_balance_a,
        'balance_b', v_new_balance_b,
        'success', true
    );
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION process_channel_payment IS 'Atomically process a payment through a channel';

-- ============================================================================
-- DATABASE FUNCTION: Update Timestamp Trigger
-- ============================================================================

CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger for payment_channels
CREATE TRIGGER update_payment_channels_updated_at
    BEFORE UPDATE ON payment_channels
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- ============================================================================
-- GRANT PERMISSIONS (adjust user as needed)
-- ============================================================================

-- GRANT SELECT, INSERT, UPDATE ON payment_channels TO bsv_bank_user;
-- GRANT SELECT, INSERT ON channel_states TO bsv_bank_user;
-- GRANT SELECT, INSERT ON channel_payments TO bsv_bank_user;

-- ============================================================================
-- MIGRATION COMPLETE
-- ============================================================================

-- Verify tables exist
DO $$
BEGIN
    IF NOT EXISTS (SELECT FROM pg_tables WHERE tablename = 'payment_channels') THEN
        RAISE EXCEPTION 'Migration failed: payment_channels table not created';
    END IF;
    
    IF NOT EXISTS (SELECT FROM pg_tables WHERE tablename = 'channel_states') THEN
        RAISE EXCEPTION 'Migration failed: channel_states table not created';
    END IF;
    
    IF NOT EXISTS (SELECT FROM pg_tables WHERE tablename = 'channel_payments') THEN
        RAISE EXCEPTION 'Migration failed: channel_payments table not created';
    END IF;
    
    RAISE NOTICE 'Migration 003_payment_channels completed successfully';
END $$;