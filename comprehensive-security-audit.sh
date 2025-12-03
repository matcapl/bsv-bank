#!/bin/bash

# Comprehensive Security Audit Script for BSV Bank
# Version 2.0 - Security Hardened
# Checks all files (committed and uncommitted) for security issues

set -euo pipefail

RED='\033[0;31m'
YELLOW='\033[1;33m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo "════════════════════════════════════════════════════════════"
echo "  BSV BANK - COMPREHENSIVE SECURITY AUDIT v2.0"
echo "════════════════════════════════════════════════════════════"
echo ""
echo "Auditing Date: $(date)"
echo "Working Directory: $(pwd)"
echo ""

# Create audit output directory with restricted permissions
mkdir -p audit-reports
chmod 700 audit-reports  # Only owner can access

REPORT_FILE="audit-reports/security-audit-$(date +%Y%m%d-%H%M%S).txt"
touch "$REPORT_FILE"
chmod 600 "$REPORT_FILE"  # Only owner can read/write

# Function to log findings
log_finding() {
    local severity=$1
    local category=$2
    local message=$3
    echo "[$severity] $category: $message" | tee -a "$REPORT_FILE"
}

# Function to redact secrets from output
redact_secrets() {
    sed -E 's/"[^"]{8,}"/"<REDACTED>"/g' | \
    sed -E 's/secret[^=]*=[^;]*/secret=<REDACTED>/gi' | \
    sed -E 's/password[^=]*=[^;]*/password=<REDACTED>/gi'
}

echo "════════════════════════════════════════════════════════════"
echo "1. CHECKING GIT STATUS (Committed vs Uncommitted Files)"
echo "════════════════════════════════════════════════════════════"
echo ""

# Show git status
echo "Git Status:" | tee -a "$REPORT_FILE"
git status --short 2>/dev/null | tee -a "$REPORT_FILE" || echo "Not a git repository" | tee -a "$REPORT_FILE"
echo "" | tee -a "$REPORT_FILE"

# List all tracked files
echo "All tracked source files:" | tee -a "$REPORT_FILE"
git ls-files 2>/dev/null | grep -E '\.(rs|toml|md|sh|sql)$' | tee -a "$REPORT_FILE" || echo "  No git tracked files found"
echo "" | tee -a "$REPORT_FILE"

# List untracked files
echo "Untracked source files:" | tee -a "$REPORT_FILE"
git ls-files --others --exclude-standard 2>/dev/null | grep -E '\.(rs|toml|md|sh|sql)$' | tee -a "$REPORT_FILE" || echo "  No untracked source files"
echo "" | tee -a "$REPORT_FILE"

echo "════════════════════════════════════════════════════════════"
echo "2. FINDING ALL MAIN.RS FILES"
echo "════════════════════════════════════════════════════════════"
echo ""

# Find all main.rs files using safe iteration
echo "Found main.rs files:" | tee -a "$REPORT_FILE"
find . -name "main.rs" -not -path "*/target/*" -not -path "*/.git/*" -print0 | \
    xargs -0 -I {} echo "{}" | tee -a "$REPORT_FILE"
echo "" | tee -a "$REPORT_FILE"

echo "════════════════════════════════════════════════════════════"
echo "3. CHECKING SECURITY HEADERS IN MAIN.RS FILES"
echo "════════════════════════════════════════════════════════════"
echo ""

while IFS= read -r -d '' file; do
    echo "Checking: $file" | tee -a "$REPORT_FILE"
    
    # Check for security headers
    if grep -q "X-Frame-Options" "$file" 2>/dev/null; then
        echo -e "  ${GREEN}✓${NC} Has X-Frame-Options" | tee -a "$REPORT_FILE"
    else
        log_finding "WARNING" "Security Headers" "$file missing X-Frame-Options"
    fi
    
    if grep -q "X-Content-Type-Options" "$file" 2>/dev/null; then
        echo -e "  ${GREEN}✓${NC} Has X-Content-Type-Options" | tee -a "$REPORT_FILE"
    else
        log_finding "WARNING" "Security Headers" "$file missing X-Content-Type-Options"
    fi
    
    if grep -q "Content-Security-Policy" "$file" 2>/dev/null; then
        echo -e "  ${GREEN}✓${NC} Has Content-Security-Policy" | tee -a "$REPORT_FILE"
    else
        log_finding "WARNING" "Security Headers" "$file missing Content-Security-Policy"
    fi
    
    if grep -q "Strict-Transport-Security" "$file" 2>/dev/null; then
        echo -e "  ${GREEN}✓${NC} Has HSTS" | tee -a "$REPORT_FILE"
    else
        log_finding "INFO" "Security Headers" "$file missing HSTS (optional in dev)"
    fi
    
    echo "" | tee -a "$REPORT_FILE"
