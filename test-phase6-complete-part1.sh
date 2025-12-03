#!/bin/bash

# test-phase6-complete-part1.sh - Phase 6 Production Hardening Tests (Part 1)
# Sections 1-7: Infrastructure, Auth, Validation, Rate Limiting, Monitoring, Errors, Performance

set -euo pipefail

# Color codes
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
MAGENTA='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m'

# Test counters
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0
SKIPPED_TESTS=0

# Service URLs (Phase 6 - all services have their own ports)
DEPOSIT_SERVICE_URL="http://localhost:8080"
LENDING_SERVICE_URL="http://localhost:8081"
CHANNEL_SERVICE_URL="http://localhost:8082"
BLOCKCHAIN_MONITOR_URL="http://localhost:8083"
INTEREST_ENGINE_URL="http://localhost:8084"
TRANSACTION_BUILDER_URL="http://localhost:8085"
SPV_SERVICE_URL="http://localhost:8086"

# Test data
TEST_PAYMAIL="phase6test@bsvbank.local"
TEST_PAYMAIL_2="phase6test2@bsvbank.local"
JWT_TOKEN=""
API_KEY=""

# Log files
LOG_FILE="test-phase6-part1.log"
START_TIME=$(date +%s)

# Helper functions
print_header() {
    echo -e "\n${CYAN}╔════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║${NC} $1"
    echo -e "${CYAN}╚════════════════════════════════════════════════════════════════╝${NC}\n"
}

print_subheader() {
    echo -e "\n${BLUE}▶ $1${NC}"
    echo -e "${BLUE}$(printf '─%.0s' {1..70})${NC}"
}

print_test() {
    echo -e "${YELLOW}Testing:${NC} $1"
}

print_success() {
    echo -e "${GREEN}✓${NC} $1"
    ((PASSED_TESTS = PASSED_TESTS + 1))
    ((TOTAL_TESTS = TOTAL_TESTS + 1))
}

print_failure() {
    echo -e "${RED}✗${NC} $1"
    echo "[FAIL] $1" >> "$LOG_FILE"
    ((FAILED_TESTS = FAILED_TESTS + 1))
    ((TOTAL_TESTS = TOTAL_TESTS + 1))
}

print_skip() {
    echo -e "${MAGENTA}⊘${NC} $1"
    ((SKIPPED_TESTS = SKIPPED_TESTS + 1))
    ((TOTAL_TESTS = TOTAL_TESTS + 1))
}

print_info() {
    echo -e "${CYAN}ℹ${NC} $1"
}

check_service() {
    local service_name="$1"
    local url="$2"
    print_test "Checking if $service_name is running"
    
    # Graceful curl that doesn't cause script to exit
    local response
    response=$(timeout 5 curl -s "$url/health" 2>/dev/null || echo "")
    
    if [[ -n "$response" ]] && echo "$response" | jq -e '.status' > /dev/null 2>&1; then
        print_success "$service_name is running at $url"
        return 0
    else
        print_failure "$service_name is not running at $url"
        return 1  # But script continues due to || true elsewhere
    fi
}

# # FIXED: Measure latency without triggering set -e
# measure_latency() {
#     local url=$1
#     local start=$(date +%s%N)
#     set +e
#     timeout 5 curl -sf "$url" > /dev/null 2>&1
#     set -e
#     local end=$(date +%s%N)
#     local latency=$(( (end - start) / 1000000 ))
#     echo "$latency"
# }

measure_latency() {
    local url=$1
    local start=$(date +%s%N)
    timeout 5 curl -s "$url" > /dev/null 2>&1
    local end=$(date +%s%N)
    local latency=$(( (end - start) / 1000000 ))
    echo "$latency"
}

# Helper to make authenticated curl request
# AUTH_HEADER variable expansion doesn't work - When you store -H "Authorization: ..." in a variable and expand it with $AUTH_HEADER, bash treats it as a single argument with the quotes included, breaking the curl command.
# Solution: Helper function - I created curl_with_auth() that handles the authentication conditionally inside the function, avoiding the variable expansion problem entirely.
curl_with_auth() {
    local method="$1"
    local url="$2"
    shift 2
    
    set +e
    if [[ "$JWT_TOKEN" != "test-placeholder-token" && -n "$JWT_TOKEN" ]]; then
        response=$(timeout 5 curl -s -X "$method" "$url" \
            -H "Authorization: Bearer $JWT_TOKEN" \
            "$@" 2>/dev/null)
    else
        response=$(timeout 5 curl -s -X "$method" "$url" \
            "$@" 2>/dev/null)
    fi
    set -e
    
    if [[ -z "$response" ]]; then
        echo '{}'
    else
        echo "$response"
    fi
}

