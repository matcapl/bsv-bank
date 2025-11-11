# Phase 4: Payment Channels - Technical Specification

**Version:** 1.0  
**Date:** November 11, 2025  
**Status:** Planning  

---

## Executive Summary

This specification defines the complete implementation of payment channels for BSV Bank, enabling instant, low-cost micropayments between users through off-chain state channels with on-chain settlement.

### Goals
- **Instant payments:** Sub-second transaction processing
- **Low cost:** Minimal fees for micropayments
- **Scalability:** Support thousands of payments per channel
- **Security:** Cryptographically secure state management
- **Reliability:** 99.9% uptime with data persistence

---

## 1. System Architecture

### 1.1 Overview

```
┌──────────────────────────────────────────────────────────┐
│                    Frontend (React)                       │
│  - Channel management UI                                  │
│  - Payment sending interface                              │
│  - Real-time balance updates                              │
│  - Channel history viewer                                 │
└──────────────────────────────────────────────────────────┘
                           ↓ HTTP/REST
┌──────────────────────────────────────────────────────────┐
│            Payment Channel Service (Rust)                 │
│                     Port 8083                             │
│                                                           │
│  ┌──────────────┬──────────────┬──────────────┐         │
│  │   Channel    │   Payment    │   State      │         │
│  │   Manager    │   Processor  │   Manager    │         │
│  └──────────────┴──────────────┴──────────────┘         │
│                                                           │
│  ┌──────────────────────────────────────────┐           │
│  │        Business Logic Layer               │           │
│  │  - Balance validation                     │           │
│  │  - Sequence number tracking               │           │
│  │  - Signature verification                 │           │
│  │  - Settlement calculation                 │           │
│  └──────────────────────────────────────────┘           │
└──────────────────────────────────────────────────────────┘
                           ↓ SQL
┌──────────────────────────────────────────────────────────┐
│                  PostgreSQL Database                      │
│  - payment_channels                                       │
│  - channel_states                                         │
│  - channel_payments                                       │
└──────────────────────────────────────────────────────────┘
```

### 1.2 Service Components

**Payment Channel Service** (`payment-channel-service`)
- **Purpose:** Manage payment channels and process micropayments
- **Port:** 8083
- **Language:** Rust + Actix-web
- **Database:** PostgreSQL (shared with other services)

---

## 2. Database Schema

### 2.1 Tables

#### `payment_channels`
Primary table for channel management.

```sql
CREATE TABLE payment_channels (
    -- Identity
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    channel_id VARCHAR(66) UNIQUE NOT NULL,  -- Hash of parties + timestamp
    
    -- Parties
    party_a_paymail VARCHAR(255) NOT NULL,
    party_b_paymail VARCHAR(255) NOT NULL,
    
    -- Balances (in satoshis)
    initial_balance_a BIGINT NOT NULL CHECK (initial_balance_a >= 0),
    initial_balance_b BIGINT NOT NULL CHECK (initial_balance_b >= 0),
    current_balance_a BIGINT NOT NULL CHECK (current_balance_a >= 0),
    current_balance_b BIGINT NOT NULL CHECK (current_balance_b >= 0),
    
    -- State management
    status VARCHAR(20) NOT NULL DEFAULT 'Open',
        -- Values: 'Open', 'Active', 'Closing', 'Closed', 'Disputed'
    sequence_number BIGINT NOT NULL DEFAULT 0,
    
    -- Timing
    opened_at TIMESTAMP NOT NULL DEFAULT NOW(),
    closed_at TIMESTAMP,
    last_payment_at TIMESTAMP,
    
    -- Settlement
    settlement_txid VARCHAR(64),
    timeout_blocks INT DEFAULT 144,  -- ~24 hours at 10min blocks
    
    -- Metadata
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    
    -- Constraints
    CONSTRAINT balance_conservation CHECK (
        current_balance_a + current_balance_b = 
        initial_balance_a + initial_balance_b
    ),
    CONSTRAINT different_parties CHECK (party_a_paymail != party_b_paymail)
);

-- Indexes for performance
CREATE INDEX idx_channels_party_a ON payment_channels(party_a_paymail);
CREATE INDEX idx_channels_party_b ON payment_channels(party_b_paymail);
CREATE INDEX idx_channels_status ON payment_channels(status);
CREATE INDEX idx_channels_opened_at ON payment_channels(opened_at);
```

