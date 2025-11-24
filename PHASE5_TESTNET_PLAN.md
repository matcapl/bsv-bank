# Phase 5: BSV Testnet Integration

**Goal:** Connect BSV Bank to real Bitcoin SV testnet for safe testing with test coins

**Timeline:** 2-3 weeks  
**Status:** Week 1 - Research & Foundation  

---

## 🎯 Objectives

1. ✅ Connect to BSV testnet
2. ✅ Monitor real transactions
3. ✅ Verify deposits on-chain
4. ✅ Create real BSV transactions
5. ✅ Integrate with testnet faucet
6. ✅ Display real transaction confirmations

---

## 📋 Week 1: Foundation & Research (Nov 13-20)

### Day 1-2: Research & Setup
- [ ] Research BSV testnet APIs
- [ ] Choose blockchain integration approach:
  - Option A: WhatsOnChain API (easiest)
  - Option B: Run own testnet node (more control)
  - Option C: SPV Wallet library (most feature-rich)
- [ ] Set up testnet wallet for testing
- [ ] Get testnet coins from faucet
- [ ] Document testnet transaction flow

### Day 3-4: Transaction Monitoring Service
- [ ] Create `blockchain-monitor` service (port 8084)
- [ ] Connect to WhatsOnChain testnet API
- [ ] Implement transaction polling
- [ ] Add webhook support for new transactions
- [ ] Store transaction data in database

### Day 5-7: Deposit Verification
- [ ] Update deposit service to use real txids
- [ ] Add transaction verification endpoint
- [ ] Implement SPV proof checking (basic)
- [ ] Add confirmation count tracking
- [ ] Update frontend to show confirmations

**Deliverable:** Can verify real testnet deposits

---

## 📋 Week 2: Transaction Creation (Nov 20-27)

### Day 8-10: Wallet Integration
- [ ] Generate testnet addresses
- [ ] Create basic wallet service
- [ ] Implement UTXO tracking
- [ ] Add balance calculation
- [ ] Test with faucet coins

### Day 11-13: Transaction Building
- [ ] Build withdrawal transactions
- [ ] Implement fee calculation
- [ ] Add transaction signing (basic)
- [ ] Broadcast to testnet
- [ ] Monitor confirmation status

### Day 14: Testing & Integration
- [ ] Test complete deposit → withdraw flow
- [ ] Verify on testnet explorer
- [ ] Update frontend for real transactions
- [ ] Document testnet usage

**Deliverable:** Can send real testnet transactions

---

## 📋 Week 3: Channel Settlement (Nov 27-Dec 4)

### Day 15-17: Payment Channel Settlement
- [ ] Design channel funding transaction
- [ ] Create channel closing transaction
- [ ] Test cooperative settlement
- [ ] Verify settlement on testnet

### Day 18-20: Polish & Testing
- [ ] End-to-end testing
- [ ] Bug fixes
- [ ] Performance optimization
- [ ] Documentation

### Day 21: Launch Testnet Alpha
- [ ] Deploy to testnet
- [ ] Invite testers
- [ ] Monitor operations

**Deliverable:** Full system running on testnet

---

## 🏗️ Technical Architecture

### New Service: Blockchain Monitor

```
┌─────────────────────────────────────┐
│   Blockchain Monitor (Port 8084)    │
│                                     │
│  ┌──────────────────────────────┐  │
│  │  Transaction Verifier        │  │
│  │  - Check txid exists         │  │
│  │  - Count confirmations       │  │
│  │  - Verify outputs            │  │
│  └──────────────────────────────┘  │
│                                     │
│  ┌──────────────────────────────┐  │
│  │  Address Monitor             │  │
│  │  - Watch addresses           │  │
│  │  - Detect new txs            │  │
│  │  - Notify services           │  │
│  └──────────────────────────────┘  │
└─────────────────────────────────────┘
            ↕
    BSV Testnet Network
```

### Database Changes

```sql
-- Add to deposits table
ALTER TABLE deposits ADD COLUMN confirmations INT DEFAULT 0;
ALTER TABLE deposits ADD COLUMN testnet_verified BOOLEAN DEFAULT FALSE;

-- New table for tracking transactions
CREATE TABLE blockchain_transactions (
    id UUID PRIMARY KEY,
    txid VARCHAR(64) UNIQUE NOT NULL,
    type VARCHAR(20) NOT NULL, -- 'deposit', 'withdrawal', 'settlement'
    amount_satoshis BIGINT NOT NULL,
    from_address VARCHAR(255),
    to_address VARCHAR(255),
    confirmations INT DEFAULT 0,
    status VARCHAR(20) DEFAULT 'pending',
    broadcasted_at TIMESTAMPTZ,
    confirmed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Table for monitoring addresses
CREATE TABLE watched_addresses (
    id UUID PRIMARY KEY,
    address VARCHAR(255) UNIQUE NOT NULL,
    user_paymail VARCHAR(255) NOT NULL,
    purpose VARCHAR(50) NOT NULL, -- 'deposit', 'channel', etc
    created_at TIMESTAMPTZ DEFAULT NOW()
);
```

