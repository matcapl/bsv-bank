#!/bin/bash
# Check status of all Phase 5 services

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

GREEN='\033[0;32m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}╔═══════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║           BSV Bank - Service Status Check                ║${NC}"
echo -e "${BLUE}╚═══════════════════════════════════════════════════════════╝${NC}"
echo ""

check_service_detailed() {
    local name=$1
    local port=$2
    
    printf "%-30s " "$name:"
    
    if response=$(curl -sf "http://localhost:$port/health" 2>/dev/null); then
        status=$(echo "$response" | jq -r '.status // "unknown"' 2>/dev/null || echo "unknown")
        version=$(echo "$response" | jq -r '.version // "unknown"' 2>/dev/null || echo "")
        
        if [ "$status" == "healthy" ]; then
            echo -e "${GREEN}✓ Running${NC} (port $port) $version"
        else
            echo -e "${RED}✗ Unhealthy${NC} (port $port)"
        fi
    else
        echo -e "${RED}✗ Not running${NC} (port $port)"
    fi
}

echo "Phase 5 Services:"
echo "─────────────────────────────────────────────────────────────"
check_service_detailed "Blockchain Monitor" 8084
check_service_detailed "Transaction Builder" 8085
check_service_detailed "SPV Verification" 8086

echo ""
echo "Phase 1-4 Services:"
echo "─────────────────────────────────────────────────────────────"
check_service_detailed "Deposit Service" 8080
check_service_detailed "Interest Engine" 8081
check_service_detailed "Lending Service" 8082
check_service_detailed "Payment Channels" 8083

echo ""
echo "Database:"
echo "─────────────────────────────────────────────────────────────"
printf "%-30s " "PostgreSQL:"
if pg_isready -h localhost -p 5432 > /dev/null 2>&1; then
    echo -e "${GREEN}✓ Running${NC}"
else
    echo -e "${RED}✗ Not running${NC}"
fi

echo ""
echo "Testnet Connectivity:"
echo "─────────────────────────────────────────────────────────────"
printf "%-30s " "WhatsOnChain API:"
if curl -sf "https://api.whatsonchain.com/v1/bsv/test/chain/info" > /dev/null 2>&1; then
    height=$(curl -sf "https://api.whatsonchain.com/v1/bsv/test/chain/info" | jq -r '.blocks' 2>/dev/null || echo "unknown")
    echo -e "${GREEN}✓ Connected${NC} (height: $height)"
else
    echo -e "${RED}✗ Cannot connect${NC}"
fi

echo ""