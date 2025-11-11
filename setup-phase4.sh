#!/bin/bash

# setup-phase4.sh
# Sets up Phase 4: Payment Channel Service

set -e

GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${BLUE}🚀 Setting up Phase 4: Payment Channels${NC}"
echo ""

# Step 1: Run database migration
echo -e "${BLUE}[1/5] Running database migration...${NC}"
if [ -f "db/migrations/003_payment_channels.sql" ]; then
    PGPASSWORD="" psql -h localhost -U a -d bsv_bank -f db/migrations/003_payment_channels.sql
    echo -e "${GREEN}✓ Database migration complete${NC}"
else
    echo -e "${YELLOW}⚠ Migration file not found at db/migrations/003_payment_channels.sql${NC}"
    exit 1
fi

echo ""

# Step 2: Create service directory structure
echo -e "${BLUE}[2/5] Creating service directory...${NC}"
mkdir -p core/payment-channel-service/src
echo -e "${GREEN}✓ Directory created${NC}"

echo ""

# Step 3: Build the service
echo -e "${BLUE}[3/5] Building payment channel service...${NC}"
cd core/payment-channel-service
cargo build --release
cd ../..
echo -e "${GREEN}✓ Build complete${NC}"

echo ""

# Step 4: Update start-all.sh
echo -e "${BLUE}[4/5] Updating startup scripts...${NC}"

if ! grep -q "payment-channel-service" start-all.sh; then
    # Backup original
    cp start-all.sh start-all.sh.backup
    
    # Add payment channel service to startup
    sed -i.tmp '/Starting lending-service/a\
\
echo "Starting payment channel service..."\
nohup ./core/payment-channel-service/target/release/payment-channel-service > logs/payment-channels.log 2>&1 &\
PAYMENT_PID=$!\
echo "  ✓ Payment channel service (PID: $PAYMENT_PID)"
' start-all.sh
    
    rm start-all.sh.tmp
    
    # Update the services list
    sed -i.tmp 's/Lending Service:  http:\/\/localhost:8082/Lending Service:       http:\/\/localhost:8082\
  Payment Channels:     http:\/\/localhost:8083/' start-all.sh
    
    rm start-all.sh.tmp
    
    echo -e "${GREEN}✓ start-all.sh updated${NC}"
else
    echo -e "${YELLOW}⚠ start-all.sh already contains payment channel service${NC}"
fi

echo ""

# Step 5: Update stop-all.sh
echo -e "${BLUE}[5/5] Updating stop script...${NC}"

if ! grep -q "payment-channel-service" stop-all.sh; then
    # Backup original
    cp stop-all.sh stop-all.sh.backup
    
    # Add payment channel service to stop
    sed -i.tmp '/pkill -f lending-service/a\
pkill -f payment-channel-service || true
' stop-all.sh
    
    rm stop-all.sh.tmp
    
    echo -e "${GREEN}✓ stop-all.sh updated${NC}"
else
    echo -e "${YELLOW}⚠ stop-all.sh already contains payment channel service${NC}"
fi

echo ""
echo -e "${GREEN}╔══════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║  ✅ Phase 4 Setup Complete!             ║${NC}"
echo -e "${GREEN}╚══════════════════════════════════════════╝${NC}"
echo ""
echo -e "Next steps:"
echo -e "  1. Start services:  ${BLUE}./start-all.sh${NC}"
echo -e "  2. Test service:    ${BLUE}curl http://localhost:8083/health${NC}"
echo -e "  3. Run tests:       ${BLUE}./test-phase4-complete.sh${NC}"
echo ""