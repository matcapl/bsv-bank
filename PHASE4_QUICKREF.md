# Phase 4 Quick Reference

**Essential commands and API examples for payment channels**

---

## 🚀 Service Management

```bash
# Build service
cd core/payment-channel-service && cargo build --release

# Start all services
./start-all.sh

# Stop all services
./stop-all.sh

# View logs
tail -f logs/payment-channels.log

# Check if running
curl http://localhost:8083/health
```

---

## 📡 API Endpoints

### 1. Health Check
```bash
curl http://localhost:8083/health
```

### 2. Open Channel
```bash
curl -X POST http://localhost:8083/channels/open \
  -H "Content-Type: application/json" \
  -d '{
    "party_a_paymail": "alice@test.io",
    "party_b_paymail": "bob@test.io",
    "initial_balance_a": 100000,
    "initial_balance_b": 0,
    "timeout_blocks": 144
  }'
```

### 3. Send Payment
```bash
curl -X POST http://localhost:8083/channels/CHANNEL_ID/payment \
  -H "Content-Type: application/json" \
  -d '{
    "from_paymail": "alice@test.io",
    "to_paymail": "bob@test.io",
    "amount_satoshis": 1000,
    "memo": "Coffee payment"
  }'
```

### 4. Get Channel Details
```bash
curl http://localhost:8083/channels/CHANNEL_ID
```

### 5. Get Payment History
```bash
curl http://localhost:8083/channels/CHANNEL_ID/history
```

### 6. Get Channel Balance
```bash
curl http://localhost:8083/channels/CHANNEL_ID/balance
```

### 7. Get User's Channels
```bash
curl http://localhost:8083/channels/user/alice@test.io
```

### 8. Close Channel
```bash
curl -X POST http://localhost:8083/channels/CHANNEL_ID/close \
  -H "Content-Type: application/json" \
  -d '{
    "party_paymail": "alice@test.io"
  }'
```

---

## 💾 Database Queries

```bash
# Connect to database
psql -h localhost -U a -d bsv_bank
```

### View All Channels
```sql
SELECT 
  channel_id,
  party_a_paymail,
  party_b_paymail,
  status,
  current_balance_a,
  current_balance_b,
  sequence_number
FROM payment_channels
ORDER BY opened_at DESC;
```

### View Recent Payments
```sql
SELECT 
  from_paymail,
  to_paymail,
  amount_satoshis,
  memo,
  created_at
FROM channel_payments
ORDER BY created_at DESC
LIMIT 20;
```

### View Channel States
```sql
SELECT 
  channel_id,
  sequence_number,
  balance_a,
  balance_b,
  created_at
FROM channel_states
WHERE channel_id = 'CHANNEL_ID'
ORDER BY sequence_number DESC;
```

### Channel Statistics
```sql
SELECT 
  COUNT(*) as total_channels,
  COUNT(*) FILTER (WHERE status = 'Active') as active_channels,
  SUM(current_balance_a + current_balance_b) as total_value_locked
FROM payment_channels;
```

### Payment Statistics
```sql
SELECT 
  COUNT(*) as total_payments,
  SUM(amount_satoshis) as total_volume,
  AVG(amount_satoshis) as avg_payment,
  MIN(amount_satoshis) as min_payment,
  MAX(amount_satoshis) as max_payment
FROM channel_payments;
```

---

## 🧪 Testing

```bash
# Run full test suite
./test-phase4-complete.sh

# Test specific endpoint
curl http://localhost:8083/health

# Performance test
time curl -X POST http://localhost:8083/channels/CHANNEL_ID/payment \
  -H "Content-Type: application/json" \
  -d '{"from_paymail":"a@test.io","to_paymail":"b@test.io","amount_satoshis":100}'
```

---

## 🔍 Debugging

### Check Service Status
```bash
# Is it running?
ps aux | grep payment-channel-service

# Check port
lsof -i :8083

# View recent logs
tail -50 logs/payment-channels.log

# Follow logs live
tail -f logs/payment-channels.log
```

### Common Issues

**Service won't start:**
```bash
# Check if port is in use
lsof -i :8083
kill -9 PID

# Check database connection
psql -h localhost -U a -d bsv_bank -c "SELECT 1"
```