done < <(find . -name "main.rs" -not -path "*/target/*" -not -path "*/.git/*" -print0)

echo "════════════════════════════════════════════════════════════"
echo "4. CHECKING FOR HARDCODED SECRETS"
echo "════════════════════════════════════════════════════════════"
echo ""

# Pattern 1: Hardcoded JWT secrets (specific patterns only)
echo "Checking for hardcoded JWT secrets..." | tee -a "$REPORT_FILE"
HARDCODED_SECRETS=$(grep -rn --include="*.rs" \
    -e '"test-secret"' \
    -e '"dev-secret-key"' \
    -e '"development-secret-change-in-production"' \
    -e 'JwtManager::new("' \
    . --exclude-dir=target --exclude-dir=.git 2>/dev/null | \
    grep -v "#\[cfg(test)\]" | \
    grep -v "^[[:space:]]*#" | \
    grep -v "^[[:space:]]*//" || true)

if [ -n "$HARDCODED_SECRETS" ]; then
    echo -e "${RED}✗ CRITICAL: Hardcoded secrets found:${NC}" | tee -a "$REPORT_FILE"
    echo "$HARDCODED_SECRETS" | redact_secrets | tee -a "$REPORT_FILE"
    log_finding "CRITICAL" "Hardcoded Secrets" "Found hardcoded secrets in code (see redacted output above)"
else
    echo -e "${GREEN}✓ No obvious hardcoded secrets${NC}" | tee -a "$REPORT_FILE"
fi
echo "" | tee -a "$REPORT_FILE"

# Pattern 2: Weak password defaults (in test code)
echo "Checking for weak password patterns in non-test code..." | tee -a "$REPORT_FILE"
WEAK_PASSWORDS=$(grep -rn --include="*.rs" \
    -e 'password.*=.*"password' \
    -e 'password.*=.*"123' \
    -e 'password.*=.*"admin' \
    -e 'password.*=.*"test' \
    . --exclude-dir=target --exclude-dir=.git 2>/dev/null | \
    grep -v "#\[cfg(test)\]" | \
    grep -v "mod tests" | \
    grep -v "^[[:space:]]*#" | \
    grep -v "^[[:space:]]*//" || true)

if [ -n "$WEAK_PASSWORDS" ]; then
    echo -e "${YELLOW}⚠ WARNING: Weak password patterns found:${NC}" | tee -a "$REPORT_FILE"
    echo "$WEAK_PASSWORDS" | redact_secrets | tee -a "$REPORT_FILE"
else
    echo -e "${GREEN}✓ No weak password patterns in production code${NC}" | tee -a "$REPORT_FILE"
fi
echo "" | tee -a "$REPORT_FILE"

echo "════════════════════════════════════════════════════════════"
echo "5. CHECKING ENVIRONMENT VARIABLE USAGE"
echo "════════════════════════════════════════════════════════════"
echo ""

echo "Checking JWT_SECRET usage..." | tee -a "$REPORT_FILE"
JWT_SECRET_USAGE=$(grep -rn --include="*.rs" "JWT_SECRET" . --exclude-dir=target --exclude-dir=.git 2>/dev/null || true)
if [ -n "$JWT_SECRET_USAGE" ]; then
    echo "$JWT_SECRET_USAGE" | tee -a "$REPORT_FILE"
    echo -e "${GREEN}✓ JWT_SECRET references found (verify they use env::var)${NC}"
    
    # Check if any don't use env::var
    BAD_SECRET_USAGE=$(echo "$JWT_SECRET_USAGE" | grep -v "env::var\|std::env::var" || true)
    if [ -n "$BAD_SECRET_USAGE" ]; then
        log_finding "WARNING" "Environment Variables" "JWT_SECRET used without env::var in some places"
    fi