#### `channel_states`
Audit trail for all channel state changes.

```sql
CREATE TABLE channel_states (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    channel_id VARCHAR(66) NOT NULL REFERENCES payment_channels(channel_id) ON DELETE CASCADE,
    
    -- State snapshot
    sequence_number BIGINT NOT NULL,
    balance_a BIGINT NOT NULL,
    balance_b BIGINT NOT NULL,
    
    -- Cryptographic proof (Phase 5)
    state_hash VARCHAR(64),
    signature_a TEXT,
    signature_b TEXT,
    
    -- Metadata
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    
    -- Ensure unique sequence per channel
    UNIQUE(channel_id, sequence_number)
);

CREATE INDEX idx_channel_states_channel ON channel_states(channel_id);
CREATE INDEX idx_channel_states_sequence ON channel_states(channel_id, sequence_number DESC);
```

#### `channel_payments`
Individual payment records through channels.

```sql
CREATE TABLE channel_payments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    channel_id VARCHAR(66) NOT NULL REFERENCES payment_channels(channel_id) ON DELETE CASCADE,
    
    -- Payment details
    from_paymail VARCHAR(255) NOT NULL,
    to_paymail VARCHAR(255) NOT NULL,
    amount_satoshis BIGINT NOT NULL CHECK (amount_satoshis > 0),
    
    -- Context
    sequence_number BIGINT NOT NULL,
    memo TEXT,
    
    -- State after payment
    balance_a_after BIGINT NOT NULL,
    balance_b_after BIGINT NOT NULL,
    
    -- Metadata
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    processing_time_ms INT  -- For performance monitoring
);

CREATE INDEX idx_channel_payments_channel ON channel_payments(channel_id);
CREATE INDEX idx_channel_payments_created ON channel_payments(created_at DESC);
CREATE INDEX idx_channel_payments_from ON channel_payments(from_paymail);
CREATE INDEX idx_channel_payments_to ON channel_payments(to_paymail);
```

### 2.2 Database Functions

```sql
-- Function to update channel balances atomically
CREATE OR REPLACE FUNCTION process_channel_payment(
    p_channel_id VARCHAR(66),
    p_from_paymail VARCHAR(255),
    p_to_paymail VARCHAR(255),
    p_amount BIGINT
) RETURNS JSON AS $$
DECLARE
    v_channel payment_channels;
    v_new_balance_a BIGINT;
    v_new_balance_b BIGINT;
    v_new_sequence BIGINT;
BEGIN
    -- Lock the channel row
    SELECT * INTO v_channel
    FROM payment_channels
    WHERE channel_id = p_channel_id
    FOR UPDATE;
    
    -- Validate channel status
    IF v_channel.status NOT IN ('Open', 'Active') THEN
        RAISE EXCEPTION 'Channel is not active';
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
    IF v_new_balance_a < 0 OR v_new_balance_b < 0 THEN
        RAISE EXCEPTION 'Insufficient balance';
    END IF;
    
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
    
    -- Return new state
    RETURN json_build_object(
        'sequence_number', v_new_sequence,
        'balance_a', v_new_balance_a,
        'balance_b', v_new_balance_b
    );
END;
$$ LANGUAGE plpgsql;
```

---

## 3. API Specification

### 3.1 Endpoints

#### Health Check
```
GET /health
```
**Response:**
```json
{
  "service": "payment-channel-service",
  "status": "healthy",
  "version": "1.0.0",
  "uptime_seconds": 3600
}
```

#### Open Channel
```
POST /channels/open
```
**Request:**
```json
{
  "party_a_paymail": "alice@test.io",
  "party_b_paymail": "bob@test.io",
  "initial_balance_a": 100000,
  "initial_balance_b": 0,
  "timeout_blocks": 144
}
```
**Response:**
```json
{
  "channel_id": "0x123abc...",
  "party_a_paymail": "alice@test.io",
  "party_b_paymail": "bob@test.io",
  "initial_balance_a": 100000,
  "initial_balance_b": 0,
  "current_balance_a": 100000,
  "current_balance_b": 0,
  "status": "Open",
  "sequence_number": 0,
  "opened_at": "2025-11-11T10:00:00Z",
  "timeout_blocks": 144
}
```

