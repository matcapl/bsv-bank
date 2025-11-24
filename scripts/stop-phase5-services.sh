#!/bin/bash
# Stop all Phase 5 services

PROJECT_ROOT="$(pwd)"

GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}Stopping Phase 5 services...${NC}"

stop_service() {
    local service_name=$1
    local pid_file="$PROJECT_ROOT/logs/$service_name.pid"
    
    if [ -f "$pid_file" ]; then
        local pid=$(cat "$pid_file")
        if kill -0 "$pid" 2>/dev/null; then
            kill "$pid"
            echo -e "${GREEN}✓${NC} Stopped $service_name (PID: $pid)"
        else
            echo "  $service_name was not running"
        fi
        rm "$pid_file"
    else
        echo "  No PID file for $service_name"
    fi
}

stop_service "blockchain-monitor"
stop_service "transaction-builder"
stop_service "spv-service"

echo ""
echo -e "${GREEN}✓ All Phase 5 services stopped${NC}"