else
    log_finding "WARNING" "Environment Variables" "No JWT_SECRET references found"
fi
echo "" | tee -a "$REPORT_FILE"

echo "════════════════════════════════════════════════════════════"
echo "6. AUDITING AUTH MIDDLEWARE FILES"
echo "════════════════════════════════════════════════════════════"
echo ""

# Find auth-related files using safe iteration
echo "Auth/Middleware files found:" | tee -a "$REPORT_FILE"
find . \( -name "auth.rs" -o -name "middleware.rs" \) \
    -not -path "*/target/*" -not -path "*/.git/*" -print0 | \
    xargs -0 -I {} echo "{}" | tee -a "$REPORT_FILE"
echo "" | tee -a "$REPORT_FILE"

while IFS= read -r -d '' file; do
    echo "Analyzing: $file" | tee -a "$REPORT_FILE"
    
    # Check for JWT verification
    if grep -q "verify_token\|decode_token\|DecodingKey" "$file" 2>/dev/null; then
        echo -e "  ${GREEN}✓${NC} Has token verification" | tee -a "$REPORT_FILE"
    else
        log_finding "WARNING" "Authentication" "$file may be missing token verification"
    fi
    
    # Check for proper error handling
    if grep -q "Result<" "$file" 2>/dev/null && grep -q "map_err\|?" "$file" 2>/dev/null; then
        echo -e "  ${GREEN}✓${NC} Has error handling" | tee -a "$REPORT_FILE"
    else
        log_finding "INFO" "Error Handling" "$file may need better error handling"
    fi
    
    echo "" | tee -a "$REPORT_FILE"
done < <(find . \( -name "auth.rs" -o -name "middleware.rs" \) \
    -not -path "*/target/*" -not -path "*/.git/*" -print0)

echo "════════════════════════════════════════════════════════════"
echo "7. AUDITING VALIDATION FILES"
echo "════════════════════════════════════════════════════════════"
echo ""

echo "Validation files found:" | tee -a "$REPORT_FILE"
find . -name "validation.rs" -not -path "*/target/*" -not -path "*/.git/*" -print0 | \
    xargs -0 -I {} echo "{}" | tee -a "$REPORT_FILE"
echo "" | tee -a "$REPORT_FILE"

while IFS= read -r -d '' file; do
    echo "Analyzing: $file" | tee -a "$REPORT_FILE"
    
    # Check for validation functions
    if grep -q "validate_paymail\|validate_amount\|validate_txid" "$file" 2>/dev/null; then
        echo -e "  ${GREEN}✓${NC} Has validation functions" | tee -a "$REPORT_FILE"
    else
        log_finding "WARNING" "Validation" "$file may be missing key validators"
    fi
    
    # Check for SQL injection prevention
    if grep -q "sql_injection\|sanitize" "$file" 2>/dev/null; then
        echo -e "  ${GREEN}✓${NC} Has SQL injection checks" | tee -a "$REPORT_FILE"
    else
        log_finding "INFO" "SQL Security" "$file may need SQL injection prevention"
    fi
    
    # Check for XSS prevention
    if grep -q "xss\|sanitize.*html\|strip.*tags" "$file" 2>/dev/null; then
        echo -e "  ${GREEN}✓${NC} Has XSS prevention" | tee -a "$REPORT_FILE"
    else
        log_finding "INFO" "XSS Prevention" "$file may need XSS prevention"
    fi
    
    echo "" | tee -a "$REPORT_FILE"
done < <(find . -name "validation.rs" -not -path "*/target/*" -not -path "*/.git/*" -print0)

echo "════════════════════════════════════════════════════════════"
echo "8. CHECKING PASSWORD HASHING IMPLEMENTATION"
echo "════════════════════════════════════════════════════════════"
echo ""

PASSWORD_HASH=$(grep -rn --include="*.rs" \
    -e "password_hash\|hash.*password" \
    . --exclude-dir=target --exclude-dir=.git 2>/dev/null || true)

if echo "$PASSWORD_HASH" | grep -q "sha256\|Sha256" 2>/dev/null; then
    log_finding "WARNING" "Password Security" "Using SHA256 for passwords (should upgrade to Argon2/bcrypt)"
    echo "" | tee -a "$REPORT_FILE"
fi