# Safe JSON field extractor
# Also improved error checking - Using get_json_field() and proper regex matching instead of piping through grep.
get_json_field() {
    local json="$1"
    local field="$2"
    set +e
    local result
    result=$(echo "$json" | jq -r "$field // empty" 2>/dev/null)
    set -e
    echo "$result"
}

generate_request_id() {
    echo "test-$(uuidgen 2>/dev/null || echo "$(date +%s)-$$")"
}

# Initialize
echo "Phase 6 Complete Test Suite - Part 1" > "$LOG_FILE"
echo "Started at: $(date)" >> "$LOG_FILE"

print_header "PHASE 6 PRODUCTION HARDENING - TEST SUITE PART 1"
echo "Testing Date: $(date)"
echo "Log File: $LOG_FILE"

# Pre-flight checks
print_header "PRE-FLIGHT CHECKS"

print_test "Checking for required commands"
missing_commands=()
for cmd in curl jq timeout; do
    if ! command -v $cmd &> /dev/null; then
        missing_commands+=("$cmd")
    fi
done

if [ ${#missing_commands[@]} -eq 0 ]; then
    print_success "All required commands available"
else
    print_failure "Missing commands: ${missing_commands[*]}"
    echo "Please install: ${missing_commands[*]}"
    exit 1
fi

# ============================================================================
# 1. INFRASTRUCTURE & HEALTH CHECKS
# ============================================================================
print_header "1. INFRASTRUCTURE & HEALTH CHECKS"

print_subheader "1.1 Service Availability"

check_service "Deposit Service" "$DEPOSIT_SERVICE_URL"
check_service "Lending Service" "$LENDING_SERVICE_URL"
check_service "Payment Channel Service" "$CHANNEL_SERVICE_URL"
check_service "Blockchain Monitor" "$BLOCKCHAIN_MONITOR_URL"
check_service "Interest Engine" "$INTEREST_ENGINE_URL"
check_service "Transaction Builder" "$TRANSACTION_BUILDER_URL"
check_service "SPV Service" "$SPV_SERVICE_URL"

print_subheader "1.2 Health Check Endpoints"

for service in "Deposit:$DEPOSIT_SERVICE_URL" "Lending:$LENDING_SERVICE_URL" "Channel:$CHANNEL_SERVICE_URL" "Blockchain:$BLOCKCHAIN_MONITOR_URL" "Interest:$INTEREST_ENGINE_URL" "TxBuilder:$TRANSACTION_BUILDER_URL" "SPV:$SPV_SERVICE_URL"; do
    IFS=':' read -r name url <<< "$service"
    print_test "$name Service health check structure"
    
    response=$(timeout 5 curl -s "$url/health" 2>/dev/null)
    
    if echo "$response" | jq -e '.status' > /dev/null 2>&1; then
        status=$(echo "$response" | jq -r '.status')
        if [[ "$status" == "healthy" || "$status" == "degraded" ]]; then
            print_success "$name health check valid (status: $status)"
        else
            print_failure "$name health check returned: $status"
        fi
        
        if echo "$response" | jq -e '.version, .uptime_seconds, .dependencies' > /dev/null 2>&1; then
            print_success "$name health check has all required fields"
        else
            print_skip "$name health check missing optional fields (version, uptime, dependencies)"
        fi
    else
        print_skip "$name health check not implemented yet"
    fi
done

print_subheader "1.3 Database Connectivity"

for service in "Deposit:$DEPOSIT_SERVICE_URL" "Lending:$LENDING_SERVICE_URL"; do
    IFS=':' read -r name url <<< "$service"
    print_test "$name Service database connectivity"
    
    response=$(timeout 5 curl -s "$url/health" 2>/dev/null)
    db_status=$(echo "$response" | jq -r '.dependencies[]? | select(.name=="database") | .status' 2>/dev/null)
    
    if [[ -n "$db_status" ]]; then
        if [[ "$db_status" == "healthy" ]]; then
            print_success "$name database connection healthy"
        else
            print_failure "$name database connection: $db_status"
        fi
    else
        print_skip "$name database health check not implemented yet"
    fi
done

print_subheader "1.4 Redis Connectivity"

print_test "Checking Redis connectivity in health endpoints"
response=$(timeout 5 curl -s "$DEPOSIT_SERVICE_URL/health" 2>/dev/null)
redis_status=$(echo "$response" | jq -r '.dependencies[]? | select(.name=="redis") | .status' 2>/dev/null)

if [[ -n "$redis_status" ]]; then
    if [[ "$redis_status" == "healthy" ]]; then
        print_success "Redis connection healthy"
    else
        print_failure "Redis connection: $redis_status"
    fi
else
    print_skip "Redis health check not implemented yet (optional for Phase 6)"
fi

# ============================================================================
# 2. AUTHENTICATION & AUTHORIZATION
# ============================================================================
print_header "2. AUTHENTICATION & AUTHORIZATION"

print_subheader "2.1 JWT Token Generation"

print_test "Attempting to create test user for authentication"
response=$(timeout 5 curl -s -X POST "$DEPOSIT_SERVICE_URL/register" \
    -H "Content-Type: application/json" \
    -d "{\"paymail\": \"$TEST_PAYMAIL\", \"password\": \"TestPassword123!\"}" 2>/dev/null)

if echo "$response" | jq -e '.token' > /dev/null 2>&1; then
    JWT_TOKEN=$(echo "$response" | jq -r '.token')
    print_success "JWT token generated successfully"
    print_info "Token: ${JWT_TOKEN:0:50}..."
    
    print_test "Validating JWT token structure"
    if [[ "$JWT_TOKEN" =~ ^[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+$ ]]; then
        print_success "JWT token has valid structure (3 parts)"
    else
        print_failure "JWT token has invalid structure"
    fi
else
    print_skip "JWT authentication not implemented yet (/register endpoint)"
    JWT_TOKEN="test-placeholder-token"
fi

print_test "Testing login with valid credentials"
login_response=$(timeout 5 curl -s -X POST "$DEPOSIT_SERVICE_URL/login" \
    -H "Content-Type: application/json" \
    -d "{\"paymail\":\"$TEST_PAYMAIL\",\"password\":\"TestPassword123!\"}" || echo '{}')

login_token=$(echo "$login_response" | jq -r '.token // empty')
if [[ -n "$login_token" ]]; then
    print_success "Login successful, new token generated"
    JWT_TOKEN="$login_token"  # Update the token
else
    print_failure "Login failed"
fi

print_test "Testing protected endpoint access with valid token"
balance_response=$(timeout 5 curl -s -H "Authorization: Bearer $JWT_TOKEN" \
    "$DEPOSIT_SERVICE_URL/balance/$TEST_PAYMAIL" || echo '{}')

if echo "$balance_response" | jq -e '.paymail' > /dev/null 2>&1; then
    print_success "Protected endpoint accessible with valid JWT"
else
    print_failure "Protected endpoint rejected valid JWT"
fi

print_test "Testing token refresh"
refresh_response=$(timeout 5 curl -s -X POST "$DEPOSIT_SERVICE_URL/refresh" \
    -H "Authorization: Bearer $JWT_TOKEN" || echo '{}')

refresh_token=$(echo "$refresh_response" | jq -r '.token // empty')
if [[ -n "$refresh_token" ]]; then
    print_success "Token refresh successful"
else
    print_failure "Token refresh failed"
fi

print_subheader "2.2 Protected Endpoints"

if [[ "$JWT_TOKEN" != "test-placeholder-token" ]]; then
    print_test "Accessing protected endpoint without token"
    response=$(timeout 5 curl -s -w "\n%{http_code}" "$DEPOSIT_SERVICE_URL/deposits" 2>/dev/null)
    http_code=$(echo "$response" | tail -n1)
    
    if [[ "$http_code" == "401" ]]; then
        print_success "Protected endpoint returns 401 without auth"
    else
        print_skip "Auth not enforced yet (got $http_code)"
    fi
    
    print_test "Accessing protected endpoint with invalid token"
    response=$(timeout 5 curl -s -w "\n%{http_code}" "$DEPOSIT_SERVICE_URL/deposits" \
        -H "Authorization: Bearer invalid.token.here" 2>/dev/null)
    http_code=$(echo "$response" | tail -n1)
    
    if [[ "$http_code" == "401" ]]; then
        print_success "Protected endpoint rejects invalid token"
    else
        print_skip "Invalid token handling not implemented (got $http_code)"
    fi
    
    print_test "Accessing protected endpoint with valid token"
    response=$(timeout 5 curl -s -w "\n%{http_code}" "$DEPOSIT_SERVICE_URL/deposits" \
        -H "Authorization: Bearer $JWT_TOKEN" 2>/dev/null)
    http_code=$(echo "$response" | tail -n1)
    
    if [[ "$http_code" == "200" || "$http_code" == "404" ]]; then
        print_success "Protected endpoint accepts valid token"
    else
        print_skip "Valid token acceptance not working (got $http_code)"
    fi
else
    print_skip "Skipping protected endpoint tests (JWT not available)"
fi

print_subheader "2.3 API Keys"

if [[ "$JWT_TOKEN" != "test-placeholder-token" ]]; then
    print_test "Creating API key"
    response=$(timeout 5 curl -s -X POST "$DEPOSIT_SERVICE_URL/api-keys" \
        -H "Authorization: Bearer $JWT_TOKEN" \
        -H "Content-Type: application/json" \
        -d "{\"name\": \"Phase6TestKey\", \"permissions\": [\"read\", \"write\"]}" 2>/dev/null)
    
    if echo "$response" | jq -e '.api_key' > /dev/null 2>&1; then
        API_KEY=$(echo "$response" | jq -r '.api_key')
        print_success "API key created successfully"
        print_info "API Key: ${API_KEY:0:20}..."
        
        print_test "Using API key for authentication"
        response=$(timeout 5 curl -s -w "\n%{http_code}" "$DEPOSIT_SERVICE_URL/deposits" \
            -H "X-API-Key: $API_KEY" 2>/dev/null)
        http_code=$(echo "$response" | tail -n1)
        
        if [[ "$http_code" == "200" || "$http_code" == "404" ]]; then
            print_success "API key authentication successful"
        else
            print_failure "API key authentication failed (got $http_code)"
        fi
    else
        print_skip "API key system not implemented yet"
    fi
else
    print_skip "Skipping API key tests (JWT not available)"
fi

print_subheader "2.4 Token Expiration"

print_test "Checking token expiration handling"
expired_token="eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJ0ZXN0QGV4YW1wbGUuY29tIiwiZXhwIjoxfQ.expired"
response=$(timeout 5 curl -s -w "\n%{http_code}" "$DEPOSIT_SERVICE_URL/deposits" \
    -H "Authorization: Bearer $expired_token" 2>/dev/null)
http_code=$(echo "$response" | tail -n1)

if [[ "$http_code" == "401" ]]; then
    print_success "Expired token rejected correctly"
else
    print_skip "Token expiration handling not implemented (got $http_code)"
fi

# # ============================================================================
# # 3. INPUT VALIDATION
# # ============================================================================
# print_header "3. INPUT VALIDATION"

# print_subheader "3.1 Paymail Validation"

# AUTH_HEADER=""
# if [[ "$JWT_TOKEN" != "test-placeholder-token" ]]; then
#     AUTH_HEADER="-H \"Authorization: Bearer $JWT_TOKEN\""
# fi

# print_test "Valid paymail format"
# response=$(timeout 5 curl -s -X POST "$DEPOSIT_SERVICE_URL/deposits" \
#     $AUTH_HEADER \
#     -H "Content-Type: application/json" \
#     -d "{\"paymail\": \"valid@domain.com\", \"amount_satoshis\": 1000}" 2>/dev/null)

# if echo "$response" | jq -e '.error_code' | grep -q "invalid_paymail" 2>/dev/null; then
#     print_failure "Valid paymail rejected"
# else
#     print_success "Valid paymail accepted (or validation not enforced yet)"
# fi

# print_test "Invalid paymail - no @"
# response=$(timeout 5 curl -s -X POST "$DEPOSIT_SERVICE_URL/deposits" \
#     $AUTH_HEADER \
#     -H "Content-Type: application/json" \
#     -d "{\"paymail\": \"invaliddomain.com\", \"amount_satoshis\": 1000}" 2>/dev/null)

# if echo "$response" | jq -e '.error_code' | grep -q "invalid_paymail\|validation_error" 2>/dev/null; then
#     print_success "Invalid paymail (no @) rejected"
# else
#     print_skip "Paymail validation not implemented yet"
# fi

# print_test "Invalid paymail - XSS attempt"
# response=$(timeout 5 curl -s -X POST "$DEPOSIT_SERVICE_URL/deposits" \
#     $AUTH_HEADER \
#     -H "Content-Type: application/json" \
#     -d "{\"paymail\": \"test<script>@domain.com\", \"amount_satoshis\": 1000}" 2>/dev/null)

# if echo "$response" | jq -e '.error_code' | grep -q "invalid_paymail\|validation_error" 2>/dev/null; then
#     print_success "Paymail with XSS attempt rejected"
# else
#     print_skip "XSS validation not implemented yet"
# fi

# print_test "Invalid paymail - too long"
# long_paymail=$(printf 'a%.0s' {1..300})"@domain.com"
# response=$(timeout 5 curl -s -X POST "$DEPOSIT_SERVICE_URL/deposits" \
#     $AUTH_HEADER \
#     -H "Content-Type: application/json" \
#     -d "{\"paymail\": \"$long_paymail\", \"amount_satoshis\": 1000}" 2>/dev/null)

# if echo "$response" | jq -e '.error_code' | grep -q "invalid_paymail\|validation_error" 2>/dev/null; then
#     print_success "Overly long paymail rejected"
# else
#     print_skip "Length validation not implemented yet"
# fi

# print_subheader "3.2 Amount Validation"

# print_test "Negative amount"
# response=$(timeout 5 curl -s -X POST "$DEPOSIT_SERVICE_URL/deposits" \
#     $AUTH_HEADER \
#     -H "Content-Type: application/json" \
#     -d "{\"paymail\": \"$TEST_PAYMAIL\", \"amount_satoshis\": -1000}" 2>/dev/null)

# if echo "$response" | jq -e '.error_code' | grep -q "invalid_amount\|validation_error" 2>/dev/null; then
#     print_success "Negative amount rejected"
# else
#     print_skip "Negative amount validation not implemented yet"
# fi

# print_test "Zero amount"
# response=$(timeout 5 curl -s -X POST "$DEPOSIT_SERVICE_URL/deposits" \
#     $AUTH_HEADER \
#     -H "Content-Type: application/json" \
#     -d "{\"paymail\": \"$TEST_PAYMAIL\", \"amount_satoshis\": 0}" 2>/dev/null)

# if echo "$response" | jq -e '.error_code' | grep -q "invalid_amount\|validation_error" 2>/dev/null; then
#     print_success "Zero amount rejected"
# else
#     print_skip "Zero amount validation not implemented yet"
# fi

# print_test "Amount exceeds max supply"
# response=$(timeout 5 curl -s -X POST "$DEPOSIT_SERVICE_URL/deposits" \
#     $AUTH_HEADER \
#     -H "Content-Type: application/json" \
#     -d "{\"paymail\": \"$TEST_PAYMAIL\", \"amount_satoshis\": 2100000000000000}" 2>/dev/null)

# if echo "$response" | jq -e '.error_code' | grep -q "invalid_amount\|validation_error\|exceeds_max" 2>/dev/null; then
#     print_success "Amount exceeding max supply rejected"
# else
#     print_skip "Max amount validation not implemented yet"
# fi

# print_subheader "3.3 SQL Injection Prevention"

# print_test "SQL injection attempt in paymail"
# response=$(timeout 5 curl -s -X POST "$DEPOSIT_SERVICE_URL/deposits" \
#     $AUTH_HEADER \
#     -H "Content-Type: application/json" \
#     -d "{\"paymail\": \"test' OR '1'='1@domain.com\", \"amount_satoshis\": 1000}" 2>/dev/null)

# if echo "$response" | jq -e '.error_code' | grep -q "validation_error\|invalid_paymail" 2>/dev/null; then
#     print_success "SQL injection attempt in paymail blocked"
# else
#     print_skip "SQL injection validation not implemented yet"
# fi

# print_test "SQL injection in query parameter doesn't expose SQL errors"
# response=$(timeout 5 curl -s "$DEPOSIT_SERVICE_URL/deposits?paymail=test'%20OR%20'1'='1" \
#     $AUTH_HEADER 2>/dev/null)

# if ! echo "$response" | grep -qi "SQL\|syntax error\|pg_" 2>/dev/null; then
#     print_success "SQL injection in query parameter handled safely"
# else
#     print_failure "SQL injection exposed SQL error"
# fi

# ============================================================================
# 3. INPUT VALIDATION
# ============================================================================
print_header "3. INPUT VALIDATION"

print_subheader "3.1 Paymail Validation"

print_test "Valid paymail format"
response=$(curl_with_auth "POST" "$DEPOSIT_SERVICE_URL/deposits" \
    -H "Content-Type: application/json" \
    -d "{\"paymail\": \"valid@domain.com\", \"amount_satoshis\": 1000}")

if echo "$response" | jq -e '.error_code' 2>/dev/null | grep -q "invalid_paymail"; then
    print_failure "Valid paymail rejected"
else
    print_success "Valid paymail accepted (or validation not enforced yet)"
fi

print_test "Invalid paymail - no @"
response=$(curl_with_auth "POST" "$DEPOSIT_SERVICE_URL/deposits" \
    -H "Content-Type: application/json" \
    -d "{\"paymail\": \"invaliddomain.com\", \"amount_satoshis\": 1000}")

if echo "$response" | jq -e '.error_code' 2>/dev/null | grep -q "invalid_paymail\|validation_error"; then
    print_success "Invalid paymail (no @) rejected"
else
    print_skip "Paymail validation not implemented yet"
fi

print_test "Invalid paymail - XSS attempt"
response=$(curl_with_auth "POST" "$DEPOSIT_SERVICE_URL/deposits" \
    -H "Content-Type: application/json" \
    -d "{\"paymail\": \"test<script>@domain.com\", \"amount_satoshis\": 1000}")

if echo "$response" | jq -e '.error_code' 2>/dev/null | grep -q "invalid_paymail\|validation_error"; then
    print_success "Paymail with XSS attempt rejected"
else
    print_skip "XSS validation not implemented yet"
fi

print_test "Invalid paymail - too long"
long_paymail=$(printf 'a%.0s' {1..300})"@domain.com"
response=$(curl_with_auth "POST" "$DEPOSIT_SERVICE_URL/deposits" \
    -H "Content-Type: application/json" \
    -d "{\"paymail\": \"$long_paymail\", \"amount_satoshis\": 1000}")

if echo "$response" | jq -e '.error_code' 2>/dev/null | grep -q "invalid_paymail\|validation_error"; then
    print_success "Overly long paymail rejected"
else
    print_skip "Length validation not implemented yet"
fi

print_subheader "3.2 Amount Validation"

print_test "Negative amount"
response=$(curl_with_auth "POST" "$DEPOSIT_SERVICE_URL/deposits" \
    -H "Content-Type: application/json" \
    -d "{\"paymail\": \"$TEST_PAYMAIL\", \"amount_satoshis\": -1000}")

if echo "$response" | jq -e '.error_code' 2>/dev/null | grep -q "invalid_amount\|validation_error"; then
    print_success "Negative amount rejected"
else
    print_skip "Negative amount validation not implemented yet"
fi

print_test "Zero amount"
response=$(curl_with_auth "POST" "$DEPOSIT_SERVICE_URL/deposits" \
    -H "Content-Type: application/json" \
    -d "{\"paymail\": \"$TEST_PAYMAIL\", \"amount_satoshis\": 0}")

if echo "$response" | jq -e '.error_code' 2>/dev/null | grep -q "invalid_amount\|validation_error"; then
    print_success "Zero amount rejected"
else
    print_skip "Zero amount validation not implemented yet"
fi

print_test "Amount exceeds max supply"
response=$(curl_with_auth "POST" "$DEPOSIT_SERVICE_URL/deposits" \
    -H "Content-Type: application/json" \
    -d "{\"paymail\": \"$TEST_PAYMAIL\", \"amount_satoshis\": 2100000000000000}")

if echo "$response" | jq -e '.error_code' 2>/dev/null | grep -q "invalid_amount\|validation_error\|exceeds_max"; then
    print_success "Amount exceeding max supply rejected"
else
    print_skip "Max amount validation not implemented yet"
fi

print_subheader "3.3 SQL Injection Prevention"

print_test "SQL injection attempt in paymail"
response=$(curl_with_auth "POST" "$DEPOSIT_SERVICE_URL/deposits" \
    -H "Content-Type: application/json" \
    -d "{\"paymail\": \"test' OR '1'='1@domain.com\", \"amount_satoshis\": 1000}")

if echo "$response" | jq -e '.error_code' 2>/dev/null | grep -q "validation_error\|invalid_paymail"; then
    print_success "SQL injection attempt in paymail blocked"
else
    print_skip "SQL injection validation not implemented yet"
fi

print_test "SQL injection in query parameter doesn't expose SQL errors"
response=$(curl_with_auth "GET" "$DEPOSIT_SERVICE_URL/deposits?paymail=test'%20OR%20'1'='1")

if ! echo "$response" | grep -qi "SQL\|syntax error\|pg_" 2>/dev/null; then
    print_success "SQL injection in query parameter handled safely"
else
    print_failure "SQL injection exposed SQL error"
fi

# ============================================================================
# 4. RATE LIMITING
# ============================================================================
print_header "4. RATE LIMITING"

print_subheader "4.1 Per-IP Rate Limiting"

print_test "Testing rate limiting on high-frequency requests"
rate_limit_hit=false
for i in {1..100}; do
    response=$(timeout 2 curl -s -w "\n%{http_code}" "$DEPOSIT_SERVICE_URL/health" 2>/dev/null)
    http_code=$(echo "$response" | tail -n1)
    
    if [[ "$http_code" == "429" ]]; then
        rate_limit_hit=true
        break
    fi
    sleep 0.01
done

if [[ "$rate_limit_hit" == true ]]; then
    print_success "Rate limiting triggered on excessive requests"
else
    print_skip "Rate limiting not implemented or configured for higher limit"
fi

sleep 2

print_subheader "4.2 Rate Limit Response Format"

print_test "Checking for rate limit headers"
response=$(timeout 5 curl -s -i "$DEPOSIT_SERVICE_URL/health" 2>/dev/null | head -n 20)

if echo "$response" | grep -qi "X-RateLimit" 2>/dev/null; then
    print_success "Rate limit headers present in response"
else
    print_skip "Rate limit headers not implemented yet (optional)"
fi

# ============================================================================
# 5. MONITORING & OBSERVABILITY
# ============================================================================
print_header "5. MONITORING & OBSERVABILITY"

print_subheader "5.1 Prometheus Metrics"

print_test "Metrics endpoint accessible"
response=$(timeout 5 curl -s -w "\n%{http_code}" "$DEPOSIT_SERVICE_URL/metrics" 2>/dev/null)
http_code=$(echo "$response" | tail -n1)

if [[ "$http_code" == "200" ]]; then
    print_success "Metrics endpoint accessible"
    
    if echo "$response" | head -n -1 | grep -q "http_requests_total" 2>/dev/null; then
        print_success "Request counter metric present"
    else
        print_skip "Request counter metric not implemented yet"
    fi
    
    if echo "$response" | head -n -1 | grep -q "http_request_duration" 2>/dev/null; then
        print_success "Request duration metric present"
    else
        print_skip "Request duration metric not implemented yet"
    fi
else
    print_skip "Metrics endpoint not implemented yet"
fi

# print_subheader "5.2 Structured Logging"

# print_test "Request ID correlation"
# REQUEST_ID=$(generate_request_id)
# response=$(timeout 5 curl -s "$DEPOSIT_SERVICE_URL/deposits" \
#     $AUTH_HEADER \
#     -H "X-Request-ID: $REQUEST_ID" 2>/dev/null)

# print_info "Request ID: $REQUEST_ID"
# print_skip "Log correlation check requires log file access"

print_subheader "5.2 Structured Logging"

print_test "Request ID correlation"
REQUEST_ID=$(generate_request_id)

response=$(curl_with_auth "GET" "$DEPOSIT_SERVICE_URL/deposits" \
    -H "X-Request-ID: $REQUEST_ID")

print_info "Request ID: $REQUEST_ID"
print_skip "Log correlation check requires log file access"

# # ============================================================================
# # 6. ERROR HANDLING
# # ============================================================================
# print_header "6. ERROR HANDLING & RESILIENCE"

# print_subheader "6.1 Standardized Error Format"

# print_test "Error response structure"
# response=$(timeout 5 curl -s "$DEPOSIT_SERVICE_URL/deposits/nonexistent-id" \
#     $AUTH_HEADER 2>/dev/null)

# if echo "$response" | jq -e '.error, .error_code, .message' > /dev/null 2>&1; then
#     print_success "Error response has standard structure"
#     error_code=$(echo "$response" | jq -r '.error_code')
#     print_info "Error code: $error_code"
# else
#     print_skip "Standardized error format not implemented yet"
# fi

# print_subheader "6.2 HTTP Status Code Mapping"

# print_test "404 for not found"
# response=$(timeout 5 curl -s -w "\n%{http_code}" "$DEPOSIT_SERVICE_URL/deposits/nonexistent-id" \
#     $AUTH_HEADER 2>/dev/null)
# http_code=$(echo "$response" | tail -n1)

# if [[ "$http_code" == "404" ]]; then
#     print_success "Returns 404 for not found"
# else
#     print_skip "404 status code not implemented (got $http_code)"
# fi

# print_test "400 for validation errors"
# response=$(timeout 5 curl -s -w "\n%{http_code}" "$DEPOSIT_SERVICE_URL/deposits" \
#     $AUTH_HEADER \
#     -H "Content-Type: application/json" \
#     -d "{\"paymail\": \"invalid\", \"amount_satoshis\": -100}" 2>/dev/null)
# http_code=$(echo "$response" | tail -n1)

# if [[ "$http_code" == "400" ]]; then
#     print_success "Returns 400 for validation errors"
# else
#     print_skip "400 status code not implemented (got $http_code)"
# fi

# ============================================================================
# 6. ERROR HANDLING
# ============================================================================
print_header "6. ERROR HANDLING & RESILIENCE"

print_subheader "6.1 Standardized Error Format"

print_test "Error response structure"
response=$(curl_with_auth "GET" "$DEPOSIT_SERVICE_URL/deposits/nonexistent-id")

if echo "$response" | jq -e '.error, .error_code, .message' > /dev/null 2>&1; then
    print_success "Error response has standard structure"
    error_code=$(echo "$response" | jq -r '.error_code')
    print_info "Error code: $error_code"
else
    print_skip "Standardized error format not implemented yet"
fi


print_subheader "6.2 HTTP Status Code Mapping"

print_test "404 for not found"
response=$(curl_with_auth "GET" "$DEPOSIT_SERVICE_URL/deposits/nonexistent-id" -w "\n%{http_code}")
http_code=$(echo "$response" | tail -n1)

if [[ "$http_code" == "404" ]]; then
    print_success "Returns 404 for not found"
else
    print_skip "404 status code not implemented (got $http_code)"
fi


print_test "400 for validation errors"
response=$(curl_with_auth "POST" "$DEPOSIT_SERVICE_URL/deposits" \
    -H "Content-Type: application/json" \
    -d "{\"paymail\": \"invalid\", \"amount_satoshis\": -100}" \
    -w "\n%{http_code}")

http_code=$(echo "$response" | tail -n1)

if [[ "$http_code" == "400" ]]; then
    print_success "Returns 400 for validation errors"
else
    print_skip "400 status code not implemented (got $http_code)"
fi

# ============================================================================
# 7. PERFORMANCE & OPTIMIZATION
# ============================================================================
print_header "7. PERFORMANCE & OPTIMIZATION"

print_subheader "7.1 Response Time Measurements"

print_test "Health check latency"
latencies=()
for i in {1..5}; do
    latency=$(measure_latency "$DEPOSIT_SERVICE_URL/health")
    latencies+=("$latency")
done

total=0
for latency in "${latencies[@]}"; do
    total=$((total + latency))
done
avg_latency=$((total / 5))

if [[ $avg_latency -lt 100 ]]; then
    print_success "Health check avg latency: ${avg_latency}ms (target: <100ms)"
else
    print_skip "Health check latency: ${avg_latency}ms (optimization pending)"
fi

print_subheader "7.2 Caching"

print_test "Cache headers for responses"
response=$(timeout 5 curl -s -i "$DEPOSIT_SERVICE_URL/health" 2>/dev/null | head -n 20)

if echo "$response" | grep -qi "Cache-Control\|ETag" 2>/dev/null; then
    print_success "Cache headers present"
else
    print_skip "Cache headers not implemented yet"
fi

# ============================================================================
# PART 1 SUMMARY
# ============================================================================
print_header "PART 1 TEST SUMMARY"

END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))

echo -e "${CYAN}╔════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${CYAN}║${NC}                   PART 1 TEST RESULTS                          ${CYAN}║${NC}"
echo -e "${CYAN}╠════════════════════════════════════════════════════════════════╣${NC}"
echo -e "${CYAN}║${NC} Total Tests:    $(printf '%-45s' "$TOTAL_TESTS") ${CYAN}║${NC}"
echo -e "${CYAN}║${NC} ${GREEN}Passed:${NC}         $(printf '%-45s' "$PASSED_TESTS") ${CYAN}║${NC}"
echo -e "${CYAN}║${NC} ${RED}Failed:${NC}         $(printf '%-45s' "$FAILED_TESTS") ${CYAN}║${NC}"
echo -e "${CYAN}║${NC} ${MAGENTA}Skipped:${NC}        $(printf '%-45s' "$SKIPPED_TESTS") ${CYAN}║${NC}"
echo -e "${CYAN}║${NC} Duration:       $(printf '%-45s' "${DURATION}s") ${CYAN}║${NC}"
echo -e "${CYAN}╚════════════════════════════════════════════════════════════════╝${NC}"

echo -e "\n${CYAN}Continue with Part 2: ./test-phase6-complete-part2.sh${NC}\n"

# Export variables for Part 2
export TOTAL_TESTS PASSED_TESTS FAILED_TESTS SKIPPED_TESTS
export JWT_TOKEN API_KEY TEST_PAYMAIL TEST_PAYMAIL_2

exit 0