#### Send Payment
```
POST /channels/{channel_id}/payment
```
**Request:**
```json
{
  "from_paymail": "alice@test.io",
  "to_paymail": "bob@test.io",
  "amount_satoshis": 1000,
  "memo": "Coffee payment"
}
```
**Response:**
```json
{
  "payment_id": "uuid-...",
  "channel_id": "0x123abc...",
  "from_paymail": "alice@test.io",
  "to_paymail": "bob@test.io",
  "amount_satoshis": 1000,
  "sequence_number": 1,
  "balance_a": 99000,
  "balance_b": 1000,
  "created_at": "2025-11-11T10:01:00Z",
  "processing_time_ms": 15
}
```

#### Get Channel Details
```
GET /channels/{channel_id}
```
**Response:**
```json
{
  "channel_id": "0x123abc...",
  "party_a_paymail": "alice@test.io",
  "party_b_paymail": "bob@test.io",
  "initial_balance_a": 100000,
  "initial_balance_b": 0,
  "current_balance_a": 99000,
  "current_balance_b": 1000,
  "status": "Active",
  "sequence_number": 1,
  "opened_at": "2025-11-11T10:00:00Z",
  "last_payment_at": "2025-11-11T10:01:00Z",
  "timeout_blocks": 144,
  "total_payments": 1
}
```

#### Get Payment History
```
GET /channels/{channel_id}/history
```
**Query Params:**
- `limit`: Number of payments to return (default: 50)
- `offset`: Pagination offset (default: 0)

**Response:**
```json
{
  "channel_id": "0x123abc...",
  "total_payments": 15,
  "payments": [
    {
      "payment_id": "uuid-...",
      "from_paymail": "alice@test.io",
      "to_paymail": "bob@test.io",
      "amount_satoshis": 1000,
      "sequence_number": 15,
      "balance_a_after": 85000,
      "balance_b_after": 15000,
      "memo": "Latest payment",
      "created_at": "2025-11-11T10:15:00Z"
    }
  ]
}
```

#### Get User's Channels
```
GET /channels/user/{paymail}
```
**Response:**
```json
{
  "paymail": "alice@test.io",
  "total_channels": 3,
  "channels": [
    {
      "channel_id": "0x123abc...",
      "counterparty": "bob@test.io",
      "role": "party_a",
      "balance": 99000,
      "status": "Active",
      "opened_at": "2025-11-11T10:00:00Z"
    }
  ]
}
```

#### Get Current Balance
```
GET /channels/{channel_id}/balance
```
**Response:**
```json
{
  "channel_id": "0x123abc...",
  "party_a_paymail": "alice@test.io",
  "party_b_paymail": "bob@test.io",
  "balance_a": 99000,
  "balance_b": 1000,
  "sequence_number": 1
}
```

#### Close Channel (Cooperative)
```
POST /channels/{channel_id}/close
```
**Request:**
```json
{
  "party_paymail": "alice@test.io"
}
```
**Response:**
```json
{
  "channel_id": "0x123abc...",
  "status": "Closed",
  "final_balance_a": 99000,
  "final_balance_b": 1000,
  "settlement_txid": "mock-tx-...",
  "closed_at": "2025-11-11T11:00:00Z",
  "total_payments": 15
}
```

#### Force Close Channel
```
POST /channels/{channel_id}/force-close
```
**Request:**
```json
{
  "party_paymail": "alice@test.io",
  "reason": "Counterparty unresponsive"
}
```
**Response:**
```json
{
  "channel_id": "0x123abc...",
  "status": "Disputed",
  "dispute_started_at": "2025-11-11T11:00:00Z",
  "timeout_expires_at": "2025-11-12T11:00:00Z",
  "current_balance_a": 99000,
  "current_balance_b": 1000
}
```

#### Get Channel Statistics
```
GET /channels/{channel_id}/stats
```
**Response:**
```json
{
  "channel_id": "0x123abc...",
  "total_payments": 15,
  "total_volume": 15000,
  "average_payment": 1000,
  "largest_payment": 5000,
  "smallest_payment": 100,
  "payment_frequency_per_hour": 12.5,
  "channel_age_hours": 1.5
}
```

