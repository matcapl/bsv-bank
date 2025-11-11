# Phase 4 Implementation Guide

**Complete step-by-step instructions to build the Payment Channel Service**

---

## 📋 What You Have Ready

✅ Enhanced database migration (`003_payment_channels.sql`)  
✅ Service skeleton (`main.rs`)  
✅ Dependencies (`Cargo.toml`)  
✅ Setup script (`setup-phase4.sh`)  
✅ Test suite (`test-phase4-complete.sh`)  
✅ Technical specification  

---

## 🚀 Step-by-Step Implementation

### Step 1: Create Directory Structure

```bash
cd ~/repo/bsv-bank

# Create service directory
mkdir -p core/payment-channel-service/src

# Create logs directory if it doesn't exist
mkdir -p logs
```

### Step 2: Save the Files

**A. Update Database Migration**

Replace your `db/migrations/003_payments_schema.sql` with the enhanced version:

```bash
# Save the enhanced migration from the artifact:
# "003_payment_channels.sql - Enhanced Migration"
# Save to: db/migrations/003_payment_channels.sql
```

**B. Create Cargo.toml**

```bash
# Save this to: core/payment-channel-service/Cargo.toml
# Use the artifact: "Cargo.toml - Payment Channel Service Dependencies"
```

**C. Create main.rs**

```bash
# Save this to: core/payment-channel-service/src/main.rs
# Use the artifact: "main.rs - Payment Channel Service Skeleton"
```

**D. Create setup script**

```bash
# Save this to: setup-phase4.sh
# Use the artifact: "setup-phase4.sh - Setup and Build Script"

# Make it executable
chmod +x setup-phase4.sh
```

**E. Test script (already created earlier)**

```bash
# Save this to: test-phase4-complete.sh
# Use the artifact from earlier: "test-phase4-complete.sh"

# Make it executable  
chmod +x test-phase4-complete.sh
```

### Step 3: Run Database Migration

```bash
# Option A: Run migration directly
PGPASSWORD="" psql -h localhost -U a -d bsv_bank -f db/migrations/003_payment_channels.sql

# Option B: Use setup script (does this automatically)
./setup-phase4.sh
```

**Verify migration:**
```bash
psql -h localhost -U a -d bsv_bank -c "\dt"
```

You should see:
- `payment_channels`
- `channel_states`
- `channel_payments`

### Step 4: Build the Service

```bash
cd core/payment-channel-service

# Initial build (will download dependencies)
cargo build --release

# This will take 2-5 minutes on first build
```

**Expected output:**
```
   Compiling payment-channel-service v1.0.0
    Finished release [optimized] target(s) in 3m 42s
```

### Step 5: Update Start/Stop Scripts

**A. Update start-all.sh**

Add after the lending service section:

```bash
# Add this after "Starting lending-service..."
echo "Starting payment channel service..."
nohup ./core/payment-channel-service/target/release/payment-channel-service > logs/payment-channels.log 2>&1 &
PAYMENT_PID=$!
echo "  ✓ Payment channel service (PID: $PAYMENT_PID)"
```

And update the services list:

```bash
Services:
  Deposit Service:      http://localhost:8080
  Interest Engine:      http://localhost:8081
  Lending Service:      http://localhost:8082
  Payment Channels:     http://localhost:8083
  Frontend:             http://localhost:3000
```

**B. Update stop-all.sh**

Add this line:

```bash
pkill -f payment-channel-service || true
```

### Step 6: Start Services

```bash
cd ~/repo/bsv-bank

# Stop any running services
./stop-all.sh

# Start all services including new payment channel service
./start-all.sh
```

**Expected output:**
```
🏦 Starting BSV Bank - Full Stack
==================================
Starting deposit service...
  ✓ Deposit service (PID: 12345)
Starting interest engine...
  ✓ Interest engine (PID: 12346)
Starting lending-service...
  ✓ Lending service (PID: 12347)
Starting payment channel service...
  ✓ Payment channel service (PID: 12348)
✅ All services started!
```

### Step 7: Verify Service is Running

```bash
# Check health endpoint
curl http://localhost:8083/health

# Should return:
# {
#   "service": "payment-channel-service",
#   "status": "healthy",
#   "version": "1.0.0",
#   "timestamp": "2025-11-11T..."
# }
```

**Check logs:**
```bash
tail -f logs/payment-channels.log

# Should show:
# ⚡ BSV Bank - Payment Channel Service Starting...
# ✅ Database connected
# ✅ Service ready on http://0.0.0.0:8083
```

### Step 8: Run Basic Tests

```bash
# Test 1: Health check
curl http://localhost:8083/health

# Test 2: Open a channel
curl -X POST http://localhost:8083/channels/open \
  -H "Content-Type: application/json" \
  -d '{
    "party_a_paymail": "alice@test.io",
    "party_b_paymail": "bob@test.io",
    "initial_balance_a": 100000,
    "initial_balance_b": 0
  }'

# Should return channel details with channel_id

# Test 3: Send a payment (use channel_id from above)
curl -X POST http://localhost:8083/channels/CHANNEL_ID/payment \
  -H "Content-Type: application/json" \
  -d '{
    "from_paymail": "alice@test.io",
    "to_paymail": "bob@test.io",
    "amount_satoshis": 1000,
    "memo": "Test payment"
  }'

# Should return payment confirmation
```

