-- Loans table with integer basis points for interest rate
CREATE TABLE IF NOT EXISTS loans (
    id UUID PRIMARY KEY,
    borrower_paymail VARCHAR(255) NOT NULL,
    lender_paymail VARCHAR(255),
    principal_satoshis BIGINT NOT NULL CHECK (principal_satoshis > 0),
    collateral_satoshis BIGINT NOT NULL CHECK (collateral_satoshis > 0),
    interest_rate_bps INTEGER NOT NULL CHECK (interest_rate_bps >= 0),
    interest_accrued BIGINT NOT NULL DEFAULT 0,
    status VARCHAR(50) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    due_date TIMESTAMPTZ NOT NULL,
    repaid_at TIMESTAMPTZ,
    liquidated_at TIMESTAMPTZ,
    CONSTRAINT check_collateral_ratio CHECK (collateral_satoshis >= principal_satoshis * 1.5)
);

CREATE INDEX idx_loans_status ON loans(status);
CREATE INDEX idx_loans_borrower ON loans(borrower_paymail);
CREATE INDEX idx_loans_lender ON loans(lender_paymail);
CREATE INDEX idx_loans_due_date ON loans(due_date);
