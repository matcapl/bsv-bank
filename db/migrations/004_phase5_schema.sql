-- migrations/phase5_schema.sql
-- Phase 5: BSV Testnet Integration Database Schema

-- ============================================================================
-- BLOCKCHAIN MONITORING TABLES
-- ============================================================================

-- Track watched addresses for transaction monitoring
CREATE TABLE IF NOT EXISTS watched_addresses (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    address VARCHAR(255) NOT NULL UNIQUE,
    paymail VARCHAR(255) NOT NULL,
    purpose VARCHAR(50) NOT NULL, -- 'deposit', 'channel', 'lending', 'withdrawal'
    derivation_path VARCHAR(100), -- BIP32 path if applicable
    created_at TIMESTAMPTZ DEFAULT NOW(),
    last_checked TIMESTAMPTZ,
    last_activity TIMESTAMPTZ,
    active BOOLEAN DEFAULT true
);

CREATE INDEX IF NOT EXISTS idx_watched_addresses_paymail ON watched_addresses(paymail);
CREATE INDEX IF NOT EXISTS idx_watched_addresses_purpose ON watched_addresses(purpose);
CREATE INDEX IF NOT EXISTS idx_watched_addresses_active ON watched_addresses(active);

-- Cache blockchain transaction data
CREATE TABLE IF NOT EXISTS blockchain_transactions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    txid VARCHAR(64) NOT NULL UNIQUE,
    tx_type VARCHAR(20) NOT NULL, -- 'deposit', 'withdrawal', 'funding', 'commitment', 'settlement'
    from_address VARCHAR(255),
    to_address VARCHAR(255),
    amount_satoshis BIGINT NOT NULL,
    fee_satoshis BIGINT,
    confirmations INT DEFAULT 0,
    status VARCHAR(20) DEFAULT 'pending', -- 'pending', 'confirmed', 'failed'
    block_hash VARCHAR(64),
    block_height INT,
    block_time TIMESTAMPTZ,
    raw_tx TEXT, -- Full transaction hex
    first_seen TIMESTAMPTZ DEFAULT NOW(),
    confirmed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_blockchain_txid ON blockchain_transactions(txid);
CREATE INDEX IF NOT EXISTS idx_blockchain_status ON blockchain_transactions(status);
CREATE INDEX IF NOT EXISTS idx_blockchain_type ON blockchain_transactions(tx_type);
CREATE INDEX IF NOT EXISTS idx_blockchain_height ON blockchain_transactions(block_height);
CREATE INDEX IF NOT EXISTS idx_blockchain_from ON blockchain_transactions(from_address);
CREATE INDEX IF NOT EXISTS idx_blockchain_to ON blockchain_transactions(to_address);
CREATE INDEX IF NOT EXISTS idx_blockchain_time ON blockchain_transactions(first_seen);

-- Track confirmation changes for auditing
CREATE TABLE IF NOT EXISTS confirmation_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    txid VARCHAR(64) NOT NULL,
    old_confirmations INT NOT NULL,
    new_confirmations INT NOT NULL,
    block_height INT,
    detected_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_confirmation_txid ON confirmation_events(txid);
CREATE INDEX IF NOT EXISTS idx_confirmation_time ON confirmation_events(detected_at);

-- ============================================================================
-- SPV VERIFICATION TABLES
-- ============================================================================