---

## 🔧 Implementation Approach

### Option A: WhatsOnChain API (RECOMMENDED)

**Pros:**
- ✅ No infrastructure needed
- ✅ Free for testnet
- ✅ Well-documented API
- ✅ Fast to implement

**Cons:**
- ❌ Depends on third-party service
- ❌ Rate limits

**Example Usage:**
```bash
# Get transaction
curl https://api.whatsonchain.com/v1/bsv/test/tx/hash/TXID

# Broadcast transaction
curl -X POST https://api.whatsonchain.com/v1/bsv/test/tx/raw \
  -d '{"txhex":"..."}'

# Get address balance
curl https://api.whatsonchain.com/v1/bsv/test/address/ADDRESS/balance
```

### Option B: BSV Library (bsv crate)

**Pros:**
- ✅ Full control
- ✅ No external dependencies
- ✅ Can run offline

**Cons:**
- ❌ More complex
- ❌ Longer development time

---

## 📊 Success Criteria

Phase 5 is complete when:

- [ ] Can connect to BSV testnet
- [ ] Can verify real transactions
- [ ] Can track confirmations
- [ ] Can generate addresses
- [ ] Can create transactions
- [ ] Can broadcast to network
- [ ] All existing features work with testnet
- [ ] Frontend shows testnet data
- [ ] Documentation complete
- [ ] 5+ successful test transactions

---

## 🧪 Testing Plan

### Unit Tests
- Transaction verification logic
- Address generation
- Fee calculation
- UTXO selection

### Integration Tests
- Full deposit flow on testnet
- Withdrawal flow on testnet
- Channel funding on testnet
- Channel settlement on testnet

### Manual Testing
- Use testnet faucet
- Create deposits
- Verify on explorer
- Test withdrawals
- Monitor confirmations

---

## 🔐 Security Considerations

### Testnet Safety
- ✅ Use testnet only (no real money)
- ✅ Clear warnings in UI
- ✅ Separate database tables
- ✅ Different configuration

### Private Key Management
- 🔄 Store keys securely (even on testnet)
- 🔄 Encrypt in database
- 🔄 Use environment variables
- 🔄 Never log private keys

---

## 📚 Resources

### BSV Testnet
- Faucet: https://faucet.bitcoinscaling.io/
- Explorer: https://test.whatsonchain.com/
- Docs: https://docs.bsvblockchain.org/

### APIs
- WhatsOnChain: https://developers.whatsonchain.com/
- BSV RPC: https://bitcoin-sv.github.io/

### Libraries
- bsv crate: https://crates.io/crates/bsv
- bitcoin-spv: https://crates.io/crates/bitcoin-spv

---

## 🎯 Milestones

### Milestone 1: Can Verify Transactions ✅
- Transaction monitoring service running
- Can fetch testnet transaction data
- Confirmation counting works

### Milestone 2: Can Create Addresses ✅
- Wallet service generates addresses
- Addresses shown in UI
- Can receive testnet coins

### Milestone 3: Can Send Transactions ✅
- Transaction building works
- Broadcasting successful
- Confirmations tracked

### Milestone 4: Complete Integration ✅
- All services use testnet
- Frontend updated
- End-to-end testing complete

---

## 💡 Quick Wins

Start with these for fast progress:

1. **WhatsOnChain integration** (Day 1-2)
   - Easy API calls
   - No infrastructure needed
   - Gets you 70% of the way

2. **Address generation** (Day 3)
   - Simple cryptography
   - Show addresses in UI
   - Users can receive coins

3. **Transaction verification** (Day 4-5)
   - Check if txid exists
   - Count confirmations
   - Update existing deposits

---

## 🚧 Risks & Mitigation

### Risk: WhatsOnChain API down
**Mitigation:** Cache data, implement retry logic

### Risk: Testnet congestion
**Mitigation:** Adjustable fees, longer timeouts

### Risk: Key management complexity
**Mitigation:** Start simple, iterate later

### Risk: Transaction building bugs
**Mitigation:** Extensive testing, use proven libraries

---

## 📈 Success Metrics

- Transaction verification: < 5 seconds
- Confirmation updates: Every 10 minutes
- Address generation: Instant
- Transaction broadcast: < 3 seconds
- Uptime: 99%+

---

**Ready to start Week 1?** Let's begin with WhatsOnChain API integration! 🚀