### Step 9: Run Full Test Suite

```bash
# Run comprehensive test suite
./test-phase4-complete.sh
```

**Expected results:**
- 60+ tests will run
- Should see sections 1-18 execute
- Target: 95%+ success rate
- Performance metrics displayed at end

### Step 10: Check Database

```bash
# Connect to database
psql -h localhost -U a -d bsv_bank

# Check channels
SELECT channel_id, party_a_paymail, party_b_paymail, status, 
       current_balance_a, current_balance_b, sequence_number 
FROM payment_channels;

# Check payments
SELECT from_paymail, to_paymail, amount_satoshis, memo, created_at 
FROM channel_payments 
ORDER BY created_at DESC 
LIMIT 10;

# Exit
\q
```

---

## 🔍 Troubleshooting

### Issue: "Failed to create database pool"

**Solution:**
```bash
# Check PostgreSQL is running
docker ps | grep postgres

# If not running, start it
docker-compose up -d

# Test connection
psql -h localhost -U a -d bsv_bank -c "SELECT 1"
```

### Issue: "Migration failed"

**Solution:**
```bash
# Check if tables already exist
psql -h localhost -U a -d bsv_bank -c "\dt"

# If tables exist, you can:
# Option A: Drop and recreate (CAREFUL - loses data)
psql -h localhost -U a -d bsv_bank -c "DROP TABLE IF EXISTS channel_payments CASCADE"
psql -h localhost -U a -d bsv_bank -c "DROP TABLE IF EXISTS channel_states CASCADE"
psql -h localhost -U a -d bsv_bank -c "DROP TABLE IF EXISTS payment_channels CASCADE"

# Option B: Skip migration if already done
```

### Issue: "Address already in use (port 8083)"

**Solution:**
```bash
# Find process using port 8083
lsof -i :8083

# Kill it
kill -9 PID

# Or use stop script
./stop-all.sh
```

### Issue: Build fails with dependency errors

**Solution:**
```bash
# Update Rust
rustup update

# Clean build
cd core/payment-channel-service
cargo clean
cargo build --release
```

### Issue: Test script fails immediately

**Solution:**
```bash
# Make sure service is running
curl http://localhost:8083/health

# Check logs for errors
tail -50 logs/payment-channels.log

# Verify database connection
psql -h localhost -U a -d bsv_bank -c "SELECT COUNT(*) FROM payment_channels"
```

### Issue: "Channel not found" errors

**Solution:**
```bash
# Check if channel was created
psql -h localhost -U a -d bsv_bank -c "SELECT * FROM payment_channels"

# Try creating a new channel with the test
curl -X POST http://localhost:8083/channels/open \
  -H "Content-Type: application/json" \
  -d '{"party_a_paymail":"test@a.io","party_b_paymail":"test@b.io","initial_balance_a":100000,"initial_balance_b":0}'
```

---

## ✅ Success Checklist

Phase 4 foundation is complete when:

- [ ] Database migration ran successfully
- [ ] Service builds without errors
- [ ] Service starts and shows in logs
- [ ] Health check returns 200 OK
- [ ] Can create a channel
- [ ] Can send a payment
- [ ] Can query channel details
- [ ] Can view payment history
- [ ] Test script runs without crashes
- [ ] All services integrated in start/stop scripts

---

## 📊 What's Working Now

After completing these steps, you'll have:

### Backend ✅
- Payment channel service running on port 8083
- 8 API endpoints functional:
  - `POST /channels/open`
  - `POST /channels/{id}/payment`
  - `GET /channels/{id}`
  - `GET /channels/{id}/history`
  - `GET /channels/{id}/balance`
  - `GET /channels/user/{paymail}`
  - `POST /channels/{id}/close`
  - `GET /health`

### Database ✅
- 3 new tables with constraints
- Atomic payment processing function
- Audit trail for all state changes
- Performance indexes

### Testing ✅
- Comprehensive test suite
- 18 test sections
- 60+ test cases
- Performance benchmarking

---

## 🎯 Next Steps

After basic setup works:

### Week 1 Tasks
1. **Add more endpoints** (force-close, stats, etc.)
2. **Improve error handling**
3. **Add input validation**
4. **Performance optimization**

### Week 2 Tasks
1. **Frontend integration**
2. **Real-time updates**
3. **User interface**
4. **Documentation**

### Week 3 Tasks
1. **Advanced features**
2. **Security hardening**
3. **Production readiness**
4. **Final testing**

---

## 📞 Need Help?

If you get stuck:

1. **Check service logs:** `tail -f logs/payment-channels.log`
2. **Check database logs:** `docker logs bsv-bank-postgres-1`
3. **Verify all services:** `./start-all.sh` output
4. **Test each endpoint:** Use curl commands above
5. **Check database state:** Use psql commands above

---

## 🎉 You're Ready!

Once you complete Step 10 successfully, you have a working payment channel service foundation. The skeleton is solid and ready for the remaining features to be added incrementally.

**Let's build Phase 4! 🚀**