#### Get Network Statistics
```
GET /stats/network
```
**Response:**
```json
{
  "total_channels": 150,
  "active_channels": 120,
  "total_value_locked": 15000000,
  "total_payments_24h": 5000,
  "total_volume_24h": 500000,
  "average_channel_balance": 100000
}
```

#### Check Timeouts (Admin)
```
POST /channels/check-timeouts
```
**Response:**
```json
{
  "checked_at": "2025-11-11T12:00:00Z",
  "expired_channels": 2,
  "channels": [
    {
      "channel_id": "0x....",
      "action": "force_closed"
    }
  ]
}
```

#### Database Consistency Check (Admin)
```
GET /admin/check-consistency
```
**Response:**
```json
{
  "status": "consistent",
  "channels_checked": 150,
  "issues_found": 0,
  "checked_at": "2025-11-11T12:00:00Z"
}
```

### 3.2 Error Responses

All errors follow this format:
```json
{
  "error": "Error type",
  "message": "Human-readable description",
  "details": {
    "field": "Additional context"
  },
  "timestamp": "2025-11-11T12:00:00Z"
}
```

**Common Error Codes:**
- `400` - Bad Request (invalid input)
- `404` - Not Found (channel doesn't exist)
- `409` - Conflict (channel already exists)
- `422` - Unprocessable Entity (business logic violation)
- `500` - Internal Server Error

---

## 4. Business Logic

### 4.1 Channel Lifecycle

```
Open → Active → Closing → Closed
  ↓              ↓
  └→ Disputed ───┘
```

**States:**
- **Open:** Channel created, no payments yet
- **Active:** Payments have been made
- **Closing:** Cooperative closure in progress
- **Closed:** Channel permanently closed, settled
- **Disputed:** Force closure initiated, timeout period active

### 4.2 Payment Processing Rules

1. **Balance Validation**
   - Sender must have sufficient balance
   - `new_balance_sender >= 0`
   - `new_balance_receiver >= 0`

2. **Amount Validation**
   - Amount must be positive: `amount > 0`
   - Amount must be integer (satoshis)
   - Maximum amount: sender's current balance

3. **Party Validation**
   - Only channel parties can send payments
   - Sender and receiver must be different
   - Must match channel parties

4. **State Validation**
   - Channel must be Open or Active
   - Cannot pay through Closed/Disputed channels

5. **Sequence Management**
   - Sequence increments by 1 per payment
   - Must be monotonically increasing
   - No gaps allowed

6. **Balance Conservation**
   - `balance_a + balance_b = constant`
   - Must equal initial total at all times

### 4.3 Closure Rules

**Cooperative Closure:**
- Either party can initiate
- Immediate settlement
- Final balances recorded
- No disputes

**Force Closure:**
- Initiated when counterparty unresponsive
- Timeout period begins (144 blocks default)
- Counterparty can challenge within timeout
- After timeout, current state settles

### 4.4 Timeout Management

```rust
// Pseudo-code for timeout logic
if channel.status == Disputed {
    let blocks_since_dispute = current_block - dispute_block;
    if blocks_since_dispute >= channel.timeout_blocks {
        // Force settle with current state
        settle_channel(channel, channel.current_state);
    }
}
```

---

## 5. Data Structures (Rust)

### 5.1 Core Types

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct PaymentChannel {
    pub id: Uuid,
    pub channel_id: String,
    pub party_a_paymail: String,
    pub party_b_paymail: String,
    pub initial_balance_a: i64,
    pub initial_balance_b: i64,
    pub current_balance_a: i64,
    pub current_balance_b: i64,
    pub status: ChannelStatus,
    pub sequence_number: i64,
    pub opened_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub last_payment_at: Option<DateTime<Utc>>,
    pub settlement_txid: Option<String>,
    pub timeout_blocks: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "varchar", rename_all = "PascalCase")]
