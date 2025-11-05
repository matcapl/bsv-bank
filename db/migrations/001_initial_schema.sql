-- Users table
CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    paymail VARCHAR(255) UNIQUE NOT NULL,
    public_key TEXT,
    kyc_status VARCHAR(50) DEFAULT 'pending',
    kyc_verified_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_users_paymail ON users(paymail);

-- Deposits table
CREATE TABLE deposits (
    id UUID PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    paymail VARCHAR(255) NOT NULL,
    amount_satoshis BIGINT NOT NULL CHECK (amount_satoshis > 0),
    txid VARCHAR(64) NOT NULL,
    block_height BIGINT,
    confirmations INTEGER DEFAULT 0,
    status VARCHAR(50) NOT NULL,
    lock_until TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    confirmed_at TIMESTAMPTZ,
    withdrawn_at TIMESTAMPTZ,
    CONSTRAINT unique_txid UNIQUE (txid)
);

CREATE INDEX idx_deposits_user_id ON deposits(user_id);
CREATE INDEX idx_deposits_paymail ON deposits(paymail);
CREATE INDEX idx_deposits_status ON deposits(status);
CREATE INDEX idx_deposits_txid ON deposits(txid);

-- Transactions table (audit trail)
CREATE TABLE transactions (
    id SERIAL PRIMARY KEY,
    deposit_id UUID REFERENCES deposits(id),
    txid VARCHAR(64) NOT NULL,
    type VARCHAR(50) NOT NULL,
    amount_satoshis BIGINT NOT NULL,
    from_address TEXT,
    to_address TEXT,
    status VARCHAR(50) NOT NULL,
    blockchain_confirmations INTEGER DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_transactions_deposit_id ON transactions(deposit_id);
CREATE INDEX idx_transactions_txid ON transactions(txid);
CREATE INDEX idx_transactions_type ON transactions(type);

-- Interest accruals table
CREATE TABLE interest_accruals (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    deposit_id UUID REFERENCES deposits(id),
    amount_satoshis BIGINT NOT NULL,
    rate_apy DECIMAL(10, 6) NOT NULL,
    period_start TIMESTAMPTZ NOT NULL,
    period_end TIMESTAMPTZ NOT NULL,
    paid_out BOOLEAN DEFAULT FALSE,
    paid_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_interest_accruals_user_id ON interest_accruals(user_id);
CREATE INDEX idx_interest_accruals_paid_out ON interest_accruals(paid_out);

-- Withdrawals table
CREATE TABLE withdrawals (
    id UUID PRIMARY KEY,
    deposit_id UUID NOT NULL REFERENCES deposits(id),
    user_id INTEGER NOT NULL REFERENCES users(id),
    amount_satoshis BIGINT NOT NULL,
    destination_address TEXT NOT NULL,
    txid VARCHAR(64),
    status VARCHAR(50) NOT NULL,
    signature TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);

CREATE INDEX idx_withdrawals_deposit_id ON withdrawals(deposit_id);
CREATE INDEX idx_withdrawals_user_id ON withdrawals(user_id);
CREATE INDEX idx_withdrawals_status ON withdrawals(status);

-- Interest rate history
CREATE TABLE interest_rates (
    id SERIAL PRIMARY KEY,
    utilization_rate DECIMAL(10, 6) NOT NULL,
    borrow_apy DECIMAL(10, 6) NOT NULL,
    supply_apy DECIMAL(10, 6) NOT NULL,
    total_deposits BIGINT NOT NULL,
    total_borrowed BIGINT NOT NULL,
    commitment_hash VARCHAR(64),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_interest_rates_created_at ON interest_rates(created_at DESC);

-- Balances view (computed)
CREATE OR REPLACE VIEW user_balances AS
SELECT 
    u.id as user_id,
    u.paymail,
    COALESCE(SUM(d.amount_satoshis) FILTER (WHERE d.status IN ('Confirmed', 'Available')), 0) as balance_satoshis,
    COALESCE(SUM(ia.amount_satoshis) FILTER (WHERE NOT ia.paid_out), 0) as accrued_interest_satoshis,
    COUNT(d.id) FILTER (WHERE d.status = 'Confirmed') as active_deposits
FROM users u
LEFT JOIN deposits d ON u.id = d.user_id
LEFT JOIN interest_accruals ia ON u.id = ia.user_id
GROUP BY u.id, u.paymail;
