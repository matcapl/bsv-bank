# Phase 5: BSV Testnet Integration - Quick Start Guide

**Last Updated:** November 14, 2025  
**Status:** Ready for Implementation

---

## 🎯 What is Phase 5?

Phase 5 transforms BSV Bank from **mock transactions** to **real blockchain integration** using BSV testnet. This enables:

- ✅ Real testnet transaction monitoring
- ✅ Actual on-chain channel funding and settlement
- ✅ SPV verification of all transactions
- ✅ Complete blockchain audit trail
- ⚡ While maintaining Phase 4's fast off-chain payments

---

## 📦 What's Included

### New Services (3)

1. **Blockchain Monitor** (Port 8084)
   - Monitors BSV testnet via WhatsOnChain API
   - Tracks transaction confirmations
   - Watches addresses for new transactions
   - Broadcasts transactions to network

2. **Transaction Builder** (Port 8085)
   - Builds all transaction types (P2PKH, multisig, channel TXs)
   - Estimates fees and selects UTXOs
   - Creates funding, commitment, and settlement transactions
   - Validates transaction structure

3. **SPV Verification** (Port 8086)
   - Verifies Merkle proofs
   - Validates block headers
   - Detects chain reorganizations
   - Ensures transaction finality

### Database Changes
- 16 new tables for blockchain data
- Enhanced existing tables with blockchain columns
- 3 monitoring views
- Audit logging and triggers

### Management Scripts
- `start-phase5-services.sh` - Start all services
- `stop-phase5-services.sh` - Stop all services
- `build-phase5.sh` - Build all services
- `check-phase5-status.sh` - Check service health

---

## 🚀 Installation & Setup

### Prerequisites

```bash
# System requirements
- Rust 1.70+ (cargo --version)
- PostgreSQL 15+ (psql --version)
- Internet connection (for testnet API)

# Existing BSV Bank installation
- Phase 1-4 services working
- Database: bsv_bank
```

### Step 1: Create Service Directories

```bash
cd ~/bsv-bank

# Create Phase 5 service directories
mkdir -p services/blockchain-monitor/src
mkdir -p services/transaction-builder/src
mkdir -p services/spv-service/src
mkdir -p migrations
mkdir -p logs
```

### Step 2: Copy Service Code

Copy the provided service implementations:

1. **Blockchain Monitor:**
   - Copy `blockchain_monitor_service.rs` to `services/blockchain-monitor/src/main.rs`
   - Copy `Cargo.toml` for blockchain-monitor

2. **Transaction Builder:**
   - Copy `transaction_builder_service.rs` to `services/transaction-builder/src/main.rs`
   - Copy `Cargo.toml` for transaction-builder

3. **SPV Service:**
   - Copy `spv_verification_service.rs` to `services/spv-service/src/main.rs`
   - Copy `Cargo.toml` for spv-service

### Step 3: Database Migration

```bash
# Save migration SQL
cat > migrations/phase5_schema.sql << 'EOF'
[Copy the phase5_migration.sql content here]
EOF

# Run migration
psql -h localhost -U postgres -d bsv_bank -f migrations/phase5_schema.sql
```

Expected output:
```
NOTICE:  ╔═══════════════════════════════════════════════════════════╗
NOTICE:  ║  Phase 5 Database Migration Complete                     ║
NOTICE:  ║  Tables Created:    16                                    ║
NOTICE:  ╚═══════════════════════════════════════════════════════════╝
```

### Step 4: Build Services

```bash
# Create build script
cat > scripts/build-phase5.sh << 'EOF'
[Copy build script content]
EOF

chmod +x scripts/build-phase5.sh

# Build all Phase 5 services
./scripts/build-phase5.sh
```

This will take 5-10 minutes for the first build.

### Step 5: Start Services

```bash
# Create startup script
cat > scripts/start-phase5-services.sh << 'EOF'
[Copy startup script content]
EOF

chmod +x scripts/start-phase5-services.sh

# Start all Phase 5 services
./scripts/start-phase5-services.sh
```

Expected output:
```
╔═══════════════════════════════════════════════════════════╗
║      BSV Bank - Phase 5 Service Manager                  ║
╚═══════════════════════════════════════════════════════════╝

[1/7] ✓ PostgreSQL is running
[2/7] ✓ Database migrations complete
[3/7] ✓ Blockchain Monitor started successfully
[4/7] ✓ Transaction Builder started successfully
[5/7] ✓ SPV Verification started successfully
[6/7] Checking existing services...
      ✓ Deposit Service (port 8080) is running
      ✓ Interest Engine (port 8081) is running
      ✓ Lending Service (port 8082) is running
      ✓ Channel Service (port 8083) is running

✓ All Phase 5 services started successfully!
```

---

## ✅ Verification

### Check Service Health

```bash
# Check all services
./scripts/check-phase5-status.sh
```

### Manual Health Checks