pub enum ChannelStatus {
    Open,
    Active,
    Closing,
    Closed,
    Disputed,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenChannelRequest {
    pub party_a_paymail: String,
    pub party_b_paymail: String,
    pub initial_balance_a: i64,
    pub initial_balance_b: i64,
    #[serde(default = "default_timeout")]
    pub timeout_blocks: i32,
}

fn default_timeout() -> i32 {
    144
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SendPaymentRequest {
    pub from_paymail: String,
    pub to_paymail: String,
    pub amount_satoshis: i64,
    pub memo: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct ChannelPayment {
    pub id: Uuid,
    pub channel_id: String,
    pub from_paymail: String,
    pub to_paymail: String,
    pub amount_satoshis: i64,
    pub sequence_number: i64,
    pub memo: Option<String>,
    pub balance_a_after: i64,
    pub balance_b_after: i64,
    pub created_at: DateTime<Utc>,
    pub processing_time_ms: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct PaymentResponse {
    pub payment_id: Uuid,
    pub channel_id: String,
    pub from_paymail: String,
    pub to_paymail: String,
    pub amount_satoshis: i64,
    pub sequence_number: i64,
    pub balance_a: i64,
    pub balance_b: i64,
    pub created_at: DateTime<Utc>,
    pub processing_time_ms: i32,
}
```

---

## 6. Testing Requirements

### 6.1 Unit Tests

- Balance calculation logic
- Sequence number increment
- Party validation
- Amount validation
- Status transitions

### 6.2 Integration Tests

- Complete payment flow
- Channel lifecycle
- Concurrent payments
- Database consistency
- Error handling

### 6.3 Performance Tests

- Payment latency (target: <100ms)
- Throughput (target: >50 payments/sec)
- Concurrent users
- Database load

### 6.4 Acceptance Criteria

All tests in `test-phase4-complete.sh` must pass:
- 18 test sections
- 80+ individual tests
- 95%+ success rate required

---

## 7. Performance Requirements

| Metric | Target | Maximum |
|--------|--------|---------|
| Payment Latency | < 50ms | < 100ms |
| Throughput | > 50 tps | > 100 tps |
| Channel Opening | < 200ms | < 500ms |
| Database Queries | < 10ms | < 50ms |
| API Response Time | < 100ms | < 250ms |
| Uptime | 99.9% | - |

---

## 8. Security Considerations

### 8.1 Authentication
- Validate paymail ownership (Phase 5)
- Verify cryptographic signatures (Phase 5)
- Session management

### 8.2 Authorization
- Only channel parties can send payments
- Only parties can close channel
- Admin endpoints require auth

### 8.3 Input Validation
- Sanitize all inputs
- Validate amounts (positive, within limits)
- Check paymails format
- Prevent SQL injection

### 8.4 Concurrency
- Database row locking
- Atomic operations
- Sequence number protection
- Race condition prevention

---

## 9. Monitoring & Logging

### 9.1 Metrics to Track

- Total channels created
- Active channels
- Total payments processed
- Total volume
- Average payment size
- Payment latency (p50, p95, p99)
- Error rate
- Database query time

### 9.2 Logging Levels

```
ERROR: Critical failures
WARN:  Business logic violations
INFO:  Channel lifecycle events
DEBUG: Payment details
TRACE: Database queries (dev only)
```

---

## 10. Deployment

### 10.1 Dependencies

```toml
[dependencies]
actix-web = "4.4"
sqlx = { version = "0.7", features = ["postgres", "runtime-tokio", "uuid", "chrono"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1.0", features = ["serde", "v4"] }
```

### 10.2 Environment Variables

```bash
DATABASE_URL="postgresql://user:pass@localhost:5432/bsv_bank"
SERVICE_PORT=8083
LOG_LEVEL=info
RUST_BACKTRACE=1
```

### 10.3 Docker Integration

Update `docker-compose.yml`:
```yaml
services:
  payment-channels:
    build: ./core/payment-channel-service
    ports:
      - "8083:8083"
    environment:
      - DATABASE_URL=postgresql://...
    depends_on:
      - postgres
```

---

## 11. Success Metrics

**Phase 4 is complete when:**

- [ ] All database tables created
- [ ] Service builds without errors
- [ ] All 18 API endpoints implemented
- [ ] `test-phase4-complete.sh` passes (95%+)
- [ ] Performance targets met
- [ ] Documentation complete
- [ ] Frontend UI functional
- [ ] Integration with existing services
- [ ] Code review passed
- [ ] Deployment successful

---

## 12. Future Enhancements (Phase 5)

- Real blockchain settlement
- Cryptographic signatures
- Multi-hop payments (routing)
- WebSocket for real-time updates
- Channel rebalancing
- Atomic swaps
- Lightning Network compatibility

---

**This specification serves as the complete blueprint for Phase 4 implementation. All code must conform to these requirements.**