if echo "$PASSWORD_HASH" | grep -q "argon2\|bcrypt" 2>/dev/null; then
    echo -e "${GREEN}✓ Using secure password hashing (Argon2/bcrypt)${NC}" | tee -a "$REPORT_FILE"
    echo "" | tee -a "$REPORT_FILE"
fi

echo "════════════════════════════════════════════════════════════"
echo "9. CHECKING CORS CONFIGURATION"
echo "════════════════════════════════════════════════════════════"
echo ""

CORS_CONFIG=$(grep -rn --include="*.rs" \
    -e "Cors\|cors" \
    . --exclude-dir=target --exclude-dir=.git 2>/dev/null || true)

if [ -n "$CORS_CONFIG" ]; then
    echo "CORS configuration found:" | tee -a "$REPORT_FILE"
    echo "$CORS_CONFIG" | head -20 | tee -a "$REPORT_FILE"
    
    if echo "$CORS_CONFIG" | grep -q "allowed_origin\|allow_any_origin" 2>/dev/null; then
        echo -e "${YELLOW}⚠ Review CORS origins for production${NC}" | tee -a "$REPORT_FILE"
    fi
else
    log_finding "WARNING" "CORS" "No CORS configuration found"
fi
echo "" | tee -a "$REPORT_FILE"

echo "════════════════════════════════════════════════════════════"
echo "10. CHECKING RATE LIMITING IMPLEMENTATION"
echo "════════════════════════════════════════════════════════════"
echo ""

RATE_LIMIT_FILES=$(find . \( -name "rate_limit.rs" -o -name "rate_limiting.rs" \) \
    -not -path "*/target/*" -not -path "*/.git/*" -print0 2>/dev/null | xargs -0 -I {} echo "{}" || true)

if [ -n "$RATE_LIMIT_FILES" ]; then
    echo -e "${GREEN}✓ Rate limiting files found:${NC}" | tee -a "$REPORT_FILE"
    echo "$RATE_LIMIT_FILES" | tee -a "$REPORT_FILE"
    
    while IFS= read -r -d '' file; do
        if grep -q "check_rate_limit\|RateLimiter" "$file" 2>/dev/null; then
            echo -e "  ${GREEN}✓${NC} $file has rate limiting logic" | tee -a "$REPORT_FILE"
        fi
    done < <(find . \( -name "rate_limit.rs" -o -name "rate_limiting.rs" \) \
        -not -path "*/target/*" -not -path "*/.git/*" -print0)
else
    log_finding "WARNING" "Rate Limiting" "No rate limiting implementation found"
fi
echo "" | tee -a "$REPORT_FILE"

echo "════════════════════════════════════════════════════════════"
echo "11. CHECKING DATABASE QUERY SAFETY"
echo "════════════════════════════════════════════════════════════"
echo ""

echo "Checking for raw SQL queries..." | tee -a "$REPORT_FILE"
RAW_SQL=$(grep -rn --include="*.rs" \
    -e 'query!.*format!\|execute!.*format!' \
    -e 'query(".*\$\{' \
    . --exclude-dir=target --exclude-dir=.git 2>/dev/null || true)

if [ -n "$RAW_SQL" ]; then
    log_finding "CRITICAL" "SQL Injection Risk" "Found potential SQL injection vulnerabilities"
    echo "$RAW_SQL" | head -20 | tee -a "$REPORT_FILE"
else
    echo -e "${GREEN}✓ No obvious SQL injection vulnerabilities${NC}" | tee -a "$REPORT_FILE"
fi
echo "" | tee -a "$REPORT_FILE"

echo "════════════════════════════════════════════════════════════"
echo "12. CHECKING .ENV AND CONFIG FILES"
echo "════════════════════════════════════════════════════════════"
echo ""

if [ -f ".env" ]; then
    echo -e "${YELLOW}⚠ .env file exists${NC}" | tee -a "$REPORT_FILE"
    echo "  Checking .gitignore..." | tee -a "$REPORT_FILE"
    
    # Check for .env in gitignore, accounting for negations
    if [ -f ".gitignore" ]; then
        GITIGNORE_CHECK=$(grep -E "^!?\.env$" .gitignore 2>/dev/null | tail -1 || true)
        if [ -n "$GITIGNORE_CHECK" ]; then
            if echo "$GITIGNORE_CHECK" | grep -q "^!" 2>/dev/null; then
                log_finding "CRITICAL" "Secret Exposure" ".env file explicitly included in git (negation found)!"
            else
                echo -e "  ${GREEN}✓${NC} .env is in .gitignore" | tee -a "$REPORT_FILE"
            fi
        else
            log_finding "CRITICAL" "Secret Exposure" ".env file NOT in .gitignore!"
        fi
    else
        log_finding "WARNING" "Configuration" "No .gitignore file found"
    fi