```bash
# Blockchain Monitor
curl http://localhost:8084/health
# Expected: {"status":"healthy","service":"blockchain-monitor",...}

# Transaction Builder
curl http://localhost:8085/health
# Expected: {"status":"healthy","service":"transaction-builder",...}

# SPV Verification
curl http://localhost:8086/health
# Expected: {"status":"healthy","service":"spv-verification",...}

# Test blockchain connectivity
curl http://localhost:8084/chain/info
# Expected: {"height":2450000+,"best_block_hash":"...", ...}
```

### View Logs

```bash
# Real-time log monitoring
tail -f logs/blockchain-monitor.log
tail -f logs/transaction-builder.log
tail -f logs/spv-service.log

# Search for errors
grep ERROR logs/*.log
```

---

## 🧪 Testing

### Run Complete Test Suite

```bash
# Make test script executable
chmod +x tests/phase5/test-phase5-complete.sh

# Run all 210 tests
./tests/phase5/test-phase5-complete.sh
```

Expected result:
```
╔═══════════════════════════════════════════════════════════╗
║              ✓ ALL TESTS PASSED! ✓                        ║
║         Phase 5 Implementation Complete                   ║
╚═══════════════════════════════════════════════════════════╝

Total Tests:  210
Passed:       200+
Failed:       0-5 (skipped tests require testnet funding)
Skipped:      5-10
```

### Quick Smoke Tests

```bash
# Test 1: Query testnet transaction
curl "http://localhost:8084/tx/[some-testnet-txid]"

# Test 2: Build P2PKH transaction
curl -X POST http://localhost:8085/tx/build/p2pkh \
  -H "Content-Type: application/json" \
  -d '{
    "from_address": "n1234...",
    "to_address": "n5678...",
    "amount_satoshis": 10000,
    "fee_per_byte": 50
  }'

# Test 3: Get current chain height
curl http://localhost:8086/chain/height

# Test 4: Create mock channel (Phase 4 still works!)
curl -X POST http://localhost:8083/channels/create \
  -H "Content-Type: application/json" \
  -d '{
    "party_a_paymail": "alice@test.com",
    "party_b_paymail": "bob@test.com",
    "amount_a": 100000,
    "amount_b": 100000,
    "blockchain_enabled": false
  }'
```

---

## 🔧 Configuration

### Environment Variables

Create `.env` file in project root:

```bash
# Database
DATABASE_URL=postgresql://postgres:postgres@localhost/bsv_bank

# Network
NETWORK=testnet
WOC_API_BASE=https://api.whatsonchain.com/v1/bsv/test

# Fees
FEE_PER_BYTE=50
DEFAULT_FEE_PER_BYTE=50

# Confirmations
MIN_CONFIRMATIONS=1
CHANNEL_FUNDING_CONFIRMATIONS=1
CHANNEL_SETTLEMENT_CONFIRMATIONS=1

# Monitoring
POLLING_INTERVAL=10
```

Load environment:
```bash
export $(cat .env | xargs)
```

---

## 📊 Monitoring & Operations

### Database Queries

```sql
-- Check watched addresses
SELECT * FROM watched_addresses ORDER BY created_at DESC LIMIT 10;

-- View pending transactions
SELECT * FROM pending_blockchain_transactions;

-- Check channel blockchain status
SELECT * FROM channel_blockchain_status WHERE blockchain_enabled = true;

-- View transaction verification status
SELECT * FROM transaction_verification_status ORDER BY block_height DESC LIMIT 20;

-- Check recent blockchain events
SELECT * FROM channel_blockchain_events ORDER BY event_time DESC LIMIT 50;
```

### System Health Dashboard

```bash
# Create monitoring dashboard
watch -n 5 './scripts/check-phase5-status.sh'
```

### Performance Metrics

```bash
# Check service response times
time curl -s http://localhost:8084/chain/info > /dev/null
time curl -s http://localhost:8085/health > /dev/null
time curl -s http://localhost:8086/chain/height > /dev/null

# Check database query performance
psql -d bsv_bank -c "EXPLAIN ANALYZE SELECT * FROM blockchain_transactions WHERE status = 'pending';"
```

---

## 🐛 Troubleshooting

### Service Won't Start

**Problem:** Service fails to start or crashes immediately

**Solutions:**
```bash
# Check if port is already in use
lsof -i :8084  # or 8085, 8086

# Kill existing process
kill -9 $(lsof -t -i:8084)

# Check logs for errors
cat logs/blockchain-monitor.log | grep -i error

# Rebuild service
cd services/blockchain-monitor
cargo clean
cargo build --release

# Check database connection
psql -h localhost -U postgres -d bsv_bank -c "SELECT 1"
```

### Cannot Connect to Testnet

**Problem:** WhatsOnChain API not responding

