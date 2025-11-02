# BSV Bank - Quick Start

## Running Locally

1. Start database services:
```bash
   docker-compose up -d
```

2. Start deposit service:
```bash
   cd core/deposit-service
   cargo run
```

3. Start frontend (new terminal):
```bash
   cd frontend
   npm start
```

4. Open http://localhost:3000

## Testing API
```bash
# Create deposit
curl -X POST http://localhost:8080/deposits \
  -H "Content-Type: application/json" \
  -d '{"user_paymail": "test@handcash.io", "amount_satoshis": 100000, "txid": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef", "lock_duration_days": 30}'

# Check balance
curl http://localhost:8080/balance/test@handcash.io
```

## Next Steps
- Add HandCash credentials to .env
- Implement interest-engine service
- Add P2P lending functionality