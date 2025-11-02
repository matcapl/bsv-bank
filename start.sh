#!/bin/bash
echo "Starting BSV Bank..."

# Start databases
docker-compose up -d

# Start deposit service in background
cd core/deposit-service
cargo run > ../../logs/deposit.log 2>&1 &
echo "Deposit service started (PID: $!)"

# Wait a moment
sleep 3

# Start frontend
cd ../../frontend
npm start