#!/bin/bash
echo "=== Quick System Check ==="
echo ""

echo "1. Backend Services:"
curl -s http://localhost:8080/health | jq || echo "❌ Deposit service down"
curl -s http://localhost:8081/health | jq || echo "❌ Interest engine down"
echo ""

echo "2. Database:"
psql -d bsv_bank -c "SELECT COUNT(*) as total_deposits FROM deposits;" 2>/dev/null || echo "❌ Database error"
echo ""

echo "3. Create Test Deposit:"
TXID=$(openssl rand -hex 32)
curl -s -X POST http://localhost:8080/deposits \
  -H "Content-Type: application/json" \
  -d "{\"user_paymail\":\"quicktest@test.io\",\"amount_satoshis\":50000,\"txid\":\"$TXID\"}" | jq
echo ""

echo "4. Check Balance:"
curl -s "http://localhost:8080/balance/quicktest@test.io" | jq