**Payment fails:**
```bash
# Check channel exists
curl http://localhost:8083/channels/CHANNEL_ID

# Check balances
curl http://localhost:8083/channels/CHANNEL_ID/balance

# View in database
psql -h localhost -U a -d bsv_bank -c \
  "SELECT * FROM payment_channels WHERE channel_id = 'CHANNEL_ID'"
```

**Migration issues:**
```bash
# Check tables exist
psql -h localhost -U a -d bsv_bank -c "\dt"

# Re-run migration
psql -h localhost -U a -d bsv_bank -f db/migrations/003_payment_channels.sql
```

---

## 📊 Performance Monitoring

### Payment Latency
```sql
SELECT 
  AVG(processing_time_ms) as avg_latency,
  MIN(processing_time_ms) as min_latency,
  MAX(processing_time_ms) as max_latency,
  PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY processing_time_ms) as p95_latency
FROM channel_payments
WHERE processing_time_ms IS NOT NULL;
```

### Channel Activity
```sql
SELECT 
  channel_id,
  COUNT(*) as payment_count,
  SUM(amount_satoshis) as total_volume,
  MAX(created_at) as last_payment
FROM channel_payments
GROUP BY channel_id
ORDER BY payment_count DESC
LIMIT 10;
```

---

## 🎯 Quick Workflow Example

### Complete Payment Flow
```bash
# 1. Create channel
RESPONSE=$(curl -s -X POST http://localhost:8083/channels/open \
  -H "Content-Type: application/json" \
  -d '{"party_a_paymail":"alice@test.io","party_b_paymail":"bob@test.io","initial_balance_a":100000,"initial_balance_b":0}')

# 2. Extract channel ID
CHANNEL_ID=$(echo $RESPONSE | jq -r '.channel_id')
echo "Channel ID: $CHANNEL_ID"

# 3. Send payment
curl -X POST http://localhost:8083/channels/$CHANNEL_ID/payment \
  -H "Content-Type: application/json" \
  -d '{"from_paymail":"alice@test.io","to_paymail":"bob@test.io","amount_satoshis":5000,"memo":"Payment 1"}'

# 4. Check balance
curl http://localhost:8083/channels/$CHANNEL_ID/balance

# 5. View history
curl http://localhost:8083/channels/$CHANNEL_ID/history

# 6. Close channel
curl -X POST http://localhost:8083/channels/$CHANNEL_ID/close \
  -H "Content-Type: application/json" \
  -d '{"party_paymail":"alice@test.io"}'
```

---

## 📝 File Locations

```
bsv-bank/
├── core/payment-channel-service/
│   ├── src/
│   │   └── main.rs                    # Service code
│   ├── Cargo.toml                     # Dependencies
│   └── target/release/
│       └── payment-channel-service    # Compiled binary
├── db/migrations/
│   └── 003_payment_channels.sql       # Database schema
├── logs/
│   └── payment-channels.log           # Service logs
├── test-phase4-complete.sh            # Test suite
└── setup-phase4.sh                    # Setup script
```

---

## 🎓 Key Concepts

### Channel Lifecycle
```
Open → Active → Closing → Closed
  ↓              ↓
  └→ Disputed ───┘
```

### Balance Conservation
```
current_balance_a + current_balance_b = 
initial_balance_a + initial_balance_b
```

### Sequence Numbers
- Start at 0
- Increment by 1 per payment
- Must be monotonic (no gaps)
- Used for state verification

### Timeout Blocks
- Default: 144 blocks (~24 hours)
- Used for dispute resolution
- Prevents indefinite channel locks

---

## 🔑 Environment Variables

```bash
# .env file (if using)
DATABASE_URL="postgresql://a:@localhost:5432/bsv_bank"
SERVICE_PORT=8083
RUST_LOG=info
RUST_BACKTRACE=1
```

---

## 📚 Resources

- **Specification:** `PHASE4_SPECIFICATION.md`
- **Implementation Guide:** `PHASE4_IMPLEMENTATION_GUIDE.md`
- **Test Suite:** `test-phase4-complete.sh`
- **Setup Script:** `setup-phase4.sh`

---

**Keep this file handy for quick reference during development! 📌**