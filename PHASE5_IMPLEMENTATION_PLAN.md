# Phase 5: BSV Testnet Integration - Complete Implementation Plan

**Version:** 1.0  
**Date:** November 14, 2025  
**Status:** Ready for Implementation  
**Duration:** 5 weeks (Nov 13 - Dec 18, 2025)

---

## 🎯 Executive Summary

Phase 5 transforms BSV Bank from a **mock transaction system** into a **real blockchain-integrated platform** using BSV testnet. This phase maintains all Phase 4 functionality while adding actual on-chain settlement capabilities.

**Key Principle:** Keep fast off-chain payments (10ms), add slow on-chain settlement (10 minutes).

---

## 📋 Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Week-by-Week Implementation](#week-by-week-implementation)
3. [Service Specifications](#service-specifications)
4. [Database Schema](#database-schema)
5. [Integration Points](#integration-points)
6. [Testing Strategy](#testing-strategy)
7. [Security Considerations](#security-considerations)
8. [Success Criteria](#success-criteria)

---

## Architecture Overview

### System Design Philosophy

```
┌─────────────────────────────────────────────────────────────┐
│                    BSV Bank Architecture                     │
│                                                              │
│  ┌──────────────┐         ┌──────────────┐                 │
│  │   Phase 4    │         │   Phase 5    │                 │
│  │ (Off-Chain)  │ ◄─────► │ (On-Chain)   │                 │
│  │              │         │              │                 │
│  │ • Fast (10ms)│         │ • Slow (10m) │                 │
│  │ • Free       │         │ • Fees       │                 │
│  │ • Database   │         │ • Blockchain │                 │
│  └──────────────┘         └──────────────┘                 │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

### Service Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Frontend (React)                         │
│              localhost:3000 - User Interface                 │
└────────────┬────────────────────────────────────────────────┘
             │
             │ HTTP/JSON
             │
┌────────────┴────────────────────────────────────────────────┐
│                  Existing Services (Phase 1-4)               │
├──────────────────┬───────────────────┬──────────────────────┤
│  Deposit Service │ Interest Engine   │ Lending Service      │
│  Port 8080       │ Port 8081         │ Port 8082            │
│                  │                   │                      │
│  • Deposits      │ • APY Calc        │ • Loan Requests      │
│  • Balances      │ • Accruals        │ • Funding            │
│  • Mock TXIDs    │ • Rates           │ • Repayments         │
└──────────────────┴───────────────────┴──────────────────────┘
             │
             │ Enhanced with blockchain verification
             │
┌────────────┴────────────────────────────────────────────────┐
│             Payment Channel Service (Enhanced)               │
│                      Port 8083                               │
│                                                              │
│  Phase 4 Features:          Phase 5 Additions:              │
│  • Channel creation         • Real funding TXs              │
│  • Micropayments (mock)     • On-chain settlement           │
│  • Balance tracking         • SPV verification              │
│  • Cooperative close        • Force-close TXs               │
│  • Mock TXIDs               • Testnet broadcasting          │
└──────────────────────────────────────────────────────────────┘
             │
             │ Uses new Phase 5 services
             │
┌────────────┴────────────────────────────────────────────────┐
│                    New Phase 5 Services                      │
├──────────────────┬───────────────────┬──────────────────────┤
│ Blockchain       │  Transaction      │  SPV Verification    │
│ Monitor          │  Builder          │  Service             │
│ Port 8084        │  Port 8085        │  Port 8086           │
│                  │                   │                      │
│ • TX monitoring  │ • Build TXs       │ • Merkle proofs      │
│ • Confirmations  │ • Sign TXs        │ • Block headers      │
│ • Address watch  │ • Broadcast       │ • Confirmation count │
│ • Webhooks       │ • Fee estimation  │ • Reorg detection    │
└──────────────────┴───────────────────┴──────────────────────┘
             │
             │ All services query
             │
┌────────────┴────────────────────────────────────────────────┐
│                      Data Layer                              │
├──────────────────┬───────────────────────────────────────────┤
│  PostgreSQL      │       BSV Testnet                        │
│  Port 5432       │   (WhatsOnChain API)                     │
│                  │                                           │
│  • User data     │  • Real transactions                     │
│  • Mock TXIDs    │  • Block confirmations                   │
│  • Balances      │  • UTXO data                             │
│  • Channel state │  • Network info                          │
└──────────────────┴───────────────────────────────────────────┘
```

---

## Week-by-Week Implementation

### Week 1: Blockchain Monitor Service (Nov 13-20)

**Goal:** Can query BSV testnet and track transactions in real-time.

#### Day 1-2: Service Foundation

**Tasks:**
- [ ] Create new Rust project: `services/blockchain-monitor`
- [ ] Set up basic Actix-web server (port 8084)
- [ ] Implement WhatsOnChain API client
- [ ] Add health check endpoint
- [ ] Create basic logging infrastructure

**Deliverables:**
```rust
// blockchain-monitor/src/main.rs
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .route("/health", web::get().to(health_check))
            .route("/tx/{txid}", web::get().to(get_transaction))
            .route("/address/{address}/utxos", web::get().to(get_utxos))
    })
    .bind("127.0.0.1:8084")?
    .run()
    .await
}
```

**API Endpoints:**
```
GET  /health                          → Service health
GET  /tx/{txid}                       → Transaction details
GET  /tx/{txid}/confirmations         → Confirmation count
GET  /address/{address}/balance       → Address balance
GET  /address/{address}/utxos         → List UTXOs
GET  /chain/info                      → Network info
```

#### Day 3-4: Transaction Monitoring

**Tasks:**
- [ ] Implement transaction polling system
- [ ] Add confirmation tracking
- [ ] Create webhook notification system
- [ ] Build address watch list
- [ ] Add transaction cache (reduce API calls)

**Database Tables:**
```sql
-- Track watched addresses
CREATE TABLE watched_addresses (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    address VARCHAR(255) NOT NULL UNIQUE,
    paymail VARCHAR(255) NOT NULL,
    purpose VARCHAR(50) NOT NULL, -- 'deposit', 'channel', 'lending'
    created_at TIMESTAMPTZ DEFAULT NOW(),
    last_checked TIMESTAMPTZ,
    INDEX idx_watched_addresses_paymail (paymail)
);

-- Cache transaction data
CREATE TABLE blockchain_transactions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    txid VARCHAR(64) NOT NULL UNIQUE,
    tx_type VARCHAR(20) NOT NULL, -- 'deposit', 'withdrawal', 'funding', 'settlement'
    from_address VARCHAR(255),
    to_address VARCHAR(255),
    amount_satoshis BIGINT NOT NULL,
    fee_satoshis BIGINT,
    confirmations INT DEFAULT 0,
    status VARCHAR(20) DEFAULT 'pending', -- 'pending', 'confirmed', 'failed'
    block_hash VARCHAR(64),
    block_height INT,
    block_time TIMESTAMPTZ,
    raw_tx TEXT,
    first_seen TIMESTAMPTZ DEFAULT NOW(),
    confirmed_at TIMESTAMPTZ,
    INDEX idx_blockchain_txid (txid),
    INDEX idx_blockchain_status (status),
    INDEX idx_blockchain_type (tx_type),
    INDEX idx_blockchain_height (block_height)
);

-- Track confirmation changes
CREATE TABLE confirmation_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    txid VARCHAR(64) NOT NULL,
    old_confirmations INT NOT NULL,
    new_confirmations INT NOT NULL,
    block_height INT,
    detected_at TIMESTAMPTZ DEFAULT NOW(),
    INDEX idx_confirmation_txid (txid),
    INDEX idx_confirmation_time (detected_at)
);

-- Store block headers for SPV
CREATE TABLE block_headers (
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
    created_at TIMESTAMPTZ DEFAULT NOW(),
    INDEX idx_block_hash (hash),
    INDEX idx_block_prev (prev_block),
    INDEX idx_block_time (timestamp)
);

-- Store Merkle proofs
CREATE TABLE merkle_proofs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    txid VARCHAR(64) NOT NULL,
    block_hash VARCHAR(64) NOT NULL,
    block_height INT,
    merkle_root VARCHAR(64) NOT NULL,
    siblings JSONB NOT NULL,
    tx_index INT NOT NULL,
    verified BOOLEAN DEFAULT false,
    verified_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    INDEX idx_merkle_txid (txid),
    INDEX idx_merkle_block (block_hash)
);

-- Transaction templates for channels
CREATE TABLE transaction_templates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    template_type VARCHAR(50) NOT NULL,
    channel_id UUID REFERENCES payment_channels(id),
    tx_hex TEXT NOT NULL,
    txid VARCHAR(64),
    status VARCHAR(20) DEFAULT 'unsigned',
    party_a_signed BOOLEAN DEFAULT false,
    party_b_signed BOOLEAN DEFAULT false,
    sequence_number INT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    broadcast_at TIMESTAMPTZ,
    INDEX idx_tx_templates_channel (channel_id),
    INDEX idx_tx_templates_type (template_type),
    INDEX idx_tx_templates_status (status)
);

-- Track signatures
CREATE TABLE transaction_signatures (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tx_template_id UUID REFERENCES transaction_templates(id),
    paymail VARCHAR(255) NOT NULL,
    signature TEXT NOT NULL,
    pubkey TEXT NOT NULL,
    signed_at TIMESTAMPTZ DEFAULT NOW(),
    INDEX idx_tx_sigs_template (tx_template_id)
);

-- Enhance existing payment_channels table
ALTER TABLE payment_channels ADD COLUMN blockchain_enabled BOOLEAN DEFAULT false;
ALTER TABLE payment_channels ADD COLUMN funding_address VARCHAR(255);
ALTER TABLE payment_channels ADD COLUMN funding_vout INT DEFAULT 0;
ALTER TABLE payment_channels ADD COLUMN settlement_txid VARCHAR(64);
ALTER TABLE payment_channels ADD COLUMN funding_confirmations INT DEFAULT 0;
ALTER TABLE payment_channels ADD COLUMN settlement_confirmations INT DEFAULT 0;
ALTER TABLE payment_channels ADD COLUMN spv_verified BOOLEAN DEFAULT false;
ALTER TABLE payment_channels ADD COLUMN multisig_script TEXT;

-- Track channel blockchain events
CREATE TABLE channel_blockchain_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    channel_id UUID REFERENCES payment_channels(id),
    event_type VARCHAR(50) NOT NULL,
    txid VARCHAR(64),
    confirmations INT,
    block_height INT,
    event_data JSONB,
    event_time TIMESTAMPTZ DEFAULT NOW(),
    INDEX idx_channel_events_channel (channel_id),
    INDEX idx_channel_events_type (event_type),
    INDEX idx_channel_events_time (event_time)
);

-- Track API calls for rate limiting
CREATE TABLE api_rate_limits (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    service_name VARCHAR(50) NOT NULL,
    endpoint VARCHAR(255) NOT NULL,
    calls_count INT DEFAULT 1,
    window_start TIMESTAMPTZ DEFAULT NOW(),
    last_call TIMESTAMPTZ DEFAULT NOW(),
    INDEX idx_rate_limits_service (service_name),
    INDEX idx_rate_limits_window (window_start)
);

-- Store faucet requests for testing
CREATE TABLE testnet_faucet_requests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    address VARCHAR(255) NOT NULL,
    paymail VARCHAR(255),
    amount_requested BIGINT,
    txid VARCHAR(64),
    status VARCHAR(20) DEFAULT 'pending',
    requested_at TIMESTAMPTZ DEFAULT NOW(),
    fulfilled_at TIMESTAMPTZ,
    INDEX idx_faucet_address (address),
    INDEX idx_faucet_status (status)
);
```

---

## Integration Points

### 1. Deposit Service Integration

**Current State (Phase 4):**
```rust
// Mock deposit with fake TXID
pub async fn create_deposit(
    paymail: String,
    amount: i64,
) -> Result<Deposit> {
    let mock_txid = generate_mock_txid();
    // Store in database
}
```

**Phase 5 Enhancement:**
```rust
// Real deposit with blockchain verification
pub async fn create_deposit_verified(
    paymail: String,
    amount: i64,
    txid: Option<String>,
) -> Result<Deposit> {
    
    if let Some(real_txid) = txid {
        // Verify transaction exists on blockchain
        let tx = blockchain_monitor
            .get_transaction(&real_txid)
            .await?;
        
        // Verify it pays to user's address
        let user_address = get_user_address(&paymail).await?;
        verify_tx_pays_to(&tx, &user_address, amount)?;
        
        // Get SPV proof
        let proof = spv_service
            .get_merkle_proof(&real_txid)
            .await?;
        
        // Wait for confirmations
        blockchain_monitor
            .watch_transaction(&real_txid)
            .await?;
        
        // Create verified deposit
        Deposit {
            paymail,
            amount,
            txid: real_txid,
            verified: true,
            confirmations: tx.confirmations,
            spv_proof: Some(proof),
        }
    } else {
        // Fallback to Phase 4 mock mode
        create_mock_deposit(paymail, amount).await
    }
}
```

### 2. Lending Service Integration

**Collateral Verification:**
```rust
// Verify collateral is locked on-chain
pub async fn verify_loan_collateral(
    loan_id: Uuid,
    collateral_txid: String,
) -> Result<bool> {
    
    // Get loan details
    let loan = get_loan(loan_id).await?;
    
    // Verify TX on blockchain
    let tx = blockchain_monitor
        .get_transaction(&collateral_txid)
        .await?;
    
    // Check amount is sufficient (150% of loan)
    let required_collateral = loan.amount * 150 / 100;
    if tx.amount < required_collateral {
        return Err("Insufficient collateral");
    }
    
    // Verify it's locked (timelocked or multisig)
    verify_locked(&tx)?;
    
    // Get SPV proof
    let proof = spv_service
        .get_merkle_proof(&collateral_txid)
        .await?;
    
    Ok(proof.verified && tx.confirmations >= 1)
}
```

### 3. Interest Engine Integration

**On-Chain Interest Distribution:**
```rust
// Periodically distribute interest via blockchain
pub async fn distribute_interest_onchain(
    period: InterestPeriod,
) -> Result<Vec<String>> {
    
    let accruals = get_pending_accruals(period).await?;
    let mut txids = Vec::new();
    
    for accrual in accruals {
        // Build payment transaction
        let tx = transaction_builder
            .build_interest_payment(
                accrual.paymail,
                accrual.amount,
            )
            .await?;
        
        // Broadcast
        let txid = blockchain_monitor
            .broadcast_transaction(tx)
            .await?;
        
        txids.push(txid.clone());
        
        // Mark as paid
        mark_accrual_paid(&accrual.id, &txid).await?;
    }
    
    Ok(txids)
}
```

---

## Testing Strategy

### Test Pyramid Structure

```
                    ┌────────────┐
                    │  E2E Tests │  10 tests (Full workflows)
                    │   Manual   │
                    └────────────┘
                 ┌──────────────────┐
                 │ Integration Tests │  40 tests (Service interaction)
                 │    Automated      │
                 └──────────────────┘
            ┌─────────────────────────────┐
            │      Component Tests        │  90 tests (Per service)
            │        Automated            │
            └─────────────────────────────┘
       ┌───────────────────────────────────────┐
       │           Unit Tests                  │  200+ tests (Functions)
       │          Automated                    │
       └───────────────────────────────────────┘
```

### Test Coverage Goals

| Component | Unit Tests | Integration Tests | E2E Tests | Total |
|-----------|------------|-------------------|-----------|-------|
| Blockchain Monitor | 30 | 10 | 2 | 42 |
| Transaction Builder | 40 | 12 | 2 | 54 |
| SPV Service | 25 | 8 | 2 | 35 |
| Enhanced Channels | 35 | 10 | 4 | 49 |
| **Total** | **130** | **40** | **10** | **180** |

### Critical Test Scenarios

#### 1. Happy Path: Complete Channel Lifecycle
```
1. Alice and Bob generate addresses
2. Both fund addresses from testnet faucet
3. Create payment channel with real funding TX
4. Wait for funding confirmation (1 block)
5. Exchange 100 micropayments off-chain
6. Cooperative close with settlement TX
7. Wait for settlement confirmation
8. Verify final balances on-chain
9. SPV verification of all transactions
```

#### 2. Force Close Scenario
```
1. Create funded channel
2. Exchange 50 payments
3. Bob goes offline
4. Alice broadcasts latest commitment TX
5. Wait for timelock (144 blocks on testnet ≈ 24 hours)
6. Alice claims her balance
7. Verify force close on-chain
```

#### 3. Reorganization Handling
```
1. Create channel with funding TX
2. Funding TX gets 1 confirmation
3. Blockchain reorganization occurs
4. System detects reorg
5. Re-verify funding TX
6. Update confirmation count
7. Channel remains secure
```

#### 4. Double-Spend Detection
```
1. Create channel funding TX
2. Detect conflicting TX in mempool
3. Alert system of double-spend attempt
4. Refuse to proceed with channel
5. Return funds safely
```

#### 5. Network Failure Recovery
```
1. Create channel
2. Network connection lost
3. System queues transactions
4. Network reconnects
5. Re-broadcast pending transactions
6. Verify all succeeded
```

---

## Security Considerations

### 1. Private Key Management

**Testnet Phase (Phase 5):**
```rust
// Acceptable for testnet
pub struct KeyStore {
    keys: HashMap<String, PrivateKey>, // In-memory only
}
```

**Mainnet Phase (Phase 6):**
```rust
// Required for mainnet
pub struct SecureKeyStore {
    db: EncryptedDatabase,
    hsm: HardwareSecurityModule,
    backup: MultiSigBackup,
}
```

### 2. Transaction Validation

**Critical Checks:**
```rust
pub fn validate_transaction(tx: &Transaction) -> Result<()> {
    // 1. Check all inputs exist
    verify_inputs_exist(&tx.inputs)?;
    
    // 2. Verify signatures
    verify_all_signatures(tx)?;
    
    // 3. Check amounts (no inflation)
    let input_sum = tx.inputs.iter().sum();
    let output_sum = tx.outputs.iter().sum();
    if input_sum < output_sum {
        return Err("Output exceeds input");
    }
    
    // 4. Verify scripts
    verify_script_semantics(&tx.outputs)?;
    
    // 5. Check for double-spends
    check_double_spend(&tx.inputs)?;
    
    Ok(())
}
```

### 3. SPV Security Model

**Trust Assumptions:**
- ✅ Trust longest chain (most proof-of-work)
- ✅ Trust Merkle proofs from valid blocks
- ⚠️ Don't trust unconfirmed transactions
- ⚠️ Watch for reorganizations
- ⚠️ Require multiple confirmations for large amounts

**Mitigation Strategies:**
```rust
pub struct ConfirmationRequirements {
    small_tx: u32,      // < 1000 sats: 1 confirmation
    medium_tx: u32,     // < 10000 sats: 3 confirmations
    large_tx: u32,      // >= 10000 sats: 6 confirmations
    channel_funding: u32, // Always: 1 confirmation
    channel_close: u32,   // Always: 1 confirmation
}
```

### 4. Rate Limiting

**API Protection:**
```rust
pub struct RateLimiter {
    max_calls_per_minute: u32,
    max_calls_per_hour: u32,
}

impl RateLimiter {
    pub async fn check_limit(&self, service: &str) -> Result<()> {
        let calls_minute = get_recent_calls(service, 60).await?;
        if calls_minute > self.max_calls_per_minute {
            return Err("Rate limit exceeded");
        }
        Ok(())
    }
}
```

### 5. Error Handling

**Critical Error Scenarios:**
```rust
pub enum CriticalError {
    DoubleSpendDetected(String),
    ReorganizationDetected(u32),
    InvalidMerkleProof(String),
    InsufficientConfirmations(u32),
    NetworkFailure(String),
}

pub async fn handle_critical_error(error: CriticalError) {
    match error {
        DoubleSpendDetected(txid) => {
            alert_admin(&format!("Double-spend: {}", txid));
            freeze_affected_channels(&txid).await;
        }
        ReorganizationDetected(depth) => {
            if depth > 6 {
                alert_admin(&format!("Deep reorg: {} blocks", depth));
                pause_all_operations().await;
            }
            revalidate_recent_transactions(depth).await;
        }
        // ... handle other errors
    }
}
```

---

## Success Criteria

### Phase 5 Complete When:

#### Technical Criteria
- [ ] ✅ All 3 new services running (8084, 8085, 8086)
- [ ] ✅ Database schema fully migrated
- [ ] ✅ 180+ tests passing (>95% coverage)
- [ ] ✅ Can create channel with real testnet funding
- [ ] ✅ Can exchange 100+ off-chain payments
- [ ] ✅ Can close channel cooperatively with real settlement
- [ ] ✅ SPV verification works for all transactions
- [ ] ✅ Force-close mechanism operational
- [ ] ✅ Reorganization detection functional
- [ ] ✅ Phase 4 mock mode still works (backwards compatible)

#### Performance Criteria
- [ ] ✅ Off-chain payments: <20ms latency (same as Phase 4)
- [ ] ✅ Transaction building: <50ms
- [ ] ✅ Blockchain query (cached): <100ms
- [ ] ✅ SPV verification: <100ms
- [ ] ✅ Confirmation tracking: 10-second intervals
- [ ] ✅ Can handle 100+ concurrent channels

#### Integration Criteria
- [ ] ✅ Deposit service verifies real transactions
- [ ] ✅ Lending service verifies collateral on-chain
- [ ] ✅ Interest engine can distribute via blockchain
- [ ] ✅ Frontend shows testnet transaction links
- [ ] ✅ All existing Phase 1-4 features still work

#### Documentation Criteria
- [ ] ✅ Complete API reference
- [ ] ✅ Testnet setup guide
- [ ] ✅ Migration guide from Phase 4
- [ ] ✅ Architecture documentation
- [ ] ✅ Troubleshooting guide

#### Operational Criteria
- [ ] ✅ 5+ successful channel lifecycles on testnet
- [ ] ✅ No critical bugs in 48-hour test period
- [ ] ✅ Monitoring and alerting operational
- [ ] ✅ Backup and recovery procedures documented
- [ ] ✅ Alpha testers can successfully use system

---

## Risk Mitigation

### Risk Matrix

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| WhatsOnChain API downtime | Medium | High | Cache aggressively, fallback API |
| Testnet reorganization | Low | Medium | SPV revalidation, confirmation requirements |
| Transaction stuck in mempool | Medium | Low | Fee bumping, rebroadcast logic |
| Database migration issues | Low | High | Comprehensive backups, rollback plan |
| Performance degradation | Medium | Medium | Load testing, optimization passes |
| Security vulnerability | Low | Critical | Code review, security audit |

### Contingency Plans

**If WhatsOnChain API fails:**
1. Use cached data for 10 minutes
2. Switch to backup API (mattercloud.io)
3. Alert administrators
4. Fall back to Phase 4 mock mode if needed

**If testnet goes down:**
1. Continue operating in Phase 4 mode
2. Queue blockchain operations
3. Resume when testnet recovers
4. Re-verify all queued operations

**If critical bug found:**
1. Immediate rollback to Phase 4
2. Fix bug in isolated environment
3. Comprehensive re-testing
4. Staged re-deployment

---

## Deployment Plan

### Stage 1: Development (Week 1-4)
- Develop all services
- Unit and integration testing
- Local testnet integration

### Stage 2: Staging (Week 5)
- Deploy to staging environment
- Alpha testing with 5-10 users
- Load testing
- Bug fixes

### Stage 3: Soft Launch (Post-Week 5)
- Deploy to production (testnet mode)
- Invite 50 alpha testers
- Monitor operations closely
- Gather feedback

### Stage 4: Full Launch (Phase 6)
- Address alpha feedback
- Security audit
- Mainnet preparation
- Public launch

---

## Future Enhancements (Post-Phase 5)

### Phase 6: Production Hardening
- User authentication system
- Rate limiting
- DDoS protection
- Security audit
- Mainnet deployment

### Phase 7: Advanced Features
- Watchtower services
- Submarine swaps
- Multi-hop routing
- Cross-chain bridges

### Phase 8: Mobile & UX
- Mobile applications
- Browser extensions
- Simplified onboarding
- One-click channels

### Phase 9: Enterprise
- High-availability setup
- Geographic redundancy
- Institutional custody
- Compliance features

---

## Conclusion

Phase 5 represents a major milestone: **real blockchain integration**. By maintaining Phase 4's performance while adding testnet settlement, we create a robust foundation for future mainnet deployment.

**Key Achievement:** Users get the best of both worlds:
- ⚡ Fast off-chain payments (Phase 4)
- 🔗 Secure on-chain settlement (Phase 5)

**Next Steps:**
1. Review this implementation plan
2. Begin Week 1: Blockchain Monitor Service
3. Follow test-driven development approach
4. Iterate based on testnet feedback

**Timeline:** 5 weeks to completion  
**Confidence:** High (building on solid Phase 4 foundation)  
**Risk Level:** Medium (testnet environment, no real funds)

---

**Document Status:** ✅ Ready for Implementation  
**Last Updated:** November 14, 2025  
**Version:** 1.0time TIMESTAMPTZ,
    first_seen TIMESTAMPTZ DEFAULT NOW(),
    confirmed_at TIMESTAMPTZ,
    raw_tx TEXT, -- Full transaction hex
    INDEX idx_blockchain_txid (txid),
    INDEX idx_blockchain_status (status),
    INDEX idx_blockchain_type (tx_type)
);

-- Track confirmation updates
CREATE TABLE confirmation_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    txid VARCHAR(64) NOT NULL,
    old_confirmations INT NOT NULL,
    new_confirmations INT NOT NULL,
    block_height INT,
    detected_at TIMESTAMPTZ DEFAULT NOW(),
    INDEX idx_confirmation_txid (txid)
);
```

**Deliverables:**
```rust
// Monitoring system
pub struct TransactionMonitor {
    db: Pool<Postgres>,
    woc_client: WhatsOnChainClient,
    watched_addresses: Arc<RwLock<HashSet<String>>>,
}

impl TransactionMonitor {
    pub async fn start_polling(&self) {
        // Poll every 10 seconds
        loop {
            self.check_all_addresses().await;
            self.update_confirmations().await;
            sleep(Duration::from_secs(10)).await;
        }
    }
}
```

#### Day 5-7: Integration & Testing

**Tasks:**
- [ ] Integrate with existing deposit service
- [ ] Add endpoints for channel service
- [ ] Write comprehensive tests
- [ ] Performance optimization
- [ ] Documentation

**Tests:**
```bash
# test-blockchain-monitor.sh
- Health check works
- Can query testnet transaction
- Can track address UTXOs
- Can watch new addresses
- Confirmation updates work
- Handles API rate limits
- Cache reduces redundant calls
```

**Week 1 Success Criteria:**
- ✅ Service runs on port 8084
- ✅ Can query any testnet transaction
- ✅ Tracks confirmation counts in real-time
- ✅ Watches addresses and detects new transactions
- ✅ 95%+ test coverage

---

### Week 2: Transaction Builder Service (Nov 20-27)

**Goal:** Can construct and sign valid BSV transactions for all use cases.

#### Day 8-10: Core Transaction Building

**Tasks:**
- [ ] Create new Rust project: `services/transaction-builder`
- [ ] Implement Bitcoin script building
- [ ] Add P2PKH transaction support
- [ ] Implement UTXO selection algorithms
- [ ] Add fee estimation logic

**Key Structures:**
```rust
// transaction-builder/src/types.rs

pub struct TransactionBuilder {
    inputs: Vec<TxInput>,
    outputs: Vec<TxOutput>,
    version: u32,
    locktime: u32,
}

pub struct TxInput {
    pub txid: String,
    pub vout: u32,
    pub script_sig: Vec<u8>,
    pub sequence: u32,
    pub amount: u64, // For signature calculation
}

pub struct TxOutput {
    pub amount: u64,
    pub script_pubkey: Vec<u8>,
}

// UTXO selection strategies
pub enum UtxoStrategy {
    SmallestFirst,    // Minimize change
    LargestFirst,     // Minimize inputs
    BranchAndBound,   // Optimal selection
}
```

**API Endpoints:**
```
POST /tx/build/p2pkh              → Build basic transaction
POST /tx/build/multisig           → Build multisig transaction
POST /tx/estimate-fee             → Estimate transaction fee
POST /tx/select-utxos             → Select optimal UTXOs
POST /tx/decode                   → Decode transaction hex
POST /tx/validate                 → Validate transaction
GET  /tx/{txid}/info              → Transaction info
```

#### Day 11-13: Channel-Specific Transactions

**Tasks:**
- [ ] Implement 2-of-2 multisig addresses
- [ ] Build channel funding transactions
- [ ] Build commitment transactions (with timelocks)
- [ ] Build settlement transactions
- [ ] Add signature handling

**Channel Transaction Types:**

```rust
// 1. Funding Transaction
pub async fn build_funding_tx(
    party_a_input: TxInput,
    party_b_input: TxInput,
    multisig_output: TxOutput,
    fee_per_byte: u64,
) -> Result<Transaction> {
    // Inputs: Party A's UTXO, Party B's UTXO
    // Output: 2-of-2 multisig address
    // Both parties must sign
}

// 2. Commitment Transaction
pub async fn build_commitment_tx(
    funding_txid: String,
    funding_vout: u32,
    party_a_balance: u64,
    party_b_balance: u64,
    sequence_number: u32,
    timelock_blocks: u32,
) -> Result<Transaction> {
    // Input: Funding TX output
    // Outputs:
    //   - Party A: balance with timelock
    //   - Party B: balance immediate
}

// 3. Settlement Transaction (Cooperative Close)
pub async fn build_settlement_tx(
    funding_txid: String,
    funding_vout: u32,
    party_a_final: u64,
    party_b_final: u64,
) -> Result<Transaction> {
    // Input: Funding TX output
    // Outputs: Final balances to both parties
    // Both must sign
}
```

**Database Tables:**
```sql
-- Store transaction templates
CREATE TABLE transaction_templates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    template_type VARCHAR(50) NOT NULL, -- 'funding', 'commitment', 'settlement'
    channel_id UUID REFERENCES payment_channels(id),
    tx_hex TEXT NOT NULL,
    txid VARCHAR(64),
    status VARCHAR(20) DEFAULT 'unsigned', -- 'unsigned', 'partial', 'signed', 'broadcast'
    party_a_signed BOOLEAN DEFAULT false,
    party_b_signed BOOLEAN DEFAULT false,
    sequence_number INT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    broadcast_at TIMESTAMPTZ,
    INDEX idx_tx_templates_channel (channel_id),
    INDEX idx_tx_templates_type (template_type)
);

-- Signature tracking
CREATE TABLE transaction_signatures (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tx_template_id UUID REFERENCES transaction_templates(id),
    paymail VARCHAR(255) NOT NULL,
    signature TEXT NOT NULL,
    pubkey TEXT NOT NULL,
    signed_at TIMESTAMPTZ DEFAULT NOW()
);
```

#### Day 14: Testing & Integration

**Tasks:**
- [ ] Test all transaction types
- [ ] Verify signature validation
- [ ] Test with real testnet faucet coins
- [ ] Integration tests with blockchain monitor
- [ ] Performance benchmarks

**Week 2 Success Criteria:**
- ✅ Can build all transaction types
- ✅ Proper fee estimation
- ✅ Valid signatures
- ✅ Transactions accepted by testnet
- ✅ 90%+ test coverage

---

### Week 3: SPV Verification Service (Nov 27-Dec 4)

**Goal:** Can verify any transaction without running a full node.

#### Day 15-17: Merkle Proof Verification

**Tasks:**
- [ ] Create new Rust project: `services/spv-service`
- [ ] Implement Merkle tree verification
- [ ] Add block header validation
- [ ] Build header chain storage
- [ ] Implement SPV proof checking

**Core SPV Logic:**
```rust
// spv-service/src/verification.rs

pub struct MerkleProof {
    pub tx_hash: String,
    pub block_hash: String,
    pub merkle_root: String,
    pub siblings: Vec<String>, // Sibling hashes in Merkle tree
    pub index: u32,            // Transaction position in block
}

pub async fn verify_merkle_proof(proof: &MerkleProof) -> Result<bool> {
    let mut hash = proof.tx_hash.clone();
    
    // Climb Merkle tree
    for (i, sibling) in proof.siblings.iter().enumerate() {
        hash = if proof.index & (1 << i) == 0 {
            hash_pair(&hash, sibling)
        } else {
            hash_pair(sibling, &hash)
        };
    }
    
    Ok(hash == proof.merkle_root)
}

pub struct BlockHeader {
    pub version: u32,
    pub prev_block: String,
    pub merkle_root: String,
    pub timestamp: u32,
    pub bits: u32,
    pub nonce: u32,
}

pub fn validate_header(header: &BlockHeader, prev_header: &BlockHeader) -> Result<bool> {
    // 1. Check hash meets difficulty target
    // 2. Verify prev_block matches
    // 3. Validate timestamp
    // 4. Check difficulty adjustment
    Ok(true)
}
```

**Database Tables:**
```sql
-- Store block headers for SPV
CREATE TABLE block_headers (
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
    created_at TIMESTAMPTZ DEFAULT NOW(),
    INDEX idx_block_hash (hash),
    INDEX idx_block_prev (prev_block)
);

-- Store Merkle proofs
CREATE TABLE merkle_proofs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    txid VARCHAR(64) NOT NULL,
    block_hash VARCHAR(64) NOT NULL,
    block_height INT,
    merkle_root VARCHAR(64) NOT NULL,
    siblings JSONB NOT NULL, -- Array of sibling hashes
    tx_index INT NOT NULL,
    verified BOOLEAN DEFAULT false,
    verified_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    INDEX idx_merkle_txid (txid),
    INDEX idx_merkle_block (block_hash)
);
```

#### Day 18-20: Advanced Verification

**Tasks:**
- [ ] Implement reorganization detection
- [ ] Add double-spend monitoring
- [ ] Build confirmation threshold logic
- [ ] Create verification reports
- [ ] Add fraud proof generation

**API Endpoints:**
```
POST /verify/tx                      → Verify transaction inclusion
POST /verify/merkle-proof            → Verify Merkle proof
GET  /verify/{txid}/confirmations    → Get confirmation count
GET  /verify/{txid}/proof            → Get Merkle proof
GET  /chain/headers/{height}         → Get block header
GET  /chain/validate/{from}/{to}     → Validate header chain
GET  /chain/reorgs                   → Recent reorganizations
POST /verify/double-spend            → Check for double-spend
```

**Week 3 Success Criteria:**
- ✅ Can verify any testnet transaction
- ✅ Merkle proof validation works
- ✅ Detects reorganizations
- ✅ Tracks confirmation counts accurately
- ✅ 90%+ test coverage

---

### Week 4: Channel Integration (Dec 4-11)

**Goal:** Payment channels use real testnet transactions for funding/settlement.

#### Day 21-23: Enhanced Channel Creation

**Tasks:**
- [ ] Update channel service to use transaction builder
- [ ] Implement real funding transaction flow
- [ ] Add blockchain monitor integration
- [ ] Update channel status based on confirmations
- [ ] Maintain backwards compatibility with Phase 4

**New Channel Flow:**
```rust
// Enhanced channel creation
pub async fn create_channel_with_blockchain(
    party_a: ChannelParty,
    party_b: ChannelParty,
    use_blockchain: bool, // If false, use Phase 4 mock mode
) -> Result<Channel> {
    
    if !use_blockchain {
        // Phase 4 mode: Generate mock TXID
        return create_mock_channel(party_a, party_b).await;
    }
    
    // Phase 5 mode: Real blockchain
    
    // Step 1: Generate 2-of-2 multisig address
    let multisig = transaction_builder
        .create_multisig_address(&party_a.pubkey, &party_b.pubkey)
        .await?;
    
    // Step 2: Build funding transaction
    let funding_tx = transaction_builder
        .build_funding_tx(
            party_a.utxo,
            party_b.utxo,
            multisig.address,
            party_a.amount + party_b.amount,
        )
        .await?;
    
    // Step 3: Both parties sign
    let signed_tx = sign_by_both_parties(funding_tx).await?;
    
    // Step 4: Broadcast to testnet
    let txid = blockchain_monitor
        .broadcast_transaction(signed_tx)
        .await?;
    
    // Step 5: Create commitment transactions
    let commitment_tx = transaction_builder
        .build_commitment_tx(
            &txid,
            0,
            party_a.amount,
            party_b.amount,
            1, // Initial sequence
            144, // 1 day timelock
        )
        .await?;
    
    // Step 6: Both sign commitment before funding confirms
    let signed_commitment = sign_by_both_parties(commitment_tx).await?;
    
    // Step 7: Wait for funding confirmation
    blockchain_monitor
        .watch_transaction(&txid)
        .await?;
    
    // Step 8: Create channel record
    let channel = Channel {
        id: Uuid::new_v4(),
        party_a: party_a.paymail,
        party_b: party_b.paymail,
        balance_a: party_a.amount,
        balance_b: party_b.amount,
        funding_txid: txid,
        funding_address: multisig.address,
        commitment_tx: signed_commitment,
        sequence_number: 1,
        status: ChannelStatus::Funding, // Waiting for confirmation
        blockchain_enabled: true,
        created_at: Utc::now(),
    };
    
    Ok(channel)
}
```

#### Day 24-26: Settlement Integration

**Tasks:**
- [ ] Implement cooperative channel closure with real TX
- [ ] Add force-close mechanism with commitment broadcast
- [ ] Integrate SPV verification for settlements
- [ ] Update channel status based on confirmations
- [ ] Add settlement proof generation

**Settlement Flow:**
```rust
// Cooperative close
pub async fn close_channel_cooperative(
    channel_id: Uuid,
) -> Result<SettlementResult> {
    let channel = get_channel(channel_id).await?;
    
    // Build settlement transaction
    let settlement_tx = transaction_builder
        .build_settlement_tx(
            &channel.funding_txid,
            0,
            channel.balance_a,
            channel.balance_b,
        )
        .await?;
    
    // Both parties sign
    let signed_tx = sign_by_both_parties(settlement_tx).await?;
    
    // Broadcast to testnet
    let txid = blockchain_monitor
        .broadcast_transaction(signed_tx)
        .await?;
    
    // Update channel status
    update_channel_status(
        channel_id,
        ChannelStatus::Closing,
        Some(txid.clone()),
    ).await?;
    
    // Wait for confirmation
    let confirmations = blockchain_monitor
        .wait_for_confirmations(&txid, 1, Duration::from_secs(600))
        .await?;
    
    // Verify with SPV
    let proof = spv_service
        .get_merkle_proof(&txid)
        .await?;
    
    let verified = spv_service
        .verify_proof(&proof)
        .await?;
    
    if verified {
        update_channel_status(
            channel_id,
            ChannelStatus::Closed,
            Some(txid.clone()),
        ).await?;
    }
    
    Ok(SettlementResult {
        txid,
        confirmations,
        spv_verified: verified,
    })
}
```

**Database Schema Updates:**
```sql
-- Enhanced payment channels table
ALTER TABLE payment_channels ADD COLUMN blockchain_enabled BOOLEAN DEFAULT false;
ALTER TABLE payment_channels ADD COLUMN funding_address VARCHAR(255);
ALTER TABLE payment_channels ADD COLUMN funding_vout INT DEFAULT 0;
ALTER TABLE payment_channels ADD COLUMN settlement_txid VARCHAR(64);
ALTER TABLE payment_channels ADD COLUMN funding_confirmations INT DEFAULT 0;
ALTER TABLE payment_channels ADD COLUMN settlement_confirmations INT DEFAULT 0;
ALTER TABLE payment_channels ADD COLUMN spv_verified BOOLEAN DEFAULT false;

-- Track channel lifecycle on blockchain
CREATE TABLE channel_blockchain_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    channel_id UUID REFERENCES payment_channels(id),
    event_type VARCHAR(50) NOT NULL, -- 'funding_broadcast', 'funding_confirmed', 'settlement_broadcast', etc.
    txid VARCHAR(64),
    confirmations INT,
    block_height INT,
    event_time TIMESTAMPTZ DEFAULT NOW(),
    INDEX idx_channel_events_channel (channel_id),
    INDEX idx_channel_events_type (event_type)
);
```

#### Day 27: Integration Testing

**Tasks:**
- [ ] End-to-end channel lifecycle test with real testnet
- [ ] Test cooperative close
- [ ] Test force close
- [ ] Performance comparison: mock vs blockchain
- [ ] Documentation updates

**Week 4 Success Criteria:**
- ✅ Channels can use real testnet funding
- ✅ Phase 4 mock mode still works (backwards compatible)
- ✅ Cooperative close works with real settlement TX
- ✅ Force close works with commitment TX broadcast
- ✅ SPV verification confirms all settlements
- ✅ Performance: Payments still ~10ms (off-chain unchanged)

---

### Week 5: Testing, Polish & Documentation (Dec 11-18)

**Goal:** Production-ready Phase 5 with comprehensive testing and documentation.

#### Day 28-30: Comprehensive Testing

**Tasks:**
- [ ] Run complete test suite (95+ tests)
- [ ] Performance benchmarking
- [ ] Load testing (100 concurrent channels)
- [ ] Edge case testing
- [ ] Failure recovery testing

**Test Categories:**

1. **Unit Tests** (per service)
   - Blockchain Monitor: 30 tests
   - Transaction Builder: 40 tests
   - SPV Service: 25 tests
   - Enhanced Channels: 35 tests

2. **Integration Tests**
   - Service-to-service communication: 20 tests
   - Database consistency: 15 tests
   - Blockchain integration: 25 tests

3. **End-to-End Tests**
   - Full channel lifecycle: 10 tests
   - Multi-channel scenarios: 5 tests
   - Failure recovery: 10 tests

4. **Performance Tests**
   - Off-chain payment latency: Target 10ms
   - On-chain confirmation time: Target 10 minutes
   - Concurrent channel capacity: Target 100+

#### Day 31-32: Bug Fixes & Optimization

**Tasks:**
- [ ] Fix all critical bugs
- [ ] Optimize database queries
- [ ] Reduce API call frequency
- [ ] Improve error handling
- [ ] Add retry logic for network failures

#### Day 33-34: Documentation

**Tasks:**
- [ ] Update all API documentation
- [ ] Write integration guide
- [ ] Create testnet setup guide
- [ ] Document migration from Phase 4 to Phase 5
- [ ] Add troubleshooting guide

**Documentation Deliverables:**
- `PHASE5_API_REFERENCE.md` - All endpoint documentation
- `TESTNET_SETUP_GUIDE.md` - Getting started with testnet
- `MIGRATION_GUIDE.md` - Upgrading from Phase 4
- `TROUBLESHOOTING.md` - Common issues and solutions
- `ARCHITECTURE.md` - Updated system architecture

#### Day 35: Final Testing & Launch

**Tasks:**
- [ ] Run final test suite
- [ ] Deploy to testnet
- [ ] Invite alpha testers
- [ ] Monitor operations
- [ ] Celebrate! 🎉

**Week 5 Success Criteria:**
- ✅ All 95+ tests pass
- ✅ No critical bugs
- ✅ Performance targets met
- ✅ Documentation complete
- ✅ Ready for alpha testers

---

## Service Specifications

### 1. Blockchain Monitor Service (Port 8084)

**Purpose:** Interface with BSV testnet via WhatsOnChain API.

**Tech Stack:**
- Rust + Actix-web
- PostgreSQL for caching
- Tokio for async operations

**Key Features:**
- Transaction monitoring
- Address watching
- Confirmation tracking
- Webhook notifications
- API rate limit handling

**Performance Targets:**
- API response time: < 100ms (cached)
- Polling interval: 10 seconds
- Cache hit rate: > 80%

---

### 2. Transaction Builder Service (Port 8085)

**Purpose:** Construct and sign all transaction types.

**Tech Stack:**
- Rust + Actix-web
- bitcoin-sv crate for TX building
- secp256k1 for signatures

**Key Features:**
- P2PKH transactions
- Multisig (2-of-2)
- Channel transactions
- Fee estimation
- UTXO selection

**Performance Targets:**
- TX building time: < 50ms
- Fee estimation accuracy: ±5%

---

### 3. SPV Verification Service (Port 8086)

**Purpose:** Verify transactions without full node.

**Tech Stack:**
- Rust + Actix-web
- PostgreSQL for header storage
- Merkle tree verification

**Key Features:**
- Merkle proof verification
- Block header validation
- Chain validation
- Reorg detection
- Double-spend monitoring

**Performance Targets:**
- Verification time: < 100ms
- Header sync: < 1 second
- Reorg detection: < 30 seconds

---

## Database Schema

### Complete Schema Additions

```sql
-- ============================================================================
-- PHASE 5: Blockchain Integration Schema
-- ============================================================================

-- Track watched addresses for monitoring
CREATE TABLE watched_addresses (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    address VARCHAR(255) NOT NULL UNIQUE,
    paymail VARCHAR(255) NOT NULL,
    purpose VARCHAR(50) NOT NULL,
    derivation_path VARCHAR(100),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    last_checked TIMESTAMPTZ,
    last_activity TIMESTAMPTZ,
    INDEX idx_watched_addresses_paymail (paymail),
    INDEX idx_watched_addresses_purpose (purpose)
);

-- Cache blockchain transaction data
CREATE TABLE blockchain_transactions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    txid VARCHAR(64) NOT NULL UNIQUE,
    tx_type VARCHAR(20) NOT NULL,
    from_address VARCHAR(255),
    to_address VARCHAR(255),
    amount_satoshis BIGINT NOT NULL,
    fee_satoshis BIGINT,
    confirmations INT DEFAULT 0,
    status VARCHAR(20) DEFAULT 'pending',
    block_hash VARCHAR(64),
    block_height INT,
    block_