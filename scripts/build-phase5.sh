#!/bin/bash
# Build all Phase 5 services

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}Building Phase 5 services...${NC}"
echo ""

build_service() {
    local service_name=$1
    local service_dir=$2
    
    echo -e "${BLUE}Building${NC} $service_name..."
    cd "$PROJECT_ROOT/core/$service_dir"
    cargo build --release
    echo -e "${GREEN}✓${NC} $service_name built"
    cd "$PROJECT_ROOT"
}

build_service "Blockchain Monitor" "blockchain-monitor"
build_service "Transaction Builder" "transaction-builder"
build_service "SPV Verification Service" "spv-service"

echo ""
echo -e "${GREEN}✓ All Phase 5 services built successfully!${NC}"