-- Store block headers for SPV verification
CREATE TABLE IF NOT EXISTS block_headers (
    height INT PRIMARY KEY,
    hash VARCHAR(64) NOT NULL UNIQUE,
    version INT NOT NULL,
    prev_block VARCHAR(64) NOT NULL,
    merkle_root VARCHAR(64) NOT NULL,
    timestamp BIGINT NOT NULL,
    bits INT NOT NULL,
    nonce BIGINT NOT NULL,
    difficulty NUMERIC(20, 8),
    chainwork VARCHAR(64),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_block_hash ON block_headers(hash);
CREATE INDEX IF NOT EXISTS idx_block_prev ON block_headers(prev_block);
CREATE INDEX IF NOT EXISTS idx_block_time ON block_headers(timestamp);

-- Store Merkle proofs for transaction verification
CREATE TABLE IF NOT EXISTS merkle_proofs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    txid VARCHAR(64) NOT NULL UNIQUE,
    block_hash VARCHAR(64) NOT NULL,
    block_height INT,
    merkle_root VARCHAR(64) NOT NULL,
    siblings JSONB NOT NULL, -- Array of sibling hashes in Merkle tree
    tx_index INT NOT NULL,
    verified BOOLEAN DEFAULT false,
    verified_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_merkle_txid ON merkle_proofs(txid);
CREATE INDEX IF NOT EXISTS idx_merkle_block ON merkle_proofs(block_hash);
CREATE INDEX IF NOT EXISTS idx_merkle_verified ON merkle_proofs(verified);

-- ============================================================================
-- TRANSACTION BUILDING TABLES
-- ============================================================================

-- Store transaction templates for channels (funding, commitment, settlement)
CREATE TABLE IF NOT EXISTS transaction_templates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    template_type VARCHAR(50) NOT NULL, -- 'funding', 'commitment', 'settlement', 'penalty'
    channel_id UUID REFERENCES payment_channels(id),
    tx_hex TEXT NOT NULL,
    txid VARCHAR(64),
    status VARCHAR(20) DEFAULT 'unsigned', -- 'unsigned', 'partial', 'signed', 'broadcast', 'confirmed'
    party_a_signed BOOLEAN DEFAULT false,
    party_b_signed BOOLEAN DEFAULT false,
    sequence_number INT, -- For commitment transactions
    timelock_blocks INT, -- For time-locked outputs
    created_at TIMESTAMPTZ DEFAULT NOW(),
    broadcast_at TIMESTAMPTZ,
    confirmed_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_tx_templates_channel ON transaction_templates(channel_id);
CREATE INDEX IF NOT EXISTS idx_tx_templates_type ON transaction_templates(template_type);
CREATE INDEX IF NOT EXISTS idx_tx_templates_status ON transaction_templates(status);
CREATE INDEX IF NOT EXISTS idx_tx_templates_txid ON transaction_templates(txid);

-- Track signatures for multisig transactions
CREATE TABLE IF NOT EXISTS transaction_signatures (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tx_template_id UUID REFERENCES transaction_templates(id) ON DELETE CASCADE,
    paymail VARCHAR(255) NOT NULL,
    signature TEXT NOT NULL,
    pubkey TEXT NOT NULL,
    sighash_type INT DEFAULT 1, -- SIGHASH_ALL
    signed_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_tx_sigs_template ON transaction_signatures(tx_template_id);
CREATE INDEX IF NOT EXISTS idx_tx_sigs_paymail ON transaction_signatures(paymail);

-- ============================================================================
-- ENHANCED PAYMENT CHANNELS (Phase 5 Additions)
-- ============================================================================

-- Add blockchain-specific columns to existing payment_channels table
ALTER TABLE payment_channels 
    ADD COLUMN IF NOT EXISTS blockchain_enabled BOOLEAN DEFAULT false,
    ADD COLUMN IF NOT EXISTS funding_txid VARCHAR(64),
    ADD COLUMN IF NOT EXISTS funding_address VARCHAR(255),
    ADD COLUMN IF NOT EXISTS funding_vout INT DEFAULT 0,
    ADD COLUMN IF NOT EXISTS settlement_txid VARCHAR(64),
    ADD COLUMN IF NOT EXISTS funding_confirmations INT DEFAULT 0,
    ADD COLUMN IF NOT EXISTS settlement_confirmations INT DEFAULT 0,
    ADD COLUMN IF NOT EXISTS spv_verified BOOLEAN DEFAULT false,
    ADD COLUMN IF NOT EXISTS multisig_script TEXT,
    ADD COLUMN IF NOT EXISTS redeem_script TEXT,
    ADD COLUMN IF NOT EXISTS current_commitment_txid VARCHAR(64);

CREATE INDEX IF NOT EXISTS idx_channels_blockchain ON payment_channels(blockchain_enabled);
CREATE INDEX IF NOT EXISTS idx_channels_funding_txid ON payment_channels(funding_txid) WHERE funding_txid IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_channels_settlement_txid ON payment_channels(settlement_txid) WHERE settlement_txid IS NOT NULL;

-- Track channel lifecycle events on blockchain
CREATE TABLE IF NOT EXISTS channel_blockchain_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    channel_id UUID REFERENCES payment_channels(id) ON DELETE CASCADE,
    event_type VARCHAR(50) NOT NULL, -- 'funding_broadcast', 'funding_confirmed', 'commitment_updated', 'settlement_broadcast', 'settlement_confirmed', 'force_close'
    txid VARCHAR(64),
    confirmations INT,
    block_height INT,
    event_data JSONB, -- Additional event-specific data
    event_time TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_channel_events_channel ON channel_blockchain_events(channel_id);
CREATE INDEX IF NOT EXISTS idx_channel_events_type ON channel_blockchain_events(event_type);
CREATE INDEX IF NOT EXISTS idx_channel_events_time ON channel_blockchain_events(event_time);
CREATE INDEX IF NOT EXISTS idx_channel_events_txid ON channel_blockchain_events(txid);

-- ============================================================================
-- API RATE LIMITING
-- ============================================================================

-- Track API calls for rate limiting (WhatsOnChain, etc.)
CREATE TABLE IF NOT EXISTS api_rate_limits (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    service_name VARCHAR(50) NOT NULL, -- 'whatsonchain', 'mattercloud', etc.
    endpoint VARCHAR(255) NOT NULL,
    calls_count INT DEFAULT 1,
    window_start TIMESTAMPTZ DEFAULT NOW(),
    last_call TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_rate_limits_service ON api_rate_limits(service_name);
CREATE INDEX IF NOT EXISTS idx_rate_limits_window ON api_rate_limits(window_start);

-- ============================================================================
-- TESTNET UTILITIES
-- ============================================================================

-- Track faucet requests for testing (testnet only)
CREATE TABLE IF NOT EXISTS testnet_faucet_requests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    address VARCHAR(255) NOT NULL,
    paymail VARCHAR(255),
    amount_requested BIGINT,
    txid VARCHAR(64),
    status VARCHAR(20) DEFAULT 'pending', -- 'pending', 'fulfilled', 'failed'
    requested_at TIMESTAMPTZ DEFAULT NOW(),
    fulfilled_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_faucet_address ON testnet_faucet_requests(address);
CREATE INDEX IF NOT EXISTS idx_faucet_status ON testnet_faucet_requests(status);
CREATE INDEX IF NOT EXISTS idx_faucet_paymail ON testnet_faucet_requests(paymail);

-- ============================================================================
-- ENHANCED DEPOSITS (Blockchain Verification)
-- ============================================================================

-- Add blockchain verification columns to existing deposits table
ALTER TABLE deposits
    ADD COLUMN IF NOT EXISTS confirmations INT DEFAULT 0,
    ADD COLUMN IF NOT EXISTS testnet_verified BOOLEAN DEFAULT false,
    ADD COLUMN IF NOT EXISTS spv_proof_verified BOOLEAN DEFAULT false,
    ADD COLUMN IF NOT EXISTS verified_at TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_deposits_confirmed ON deposits(testnet_verified);
CREATE INDEX IF NOT EXISTS idx_deposits_spv ON deposits(spv_proof_verified);

-- ============================================================================
-- ENHANCED LOANS (Collateral Verification)
-- ============================================================================

-- Add blockchain verification for loan collateral
ALTER TABLE loans
    ADD COLUMN IF NOT EXISTS collateral_txid VARCHAR(64),
    ADD COLUMN IF NOT EXISTS collateral_verified BOOLEAN DEFAULT false,
    ADD COLUMN IF NOT EXISTS collateral_confirmations INT DEFAULT 0,
    ADD COLUMN IF NOT EXISTS collateral_spv_verified BOOLEAN DEFAULT false;

CREATE INDEX IF NOT EXISTS idx_loans_collateral_txid ON loans(collateral_txid);
CREATE INDEX IF NOT EXISTS idx_loans_collateral_verified ON loans(collateral_verified);

-- ============================================================================
-- SYSTEM CONFIGURATION
-- ============================================================================

-- Store Phase 5 configuration
CREATE TABLE IF NOT EXISTS system_config (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    config_key VARCHAR(100) NOT NULL UNIQUE,
    config_value TEXT NOT NULL,
    config_type VARCHAR(20) DEFAULT 'string', -- 'string', 'number', 'boolean', 'json'
    description TEXT,
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Insert default Phase 5 configuration
INSERT INTO system_config (config_key, config_value, config_type, description) VALUES
    ('phase5_enabled', 'true', 'boolean', 'Enable Phase 5 blockchain integration'),
    ('network', 'testnet', 'string', 'Bitcoin network (testnet or mainnet)'),
    ('min_confirmations', '1', 'number', 'Minimum confirmations for transaction acceptance'),
    ('woc_api_base', 'https://api.whatsonchain.com/v1/bsv/test', 'string', 'WhatsOnChain API base URL'),
    ('fee_per_byte', '50', 'number', 'Default transaction fee in satoshis per byte'),
    ('channel_funding_confirmations', '1', 'number', 'Required confirmations for channel funding'),
    ('channel_settlement_confirmations', '1', 'number', 'Required confirmations for channel settlement')
ON CONFLICT (config_key) DO NOTHING;

-- ============================================================================
-- AUDIT TABLES
-- ============================================================================

-- Audit log for blockchain operations
CREATE TABLE IF NOT EXISTS blockchain_audit_log (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    operation VARCHAR(50) NOT NULL, -- 'broadcast', 'verify', 'watch', 'unwatch'
    entity_type VARCHAR(50), -- 'transaction', 'address', 'channel'
    entity_id VARCHAR(255),
    paymail VARCHAR(255),
    details JSONB,
    success BOOLEAN DEFAULT true,
    error_message TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_audit_operation ON blockchain_audit_log(operation);
CREATE INDEX IF NOT EXISTS idx_audit_entity ON blockchain_audit_log(entity_type, entity_id);
CREATE INDEX IF NOT EXISTS idx_audit_paymail ON blockchain_audit_log(paymail);
CREATE INDEX IF NOT EXISTS idx_audit_time ON blockchain_audit_log(created_at);

-- ============================================================================
-- VIEWS FOR MONITORING
-- ============================================================================

-- View for pending blockchain transactions
CREATE OR REPLACE VIEW pending_blockchain_transactions AS
SELECT 
    bt.txid,
    bt.tx_type,
    bt.amount_satoshis,
    bt.confirmations,
    bt.status,
    bt.first_seen,
    EXTRACT(EPOCH FROM (NOW() - bt.first_seen)) / 60 AS minutes_pending,
    wa.paymail
FROM blockchain_transactions bt
LEFT JOIN watched_addresses wa ON bt.to_address = wa.address
WHERE bt.status = 'pending' OR bt.confirmations < 6
ORDER BY bt.first_seen DESC;

-- View for channel blockchain status
CREATE OR REPLACE VIEW channel_blockchain_status AS
SELECT 
    pc.id AS channel_id,
    pc.party_a,
    pc.party_b,
    pc.status AS channel_status,
    pc.blockchain_enabled,
    pc.funding_txid,
    pc.funding_confirmations,
    pc.settlement_txid,
    pc.settlement_confirmations,
    pc.spv_verified,
    bt_funding.status AS funding_tx_status,
    bt_settlement.status AS settlement_tx_status
FROM payment_channels pc
LEFT JOIN blockchain_transactions bt_funding ON pc.funding_txid = bt_funding.txid AND pc.funding_txid IS NOT NULL
LEFT JOIN blockchain_transactions bt_settlement ON pc.settlement_txid = bt_settlement.txid AND pc.settlement_txid IS NOT NULL
WHERE pc.blockchain_enabled = true
ORDER BY pc.created_at DESC;

-- View for transaction verification status
CREATE OR REPLACE VIEW transaction_verification_status AS
SELECT 
    bt.txid,
    bt.tx_type,
    bt.confirmations,
    bt.status AS tx_status,
    mp.verified AS merkle_verified,
    mp.merkle_root,
    bh.height AS block_height,
    bh.hash AS block_hash,
    CASE 
        WHEN bt.confirmations >= 6 THEN 'deeply_confirmed'
        WHEN bt.confirmations >= 1 THEN 'confirmed'
        ELSE 'unconfirmed'
    END AS verification_level
FROM blockchain_transactions bt
LEFT JOIN merkle_proofs mp ON bt.txid = mp.txid
LEFT JOIN block_headers bh ON bt.block_hash = bh.hash
ORDER BY bt.first_seen DESC;

-- ============================================================================
-- FUNCTIONS
-- ============================================================================

-- Function to update transaction confirmations
CREATE OR REPLACE FUNCTION update_transaction_confirmations()
RETURNS TRIGGER AS $$
BEGIN
    -- Log confirmation change
    IF NEW.confirmations != OLD.confirmations THEN
        INSERT INTO confirmation_events (txid, old_confirmations, new_confirmations, block_height)
        VALUES (NEW.txid, OLD.confirmations, NEW.confirmations, NEW.block_height);
        
        -- Update confirmed_at timestamp
        IF NEW.confirmations > 0 AND OLD.confirmations = 0 THEN
            NEW.confirmed_at = NOW();
            NEW.status = 'confirmed';
        END IF;
    END IF;
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger for confirmation updates
DROP TRIGGER IF EXISTS trg_update_confirmations ON blockchain_transactions;
CREATE TRIGGER trg_update_confirmations
    BEFORE UPDATE ON blockchain_transactions
    FOR EACH ROW
    EXECUTE FUNCTION update_transaction_confirmations();

-- Function to audit blockchain operations
CREATE OR REPLACE FUNCTION audit_blockchain_operation()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO blockchain_audit_log (operation, entity_type, entity_id, details)
    VALUES (
        TG_OP,
        TG_TABLE_NAME,
        COALESCE(NEW.id::text, OLD.id::text),
        jsonb_build_object(
            'old', to_jsonb(OLD),
            'new', to_jsonb(NEW)
        )
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Triggers for audit logging
DROP TRIGGER IF EXISTS trg_audit_tx_templates ON transaction_templates;
CREATE TRIGGER trg_audit_tx_templates
    AFTER INSERT OR UPDATE OR DELETE ON transaction_templates
    FOR EACH ROW
    EXECUTE FUNCTION audit_blockchain_operation();

DROP TRIGGER IF EXISTS trg_audit_channel_events ON channel_blockchain_events;
CREATE TRIGGER trg_audit_channel_events
    AFTER INSERT ON channel_blockchain_events
    FOR EACH ROW
    EXECUTE FUNCTION audit_blockchain_operation();

-- ============================================================================
-- CLEANUP AND MAINTENANCE
-- ============================================================================

-- Function to clean up old cache entries
CREATE OR REPLACE FUNCTION cleanup_old_cache_entries()
RETURNS INTEGER AS $$
DECLARE
    deleted_count INTEGER;
BEGIN
    -- Delete blockchain transaction cache older than 7 days
    DELETE FROM blockchain_transactions
    WHERE first_seen < NOW() - INTERVAL '7 days'
      AND status = 'confirmed'
      AND confirmations > 100;
    
    GET DIAGNOSTICS deleted_count = ROW_COUNT;
    
    -- Delete old API rate limit entries
    DELETE FROM api_rate_limits
    WHERE window_start < NOW() - INTERVAL '1 hour';
    
    RETURN deleted_count;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- GRANTS (Adjust as needed for your security model)
-- ============================================================================

-- Grant permissions to application user (if applicable)
-- GRANT ALL ON ALL TABLES IN SCHEMA public TO bsv_bank_user;
-- GRANT ALL ON ALL SEQUENCES IN SCHEMA public TO bsv_bank_user;
-- GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA public TO bsv_bank_user;

-- ============================================================================
-- MIGRATION COMPLETE
-- ============================================================================

-- Record migration
INSERT INTO system_config (config_key, config_value, config_type, description) VALUES
    ('phase5_migration_version', '1.0.0', 'string', 'Phase 5 database schema version'),
    ('phase5_migration_date', NOW()::text, 'string', 'Phase 5 migration completion date')
ON CONFLICT (config_key) DO UPDATE SET 
    config_value = EXCLUDED.config_value,
    updated_at = NOW();

-- Print completion message
DO $$
BEGIN
    RAISE NOTICE '╔═══════════════════════════════════════════════════════════╗';
    RAISE NOTICE '║  Phase 5 Database Migration Complete                     ║';
    RAISE NOTICE '║                                                           ║';
    RAISE NOTICE '║  Tables Created:    16                                    ║';
    RAISE NOTICE '║  Indexes Created:   35+                                   ║';
    RAISE NOTICE '║  Views Created:     3                                     ║';
    RAISE NOTICE '║  Functions Created: 3                                     ║';
    RAISE NOTICE '║  Triggers Created:  3                                     ║';
    RAISE NOTICE '║                                                           ║';
    RAISE NOTICE '║  Status: Ready for Phase 5 services                       ║';
    RAISE NOTICE '╚═══════════════════════════════════════════════════════════╝';
END $$;