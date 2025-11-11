#!/bin/bash
set -e

echo "🏦 Starting BSV Bank - Full Stack"
echo "=================================="

# Check if services are already running
if lsof -Pi :8080 -sTCP:LISTEN -t >/dev/null ; then
    echo "⚠️  Deposit service already running on port 8080"
else
    echo "Starting deposit service..."
    cd core/deposit-service
    cargo run > ../../logs/deposit.log 2>&1 &
    DEPOSIT_PID=$!
    echo "  ✓ Deposit service (PID: $DEPOSIT_PID)"
    cd ../..
fi

if lsof -Pi :8081 -sTCP:LISTEN -t >/dev/null ; then
    echo "⚠️  Interest engine already running on port 8081"
else
    echo "Starting interest engine..."
    cd core/interest-engine
    cargo run > ../../logs/interest.log 2>&1 &
    INTEREST_PID=$!
    echo "  ✓ Interest engine (PID: $INTEREST_PID)"
    cd ../..
fi

if lsof -Pi :8082 -sTCP:LISTEN -t >/dev/null ; then
    echo "⚠️  Lending service already running on port 8082"
else
    echo "Starting lending-service..."
    cd core/lending-service
    cargo run > ../../logs/lending.log 2>&1 &
    LENDING_PID=$!
    echo "  ✓ Lending service (PID: $LENDING_PID)"
    cd ../..
fi

if lsof -Pi :8083 -sTCP:LISTEN -t >/dev/null ; then
    echo "⚠️  Payment channel service already running on port 8083"
else
    echo "Starting payment-channel-service..."
    cd core/payment-channel-service
    cargo run > ../../logs/payment-channels.log 2>&1 &
    PAYMENT_PID=$!
    # nohup ./core/payment-channel-service/target/release/payment-channel-service > logs/payment-channels.log 2>&1 & PAYMENT_PID=$!
    echo "  ✓ Payment channel service (PID: $PAYMENT_PID)"
fi

sleep 3

echo ""
echo "✅ All services started!"
echo ""
echo "Services:"
echo "  Deposit Service:  http://localhost:8080"
echo "  Interest Engine:  http://localhost:8081"
echo "  Lending Service:  http://localhost:8082"
echo "  Payment Channels: http://localhost:8083"
echo "  Frontend:         http://localhost:3000 (run 'cd frontend && npm start')"
echo ""
echo "Quick tests:"
echo "  curl http://localhost:8080/health"
echo "  curl http://localhost:8081/rates/current"
echo "  curl http://localhost:8082/loans/available"
echo "  curl http://localhost:8083/health"
echo ""
echo "Logs:"
echo "  tail -f logs/deposit.log"
echo "  tail -f logs/interest.log"
echo "  tail -f logs/lending.log"
echo "  tail -f logs/payment-channels.log"