else
    echo "  No .env file (using .env.example or environment)" | tee -a "$REPORT_FILE"
fi

if [ -f ".env.example" ]; then
    echo -e "${GREEN}✓ .env.example exists${NC}" | tee -a "$REPORT_FILE"
    echo "  Sample configuration available" | tee -a "$REPORT_FILE"
else
    log_finding "INFO" "Documentation" "Consider adding .env.example"
fi
echo "" | tee -a "$REPORT_FILE"

echo "════════════════════════════════════════════════════════════"
echo "13. CHECKING UNCOMMITTED CHANGES"
echo "════════════════════════════════════════════════════════════"
echo ""

# Check for uncommitted Rust files
UNCOMMITTED=$(git diff --name-only --diff-filter=ACMR 2>/dev/null | grep -E '\.(rs|toml)$' || true)
if [ -n "$UNCOMMITTED" ]; then
    echo -e "${YELLOW}⚠ Uncommitted Rust/TOML files:${NC}" | tee -a "$REPORT_FILE"
    echo "$UNCOMMITTED" | tee -a "$REPORT_FILE"
    echo "" | tee -a "$REPORT_FILE"
    
    # Show what changed in these files (limited output)
    while IFS= read -r file; do
        echo "Changes in $file:" | tee -a "$REPORT_FILE"
        git diff "$file" 2>/dev/null | head -50 | tee -a "$REPORT_FILE"
        echo "..." | tee -a "$REPORT_FILE"
        echo "" | tee -a "$REPORT_FILE"
    done <<< "$UNCOMMITTED"
else
    echo -e "${GREEN}✓ No uncommitted Rust/TOML changes${NC}" | tee -a "$REPORT_FILE"
fi
echo "" | tee -a "$REPORT_FILE"

# Check staged files
STAGED=$(git diff --cached --name-only 2>/dev/null | grep -E '\.(rs|toml)$' || true)
if [ -n "$STAGED" ]; then
    echo -e "${BLUE}ℹ Staged files:${NC}" | tee -a "$REPORT_FILE"
    echo "$STAGED" | tee -a "$REPORT_FILE"
else
    echo "  No staged Rust/TOML files" | tee -a "$REPORT_FILE"
fi
echo "" | tee -a "$REPORT_FILE"

echo "════════════════════════════════════════════════════════════"
echo "14. GENERATING FILE INVENTORY"
echo "════════════════════════════════════════════════════════════"
echo ""

echo "Creating complete file inventory..." | tee -a "$REPORT_FILE"

INVENTORY_FILE="audit-reports/file-inventory.txt"
touch "$INVENTORY_FILE"
chmod 600 "$INVENTORY_FILE"

# All Rust files
echo "=== ALL RUST FILES ===" > "$INVENTORY_FILE"
find . -name "*.rs" -not -path "*/target/*" -not -path "*/.git/*" | sort >> "$INVENTORY_FILE"

echo "" >> "$INVENTORY_FILE"
echo "=== ALL MAIN.RS FILES ===" >> "$INVENTORY_FILE"
find . -name "main.rs" -not -path "*/target/*" -not -path "*/.git/*" | sort >> "$INVENTORY_FILE"

echo "" >> "$INVENTORY_FILE"
echo "=== AUTH FILES ===" >> "$INVENTORY_FILE"
find . -name "*auth*.rs" -not -path "*/target/*" -not -path "*/.git/*" | sort >> "$INVENTORY_FILE"

echo "" >> "$INVENTORY_FILE"
echo "=== VALIDATION FILES ===" >> "$INVENTORY_FILE"
find . -name "*validation*.rs" -not -path "*/target/*" -not -path "*/.git/*" | sort >> "$INVENTORY_FILE"

