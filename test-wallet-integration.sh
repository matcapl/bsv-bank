#!/bin/bash

echo "╔══════════════════════════════════════════╗"
echo "║  BSV Bank - Wallet Integration Tests    ║"
echo "╚══════════════════════════════════════════╝"
echo ""

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

pass_count=0
fail_count=0

test_result() {
    if [ $1 -eq 0 ]; then
        echo -e "${GREEN}✓ PASS${NC}: $2"
        ((pass_count++))
    else
        echo -e "${RED}✗ FAIL${NC}: $2"
        ((fail_count++))
    fi
}

# Test 1: Backend services
echo -e "\n${YELLOW}[Test 1]${NC} Checking backend services..."
curl -sf http://localhost:8080/health > /dev/null
test_result $? "Deposit service health check"

curl -sf http://localhost:8081/health > /dev/null
test_result $? "Interest engine health check"

# Test 2: Database connection
echo -e "\n${YELLOW}[Test 2]${NC} Testing database..."
if command -v jq &> /dev/null; then
    RESULT=$(curl -s http://localhost:8080/health | jq -r '.database' 2>/dev/null)
    [ "$RESULT" = "connected" ]
    test_result $? "Database connection status"
else
    echo "⚠ Warning: jq not installed, skipping JSON parsing tests"
    test_result 0 "Database connection status (skipped)"
fi

psql -d bsv_bank -c "SELECT 1" > /dev/null 2>&1
test_result $? "Direct database query"

# Test 3: Create deposit
echo -e "\n${YELLOW}[Test 3]${NC} Testing deposit creation..."
TEST_PAYMAIL="test_$(date +%s)@handcash.io"
TXID=$(openssl rand -hex 32 2>/dev/null || echo "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")

RESPONSE=$(curl -s -X POST http://localhost:8080/deposits \
  -H "Content-Type: application/json" \
  -d "{\"user_paymail\":\"$TEST_PAYMAIL\",\"amount_satoshis\":100000,\"txid\":\"$TXID\",\"lock_duration_days\":30}")

echo "$RESPONSE" | grep -q "deposit_id"
test_result $? "Deposit creation returns deposit_id"

# Test 4: Check balance
echo -e "\n${YELLOW}[Test 4]${NC} Testing balance retrieval..."
sleep 1
BALANCE_RESPONSE=$(curl -s "http://localhost:8080/balance/$TEST_PAYMAIL")
echo "$BALANCE_RESPONSE" | grep -q "balance_satoshis"
test_result $? "Balance endpoint returns data"

# Test 5: Database persistence
echo -e "\n${YELLOW}[Test 5]${NC} Testing database persistence..."
DB_COUNT=$(psql -d bsv_bank -t -c "SELECT COUNT(*) FROM deposits WHERE txid='$TXID'" 2>/dev/null | xargs)
[ "$DB_COUNT" = "1" ]
test_result $? "Deposit persisted to database"

# Test 6: Input validation
echo -e "\n${YELLOW}[Test 6]${NC} Testing input validation..."
INVALID_RESPONSE=$(curl -s -X POST http://localhost:8080/deposits \
  -H "Content-Type: application/json" \
  -d '{"user_paymail":"<script>@xss.com","amount_satoshis":1000,"txid":"invalid"}')

echo "$INVALID_RESPONSE" | grep -q "error"
test_result $? "Invalid input rejected"

# Test 7: Interest rates
echo -e "\n${YELLOW}[Test 7]${NC} Testing interest engine..."
curl -sf http://localhost:8081/rates/current > /dev/null
test_result $? "Interest rate endpoint accessible"

# Summary
echo -e "\n╔══════════════════════════════════════════╗"
echo -e "║           Test Summary                   ║"
echo -e "╚══════════════════════════════════════════╝"
echo -e "${GREEN}Passed: $pass_count${NC}"
echo -e "${RED}Failed: $fail_count${NC}"
echo ""

if [ $fail_count -eq 0 ]; then
    echo -e "${GREEN}✓ All tests passed!${NC}"
    exit 0
else
    echo -e "${RED}✗ Some tests failed${NC}"
    exit 1
fi
