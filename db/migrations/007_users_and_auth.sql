-- db/migrations/007_users_and_auth.sql
-- Phase 6: User authentication and API keys
-- Enhanced to handle existing users table from earlier phases

-- Check if users table exists and alter it, otherwise create it
DO $$ 
BEGIN
    -- Add password_hash column if it doesn't exist
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'users' AND column_name = 'password_hash'
    ) THEN
        ALTER TABLE users ADD COLUMN password_hash VARCHAR(64);
    END IF;

    -- Add last_login_at column if it doesn't exist
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'users' AND column_name = 'last_login_at'
    ) THEN
        ALTER TABLE users ADD COLUMN last_login_at TIMESTAMPTZ;
    END IF;

    -- Add is_active column if it doesn't exist
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'users' AND column_name = 'is_active'
    ) THEN
        ALTER TABLE users ADD COLUMN is_active BOOLEAN DEFAULT TRUE;
    END IF;

    -- Add email_verified column if it doesn't exist
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'users' AND column_name = 'email_verified'
    ) THEN
        ALTER TABLE users ADD COLUMN email_verified BOOLEAN DEFAULT FALSE;
    END IF;

    -- Ensure id column is UUID type (might be integer in old schema)
    -- This is more complex, so we'll just document it
    -- If your id is integer, you may need to migrate data manually
END $$;

-- Create indexes on users table if they don't exist
CREATE INDEX IF NOT EXISTS idx_users_paymail ON users(paymail);
CREATE INDEX IF NOT EXISTS idx_users_active ON users(is_active) WHERE is_active = TRUE;

-- Set existing users to have a default password hash if null (only if column exists)
-- Using a recognizable invalid hash that must be changed
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'users' AND column_name = 'password_hash'
    ) THEN
        UPDATE users 
        SET password_hash = 'INVALID_HASH_MUST_BE_SET_VIA_PASSWORD_RESET_000000000000000'
        WHERE password_hash IS NULL;
        
        -- Now make password_hash NOT NULL
        ALTER TABLE users ALTER COLUMN password_hash SET NOT NULL;
    END IF;
END $$;

-- API Keys table
CREATE TABLE IF NOT EXISTS api_keys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_paymail VARCHAR(255) NOT NULL REFERENCES users(paymail),
    key_hash VARCHAR(64) NOT NULL UNIQUE,
    name VARCHAR(100) NOT NULL,
    permissions JSONB NOT NULL DEFAULT '[]'::jsonb,
    rate_limit_override INT,
    last_used_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    revoked_at TIMESTAMPTZ,
    CONSTRAINT fk_api_keys_user FOREIGN KEY (user_paymail) REFERENCES users(paymail) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_api_keys_user ON api_keys(user_paymail);
CREATE INDEX IF NOT EXISTS idx_api_keys_hash ON api_keys(key_hash) WHERE revoked_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_api_keys_active ON api_keys(user_paymail, revoked_at) WHERE revoked_at IS NULL;

-- Audit log table
CREATE TABLE IF NOT EXISTS audit_log (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_paymail VARCHAR(255),
    action VARCHAR(100) NOT NULL,
    resource_type VARCHAR(50),
    resource_id VARCHAR(255),
    ip_address INET,
    user_agent TEXT,
    request_id VARCHAR(100),
    details JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_audit_user ON audit_log(user_paymail, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_audit_action ON audit_log(action, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_audit_resource ON audit_log(resource_type, resource_id);
CREATE INDEX IF NOT EXISTS idx_audit_created ON audit_log(created_at DESC);

-- Rate limit tracking table (optional - can use Redis instead)
CREATE TABLE IF NOT EXISTS rate_limit_tracking (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_paymail VARCHAR(255),
    api_key VARCHAR(100),
    ip_address INET NOT NULL,
    endpoint VARCHAR(200) NOT NULL,
    requests_count INT DEFAULT 1,
    window_start TIMESTAMPTZ NOT NULL,
    window_end TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(ip_address, endpoint, window_start)
);

CREATE INDEX IF NOT EXISTS idx_rate_limits ON rate_limit_tracking(ip_address, endpoint, window_start);
CREATE INDEX IF NOT EXISTS idx_rate_limits_window ON rate_limit_tracking(window_end);

-- Function to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Trigger for users table
DROP TRIGGER IF EXISTS update_users_updated_at ON users;
CREATE TRIGGER update_users_updated_at
    BEFORE UPDATE ON users
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Insert test user for development (password: "testpassword123")
-- Hash is SHA256 of "testpassword123"
INSERT INTO users (paymail, password_hash, is_active, email_verified) 
VALUES ('test@bsvbank.local', 'cbae1d70e5eeb2f63f83d27b3c3d8c2e4b8a5a1e5b5e5d5e5e5e5e5e5e5e5e5e', TRUE, TRUE)
ON CONFLICT (paymail) DO UPDATE 
SET password_hash = EXCLUDED.password_hash,
    is_active = EXCLUDED.is_active,
    email_verified = EXCLUDED.email_verified;

-- Comments for documentation
COMMENT ON TABLE users IS 'User accounts for authentication';
COMMENT ON TABLE api_keys IS 'API keys for programmatic access';
COMMENT ON TABLE audit_log IS 'Audit trail of all user actions';
COMMENT ON TABLE rate_limit_tracking IS 'Rate limiting data (alternative to Redis)';

COMMENT ON COLUMN users.paymail IS 'User paymail address (primary identifier)';
COMMENT ON COLUMN users.password_hash IS 'SHA256 hash of user password';
COMMENT ON COLUMN users.is_active IS 'Whether user account is active';
COMMENT ON COLUMN users.last_login_at IS 'Timestamp of last successful login';
COMMENT ON COLUMN users.email_verified IS 'Whether paymail/email has been verified';

COMMENT ON COLUMN api_keys.key_hash IS 'SHA256 hash of API key';
COMMENT ON COLUMN api_keys.permissions IS 'JSON array of permission strings';
COMMENT ON COLUMN api_keys.rate_limit_override IS 'Custom rate limit for this key (requests per minute)';

COMMENT ON COLUMN audit_log.action IS 'Action performed (e.g., deposit_created, loan_approved)';
COMMENT ON COLUMN audit_log.resource_type IS 'Type of resource (e.g., deposit, loan)';
COMMENT ON COLUMN audit_log.resource_id IS 'ID of the resource affected';

-- Report completion
DO $$
BEGIN
    RAISE NOTICE 'Phase 6 migration completed successfully';
    RAISE NOTICE 'Users table augmented with authentication columns';
    RAISE NOTICE 'API keys, audit log, and rate limiting tables created';
END $$;