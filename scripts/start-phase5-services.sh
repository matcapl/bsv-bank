#!/bin/bash
# Start all Phase 5 services

set -e

PROJECT_ROOT="$(pwd)"

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${BLUE}╔═══════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║      BSV Bank - Phase 5 Service Starter                  ║${NC}"
echo -e "${BLUE}╚═══════════════════════════════════════════════════════════╝${NC}"
echo ""

# Create logs directory
mkdir -p logs

# Check if PostgreSQL is running
echo -e "${BLUE}[1/4]${NC} Checking PostgreSQL..."
if psql -d bsv_bank -c "SELECT 1" > /dev/null 2>&1; then
    echo -e "${GREEN}✓ PostgreSQL is running${NC}"
else
    echo -e "${RED}✗ PostgreSQL is not running${NC}"
    echo "Please start PostgreSQL first"
    exit 1
fi

# Function to start a service
start_service() {
    local service_name=$1
    local service_dir=$2
    local port=$3
    
    echo ""
    echo -e "${BLUE}Starting${NC} $service_name on port $port..."
    
    cd "$PROJECT_ROOT/core/$service_dir"
    
    # Start service in background
    RUST_LOG=info ./target/release/$service_dir > "$PROJECT_ROOT/logs/$service_dir.log" 2>&1 &
    echo $! > "$PROJECT_ROOT/logs/$service_dir.pid"
    
    # Wait for service to start
    sleep 2
    
    # Check if service is running
    if curl -sf "http://localhost:$port/health" > /dev/null 2>&1; then
        echo -e "${GREEN}✓ $service_name started successfully${NC}"
    else
        echo -e "${RED}✗ $service_name failed to start${NC}"
        echo "  Check logs at: $PROJECT_ROOT/logs/$service_dir.log"
        tail -20 "$PROJECT_ROOT/logs/$service_dir.log"
    fi
    
    cd "$PROJECT_ROOT"
}

# Start Phase 5 services
echo ""
echo -e "${BLUE}[2/4]${NC} Starting Blockchain Monitor..."
start_service "Blockchain Monitor" "blockchain-monitor" 8084

echo ""
echo -e "${BLUE}[3/4]${NC} Starting Transaction Builder..."
start_service "Transaction Builder" "transaction-builder" 8085

echo ""
echo -e "${BLUE}[4/4]${NC} Starting SPV Verification Service..."
start_service "SPV Verification" "spv-service" 8086

# Print status summary
echo ""
echo -e "${BLUE}╔═══════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║             Phase 5 Services Started                     ║${NC}"
echo -e "${BLUE}╚═══════════════════════════════════════════════════════════╝${NC}"
echo ""
echo "Phase 5 Services:"
echo "  • Blockchain Monitor:    http://localhost:8084"
echo "  • Transaction Builder:   http://localhost:8085"
echo "  • SPV Verification:      http://localhost:8086"
echo ""
echo "View logs:"
echo "  tail -f logs/blockchain-monitor.log"
echo "  tail -f logs/transaction-builder.log"
echo "  tail -f logs/spv-service.log"
echo ""
echo "Stop services:"
echo "  ./scripts/stop-phase5-services.sh"
echo ""
echo -e "${GREEN}✓ All Phase 5 services started successfully!${NC}"
echo ""