**Solutions:**
```bash
# Test direct API connection
curl https://api.whatsonchain.com/v1/bsv/test/chain/info

# Check if behind firewall/proxy
curl -v https://api.whatsonchain.com/v1/bsv/test/chain/info

# Try alternative API endpoint
export WOC_API_BASE=https://api.whatsonchain.com/v1/bsv/test
```

### Database Migration Fails

**Problem:** Migration errors or conflicts

**Solutions:**
```bash
# Check if tables already exist
psql -d bsv_bank -c "\dt"

# Drop Phase 5 tables (CAUTION: loses data)
psql -d bsv_bank -c "DROP TABLE IF EXISTS watched_addresses CASCADE;"
# ... repeat for other Phase 5 tables

# Re-run migration
psql -d bsv_bank -f migrations/phase5_schema.sql
```

### Tests Fail

**Problem:** Test suite has failures

**Solutions:**
```bash
# Run tests with verbose output
RUST_LOG=debug ./tests/phase5/test-phase5-complete.sh

# Test individual services
cd services/blockchain-monitor
cargo test --release -- --nocapture

# Check if services are running
curl http://localhost:8084/health
curl http://localhost:8085/health
curl http://localhost:8086/health

# Verify database is accessible
psql -d bsv_bank -c "SELECT COUNT(*) FROM watched_addresses;"
```

### High Memory Usage

**Problem:** Services consuming too much RAM

**Solutions:**
```bash
# Check memory usage
ps aux | grep -E '(blockchain-monitor|transaction-builder|spv-service)'

# Reduce cache size (in service code)
# Adjust tx_cache HashMap size limits

# Restart services periodically
./scripts/stop-phase5-services.sh
./scripts/start-phase5-services.sh
```

---

## 🔄 Upgrading from Phase 4

If you have existing Phase 4 data:

1. **Backup database:**
   ```bash
   pg_dump -h localhost -U postgres bsv_bank > backup_phase4.sql
   ```

2. **Run Phase 5 migration:**
   ```bash
   psql -d bsv_bank -f migrations/phase5_schema.sql
   ```

3. **Verify existing data:**
   ```sql
   -- Check existing channels still work
   SELECT COUNT(*) FROM payment_channels;
   
   -- Check deposits intact
   SELECT COUNT(*) FROM deposits;
   ```

4. **Test Phase 4 functionality:**
   ```bash
   # Create mock channel (should still work)
   curl -X POST http://localhost:8083/channels/create \
     -H "Content-Type: application/json" \
     -d '{"party_a_paymail":"test@a.com","party_b_paymail":"test@b.com","amount_a":10000,"amount_b":10000,"blockchain_enabled":false}'
   ```

---

## 🎯 Next Steps

### Phase 5 Complete Checklist

- [ ] All 3 services running (8084, 8085, 8086)
- [ ] Database migration successful
- [ ] 200+ tests passing
- [ ] Can query testnet transactions
- [ ] Can build transactions
- [ ] Can verify Merkle proofs
- [ ] Phase 4 features still work
- [ ] Logs show no errors

### Start Using Phase 5

```bash
# Watch a testnet address
curl -X POST http://localhost:8084/watch/address \
  -H "Content-Type: application/json" \
  -d '{
    "address": "n2Z...",
    "paymail": "alice@bsvbank.com",
    "purpose": "deposit"
  }'

# Monitor transactions
tail -f logs/blockchain-monitor.log | grep "New TX detected"

# Build channel funding transaction
curl -X POST http://localhost:8085/tx/build/funding \
  -H "Content-Type: application/json" \
  -d '{
    "party_a": {"address":"n1...","amount":100000},
    "party_b": {"address":"n2...","amount":100000},
    "multisig_address": "2N3...",
    "fee_per_byte": 50
  }'
```

### Proceed to Phase 6

Once Phase 5 is stable:
- Security audit
- User authentication
- Rate limiting
- Production hardening
- **Mainnet deployment**

---

## 📚 Additional Resources

- **Implementation Plan:** `PHASE5_IMPLEMENTATION_PLAN.md`
- **Test Suite:** `test-phase5-complete.sh`
- **API Documentation:** (Coming in Phase 6)
- **WhatsOnChain API Docs:** https://developers.whatsonchain.com/
- **BSV Documentation:** https://docs.bsvblockchain.org/

---

## 🆘 Getting Help

**Common Issues:**
1. Service won't start → Check logs, verify ports available
2. Cannot connect to testnet → Test API directly, check firewall
3. Tests failing → Verify all services running, check database
4. High resource usage → Adjust cache sizes, restart services

**Community Support:**
- GitHub Issues: https://github.com/matcapl/bsv-bank/issues
- Project Discussions: https://github.com/matcapl/bsv-bank/discussions

---

**Status:** ✅ Phase 5 Ready for Deployment  
**Next Milestone:** Complete testnet testing → Proceed to Phase 6

*Ready to bring real blockchain integration to BSV Bank!* 🚀