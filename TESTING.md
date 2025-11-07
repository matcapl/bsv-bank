# BSV Bank - Testing Guide

## Automated Tests

Run comprehensive integration tests:
```bash
./test-wallet-integration.sh
```

## Manual UI Testing

### Test 1: Wallet Connection
1. Visit http://localhost:3000
2. Click "Connect Wallet"
3. Enter paymail: `yourname@handcash.io`
4. **Expected**: Shows "Connected" with your paymail

### Test 2: View Balance
1. After connecting, check balance cards
2. **Expected**: Shows 0 BSV if new user, or existing balance

### Test 3: Create Deposit
1. Enter amount: `0.001` BSV
2. Select lock period: `30d`
3. Click "Create Deposit"
4. **Expected**: 
   - Success message with deposit ID
   - Balance updates immediately
   - Shows "1 active deposits"

### Test 4: Multiple Deposits
1. Create another deposit with `0.002` BSV
2. **Expected**:
   - Total balance: 0.003 BSV
   - Active deposits: 2

### Test 5: Interest Accrual
1. Wait 24 hours (or simulate in DB)
2. Refresh balance
3. **Expected**: "Accrued Interest" shows non-zero value

### Test 6: Database Verification
```bash
# Check deposits
psql -d bsv_bank -c "SELECT * FROM deposits ORDER BY created_at DESC LIMIT 5;"

# Check user balances
psql -d bsv_bank -c "SELECT * FROM user_balances;"

# Check interest rates
psql -d bsv_bank -c "SELECT * FROM interest_rates ORDER BY created_at DESC LIMIT 1;"
```

## API Testing

### Create Deposit
```bash
curl -X POST http://localhost:8080/deposits \
  -H "Content-Type: application/json" \
  -d '{
    "user_paymail": "test@handcash.io",
    "amount_satoshis": 100000,
    "txid": "'$(openssl rand -hex 32)'",
    "lock_duration_days": 30
  }'
```

### Get Balance
```bash
curl http://localhost:8080/balance/test@handcash.io | jq
```

### Get Interest Rates
```bash
curl http://localhost:8081/rates/current | jq
```

## Performance Testing

### Load Test (requires k6)
```bash
k6 run - <<EOF
import http from 'k6/http';
import { check } from 'k6';

export let options = {
  stages: [
    { duration: '30s', target: 50 },
    { duration: '1m', target: 100 },
    { duration: '30s', target: 0 },
  ],
};

export default function() {
  let res = http.get('http://localhost:8080/health');
  check(res, { 'status is 200': (r) => r.status === 200 });
}