echo "" >> "$INVENTORY_FILE"
echo "=== MIDDLEWARE FILES ===" >> "$INVENTORY_FILE"
find . -name "*middleware*.rs" -not -path "*/target/*" -not -path "*/.git/*" | sort >> "$INVENTORY_FILE"

echo -e "${GREEN}✓ File inventory saved to $INVENTORY_FILE${NC}"

echo "════════════════════════════════════════════════════════════"
echo "15. SECURITY SCORE CALCULATION"
echo "════════════════════════════════════════════════════════════"
echo ""

# Count findings by severity
CRITICAL_COUNT=$(grep -c "\[CRITICAL\]" "$REPORT_FILE" 2>/dev/null || echo 0)
WARNING_COUNT=$(grep -c "\[WARNING\]" "$REPORT_FILE" 2>/dev/null || echo 0)
INFO_COUNT=$(grep -c "\[INFO\]" "$REPORT_FILE" 2>/dev/null || echo 0)

echo "Security Audit Summary:" | tee -a "$REPORT_FILE"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" | tee -a "$REPORT_FILE"
echo -e "${RED}Critical Issues:${NC} $CRITICAL_COUNT" | tee -a "$REPORT_FILE"
echo -e "${YELLOW}Warnings:${NC} $WARNING_COUNT" | tee -a "$REPORT_FILE"
echo -e "${BLUE}Informational:${NC} $INFO_COUNT" | tee -a "$REPORT_FILE"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" | tee -a "$REPORT_FILE"
echo "" | tee -a "$REPORT_FILE"

# Calculate score (100 - penalties)
SCORE=$((100 - (CRITICAL_COUNT * 20) - (WARNING_COUNT * 5) - (INFO_COUNT * 1)))
if [ $SCORE -lt 0 ]; then SCORE=0; fi

echo "Security Score: $SCORE/100" | tee -a "$REPORT_FILE"
echo "" | tee -a "$REPORT_FILE"

if [ $SCORE -ge 90 ]; then
    echo -e "${GREEN}✓ EXCELLENT - Production Ready${NC}" | tee -a "$REPORT_FILE"
elif [ $SCORE -ge 70 ]; then
    echo -e "${YELLOW}⚠ GOOD - Minor improvements needed${NC}" | tee -a "$REPORT_FILE"
elif [ $SCORE -ge 50 ]; then
    echo -e "${YELLOW}⚠ FAIR - Address warnings before production${NC}" | tee -a "$REPORT_FILE"
else
    echo -e "${RED}✗ POOR - Critical issues must be fixed${NC}" | tee -a "$REPORT_FILE"
fi
echo "" | tee -a "$REPORT_FILE"

echo "════════════════════════════════════════════════════════════"
echo "AUDIT COMPLETE"
echo "════════════════════════════════════════════════════════════"
echo ""
echo "Full report saved to: $REPORT_FILE"
echo "File inventory saved to: $INVENTORY_FILE"
echo "Report file permissions: $(stat -f %Lp "$REPORT_FILE" 2>/dev/null || stat -c %a "$REPORT_FILE" 2>/dev/null || echo "unknown")"
echo ""
echo "⚠️  IMPORTANT SECURITY NOTES:"
echo "  • Secrets in report are REDACTED for safety"
echo "  • Report files have 600 permissions (owner-only)"
echo "  • DO NOT commit audit-reports/ to git"
echo ""
echo "Next Steps:"
if [ $CRITICAL_COUNT -gt 0 ]; then
    echo "  1. ⚠️  Fix $CRITICAL_COUNT critical issues IMMEDIATELY"
fi
if [ $WARNING_COUNT -gt 0 ]; then
    echo "  2. Address $WARNING_COUNT warnings before production"
fi
echo "  3. Review $INVENTORY_FILE for complete file list"
echo "  4. Check 'git diff' for uncommitted changes"
echo "  5. Run tests after fixes: ./test-phase6-complete-part1.sh"
echo "  6. Add audit-reports/ to .gitignore if not already"
echo ""

# Ensure audit-reports is in .gitignore
if [ -f ".gitignore" ]; then
    if ! grep -q "^audit-reports/" .gitignore 2>/dev/null; then
        echo "audit-reports/" >> .gitignore
        echo "✓ Added audit-reports/ to .gitignore"
    fi
fi