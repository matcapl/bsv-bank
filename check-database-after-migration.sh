#!/bin/bash

# ============================================================================
# Post-Migration Database Validator
# ============================================================================
# Purpose: Validates database state after migration to ensure it matches
#          the expected schema defined in the SQL migration file
# Usage: ./check-database-after-migration.sh <migration-file.sql>
# ============================================================================

set -euo pipefail

# Color codes
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
MAGENTA='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m'
BOLD='\033[1m'

# Configuration
DB_NAME="${POSTGRES_DB:-bsv_bank}"
DB_USER="${POSTGRES_USER:-a}"
DB_HOST="${POSTGRES_HOST:-localhost}"
DB_PORT="${POSTGRES_PORT:-5432}"

# Counters
TOTAL_CHECKS=0
PASSED_CHECKS=0
FAILED_CHECKS=0
WARNING_COUNT=0

declare -a MISSING_ITEMS
declare -a VERIFICATION_FAILURES

# ============================================================================
# Output Functions
# ============================================================================

print_header() {
    echo -e "\n${CYAN}${BOLD}════════════════════════════════════════════════════════════════${NC}"
    echo -e "${CYAN}${BOLD}  $1${NC}"
    echo -e "${CYAN}${BOLD}════════════════════════════════════════════════════════════════${NC}\n"
}

print_section() {
    echo -e "\n${BLUE}${BOLD}▶ $1${NC}"
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
}

print_success() {
    echo -e "${GREEN}✓${NC} $1"
    ((PASSED_CHECKS = PASSED_CHECKS + 1))
}

print_error() {
    echo -e "${RED}✗${NC} $1"
    ((FAILED_CHECKS = FAILED_CHECKS + 1))
    VERIFICATION_FAILURES+=("$1")
}

print_warning() {
    echo -e "${YELLOW}⚠${NC} $1"
    ((WARNING_COUNT = WARNING_COUNT + 1))
}

print_info() {
    echo -e "${CYAN}ℹ${NC} $1"
}

# ============================================================================
# Database Query Functions
# ============================================================================

query_db() {
    local query="$1"
    PGPASSWORD="${POSTGRES_PASSWORD:-}" psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -t -A -c "$query" 2>/dev/null || echo ""
}

check_db_exists() {
    PGPASSWORD="${POSTGRES_PASSWORD:-}" psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -lqt 2>/dev/null | cut -d \| -f 1 | grep -qw "$DB_NAME"
}

table_exists() {
    local table="$1"
    local result=$(query_db "SELECT EXISTS (SELECT FROM information_schema.tables WHERE table_schema = 'public' AND table_name = '$table');")
    [[ "$result" == "t" ]]
}

column_exists() {
    local table="$1"
    local column="$2"
    local result=$(query_db "SELECT EXISTS (SELECT FROM information_schema.columns WHERE table_schema = 'public' AND table_name = '$table' AND column_name = '$column');")
    [[ "$result" == "t" ]]
}

get_column_type() {
    local table="$1"
    local column="$2"
    query_db "SELECT data_type FROM information_schema.columns WHERE table_schema = 'public' AND table_name = '$table' AND column_name = '$column';"
}

get_column_nullable() {
    local table="$1"
    local column="$2"
    query_db "SELECT is_nullable FROM information_schema.columns WHERE table_schema = 'public' AND table_name = '$table' AND column_name = '$column';"
}

get_column_default() {
    local table="$1"
    local column="$2"
    query_db "SELECT column_default FROM information_schema.columns WHERE table_schema = 'public' AND table_name = '$table' AND column_name = '$column';"
}

index_exists() {
    local index="$1"
    local result=$(query_db "SELECT EXISTS (SELECT FROM pg_indexes WHERE schemaname = 'public' AND indexname = '$index');")
    [[ "$result" == "t" ]]
}

constraint_exists() {
    local constraint="$1"
    local result=$(query_db "SELECT EXISTS (SELECT FROM information_schema.table_constraints WHERE table_schema = 'public' AND constraint_name = '$constraint');")
    [[ "$result" == "t" ]]
}

function_exists() {
    local func="$1"
    local result=$(query_db "SELECT EXISTS (SELECT FROM pg_proc p JOIN pg_namespace n ON p.pronamespace = n.oid WHERE n.nspname = 'public' AND p.proname = '$func');")
    [[ "$result" == "t" ]]
}

trigger_exists() {
    local trigger="$1"
    local result=$(query_db "SELECT EXISTS (SELECT FROM pg_trigger WHERE tgname = '$trigger');")
    [[ "$result" == "t" ]]
}

view_exists() {
    local view="$1"
    local result=$(query_db "SELECT EXISTS (SELECT FROM information_schema.views WHERE table_schema = 'public' AND table_name = '$view');")
    [[ "$result" == "t" ]]
}

get_row_count() {
    local table="$1"
    query_db "SELECT COUNT(*) FROM \"$table\";" 2>/dev/null || echo "0"
}

get_table_columns() {
    local table="$1"
    query_db "SELECT column_name, data_type, is_nullable, column_default FROM information_schema.columns WHERE table_schema = 'public' AND table_name = '$table' ORDER BY ordinal_position;"
}

# ============================================================================
# Type Normalization Function
# ============================================================================

normalize_pg_type() {
    local type="$1"
    type=$(echo "$type" | tr '[:upper:]' '[:lower:]' | sed 's/[[:space:]]//g')
    
    # Normalize common PostgreSQL type aliases
    type="${type//varchar/character varying}"
    type="${type//timestamptz/timestamp with time zone}"
    type="${type//timestamp without time zone/timestamp}"
    type="${type//int8/bigint}"
    type="${type//int4/integer}"
    type="${type//int2/smallint}"
    type="${type//int/integer}"
    type="${type//bool/boolean}"
    
    echo "$type"
}

types_match() {
    local expected="$1"
    local actual="$2"
    
    local norm_expected=$(normalize_pg_type "$expected")
    local norm_actual=$(normalize_pg_type "$actual")
    
    # Check if either contains the other (handles variations)
    [[ "$norm_actual" == *"$norm_expected"* ]] || [[ "$norm_expected" == *"$norm_actual"* ]]
}

# ============================================================================
# SQL Parsing Functions
# ============================================================================

extract_create_tables() {
    local file="$1"
    grep -iE "CREATE TABLE" "$file" | \
        grep -v "^[[:space:]]*--" | \
        sed -E 's/.*CREATE TABLE[[:space:]]+(IF NOT EXISTS[[:space:]]+)?([a-z_]+).*/\2/I' | \
        sort -u | grep -v "^$" || true
}

extract_do_block_columns() {
    local file="$1"
    awk '/DO \$\$/,/END \$\$/' "$file" | grep -iE "ADD COLUMN" | \
        sed -E 's/.*ALTER TABLE[[:space:]]+([a-z_]+).*ADD COLUMN[[:space:]]+([a-z_]+)[[:space:]]+([A-Z]+[A-Z0-9()]*).*/\1:\2:\3/I' || true
}

extract_indexes() {
    local file="$1"
    grep -iE "CREATE[[:space:]]+(UNIQUE[[:space:]]+)?INDEX" "$file" | \
        grep -v "^[[:space:]]*--" | \
        grep -v "^--" | \
        sed -E 's/.*CREATE[[:space:]]+(UNIQUE[[:space:]]+)?INDEX[[:space:]]+(IF[[:space:]]+NOT[[:space:]]+EXISTS[[:space:]]+)?([a-z_0-9]+)[[:space:]]+ON.*/\3/I' | \
        grep -v "^$" | \
        grep -vE "^(INDEX|IF|NOT|EXISTS|UNIQUE|CREATE|ON)$" | \
        sort -u || true
}

extract_constraints() {
    local file="$1"
    grep -iE "CONSTRAINT[[:space:]]+[a-z_]+" "$file" | \
        grep -v "^[[:space:]]*--" | \
        sed -E 's/.*CONSTRAINT[[:space:]]+([a-z_]+).*/\1/I' | sort -u | grep -v "^$" || true
}

extract_functions() {
    local file="$1"
    grep -iE "CREATE.*FUNCTION" "$file" | \
        grep -v "^[[:space:]]*--" | \
        sed -E 's/.*CREATE.*FUNCTION[[:space:]]+([a-z_]+)\(.*/\1/I' | \
        sort -u | grep -v "^$" || true
}

extract_triggers() {
    local file="$1"
    grep -iE "CREATE TRIGGER" "$file" | \
        grep -v "^[[:space:]]*--" | \
        sed -E 's/.*CREATE TRIGGER[[:space:]]+([a-z_]+).*/\1/I' | \
        sort -u | grep -v "^$" || true
}

extract_views() {
    local file="$1"
    grep -iE "CREATE.*VIEW" "$file" | \
        grep -v "^[[:space:]]*--" | \
        sed -E 's/.*CREATE.*VIEW[[:space:]]+([a-z_]+).*/\1/I' | \
        sort -u | grep -v "^$" || true
}

# ============================================================================
# Safe Integer Check
# ============================================================================

safe_int_check() {
    local var="$1"
    var=$(echo "$var" | tr -d '\n\r' | grep -oE '[0-9]+' | head -1)
    echo "${var:-0}"
}

# ============================================================================
# Main Validation Logic
# ============================================================================

main() {
    local migration_file="${1:-}"
    
    print_header "Post-Migration Database Validator"
    
    # Validate input
    if [[ -z "$migration_file" ]]; then
        echo -e "${RED}${BOLD}Error: No migration file specified${NC}"
        echo -e "Usage: $0 <migration-file.sql>"
        exit 1
    fi
    
    if [[ ! -f "$migration_file" ]]; then
        echo -e "${RED}${BOLD}Error: Migration file not found: $migration_file${NC}"
        exit 1
    fi
    
    print_info "Migration file: ${BOLD}$migration_file${NC}"
    print_info "Database: ${BOLD}$DB_NAME${NC} @ ${BOLD}$DB_HOST:$DB_PORT${NC}"
    
    # Check database connectivity
    print_section "Database Connectivity"
    ((TOTAL_CHECKS = TOTAL_CHECKS + 1))
    
    if check_db_exists; then
        print_success "Database '$DB_NAME' is accessible"
    else
        print_error "Cannot connect to database '$DB_NAME'"
        exit 1
    fi
    
    # ========================================================================
    # Parse Expected Schema from Migration File
    # ========================================================================
    
    print_section "Parsing Expected Schema"
    
    local expected_tables=($(extract_create_tables "$migration_file"))
    local expected_columns=($(extract_do_block_columns "$migration_file"))
    local expected_indexes=($(extract_indexes "$migration_file"))
    local expected_constraints=($(extract_constraints "$migration_file"))
    local expected_functions=($(extract_functions "$migration_file"))
    local expected_triggers=($(extract_triggers "$migration_file"))
    local expected_views=($(extract_views "$migration_file"))
    
    print_info "Expected tables: ${BOLD}${#expected_tables[@]}${NC}"
    print_info "Expected columns: ${BOLD}${#expected_columns[@]}${NC}"
    print_info "Expected indexes: ${BOLD}${#expected_indexes[@]}${NC}"
    print_info "Expected constraints: ${BOLD}${#expected_constraints[@]}${NC}"
    print_info "Expected functions: ${BOLD}${#expected_functions[@]}${NC}"
    print_info "Expected triggers: ${BOLD}${#expected_triggers[@]}${NC}"
    print_info "Expected views: ${BOLD}${#expected_views[@]}${NC}"
    
    # ========================================================================
    # Verify Tables
    # ========================================================================
    
    if [[ ${#expected_tables[@]} -gt 0 ]]; then
        print_section "Verifying Tables"
        
        for table in "${expected_tables[@]}"; do
            [[ -z "$table" ]] && continue
            ((TOTAL_CHECKS++))
            
            if table_exists "$table"; then
                local row_count=$(get_row_count "$table")
                print_success "Table '$table' exists ($row_count rows)"
                
                # Show structure
                local columns=$(get_table_columns "$table")
                if [[ -n "$columns" ]]; then
                    echo "$columns" | while IFS='|' read -r col_name col_type col_null col_default; do
                        local null_str="NULL"
                        [[ "$col_null" == "NO" ]] && null_str="NOT NULL"
                        local default_str=""
                        [[ -n "$col_default" ]] && default_str=" DEFAULT $col_default"
                        echo -e "    ${CYAN}→${NC} $col_name ${YELLOW}$col_type${NC} ${null_str}${default_str}"
                    done
                fi
            else
                print_error "Table '$table' does NOT exist"
                MISSING_ITEMS+=("Table: $table")
            fi
        done
    fi
    
    # ========================================================================
    # Verify Columns Added via DO Blocks
    # ========================================================================
    
    if [[ ${#expected_columns[@]} -gt 0 ]]; then
        print_section "Verifying Column Additions"
        
        for entry in "${expected_columns[@]}"; do
            [[ -z "$entry" ]] && continue
            ((TOTAL_CHECKS++))
            
            IFS=':' read -r table column expected_type <<< "$entry"
            
            if ! table_exists "$table"; then
                print_warning "Cannot verify column '$column' - table '$table' doesn't exist"
                continue
            fi
            
            if column_exists "$table" "$column"; then
                local actual_type=$(get_column_type "$table" "$column")
                local nullable=$(get_column_nullable "$table" "$column")
                
                if types_match "$expected_type" "$actual_type"; then
                    print_success "Column '$table.$column' exists (type: $actual_type, nullable: $nullable)"
                else
                    print_error "Column '$table.$column' has wrong type: expected $expected_type, got $actual_type"
                fi
            else
                print_error "Column '$table.$column' does NOT exist"
                MISSING_ITEMS+=("Column: $table.$column")
            fi
        done
    fi
    
    # ========================================================================
    # Verify Indexes
    # ========================================================================
    
    if [[ ${#expected_indexes[@]} -gt 0 ]]; then
        print_section "Verifying Indexes"
        
        for index in "${expected_indexes[@]}"; do
            [[ -z "$index" ]] && continue
            ((TOTAL_CHECKS++))
            
            if index_exists "$index"; then
                print_success "Index '$index' exists"
            else
                print_error "Index '$index' does NOT exist"
                MISSING_ITEMS+=("Index: $index")
            fi
        done
    fi
    
    # ========================================================================
    # Verify Constraints
    # ========================================================================
    
    if [[ ${#expected_constraints[@]} -gt 0 ]]; then
        print_section "Verifying Constraints"
        
        for constraint in "${expected_constraints[@]}"; do
            [[ -z "$constraint" ]] && continue
            ((TOTAL_CHECKS++))
            
            if constraint_exists "$constraint"; then
                print_success "Constraint '$constraint' exists"
            else
                print_warning "Constraint '$constraint' not found (may have different name)"
            fi
        done
    fi
    
    # ========================================================================
    # Verify Functions
    # ========================================================================
    
    if [[ ${#expected_functions[@]} -gt 0 ]]; then
        print_section "Verifying Functions"
        
        for func in "${expected_functions[@]}"; do
            [[ -z "$func" ]] && continue
            ((TOTAL_CHECKS++))
            
            if function_exists "$func"; then
                print_success "Function '$func' exists"
            else
                print_error "Function '$func' does NOT exist"
                MISSING_ITEMS+=("Function: $func")
            fi
        done
    fi
    
    # ========================================================================
    # Verify Triggers
    # ========================================================================
    
    if [[ ${#expected_triggers[@]} -gt 0 ]]; then
        print_section "Verifying Triggers"
        
        for trigger in "${expected_triggers[@]}"; do
            [[ -z "$trigger" ]] && continue
            ((TOTAL_CHECKS++))
            
            if trigger_exists "$trigger"; then
                print_success "Trigger '$trigger' exists"
            else
                print_error "Trigger '$trigger' does NOT exist"
                MISSING_ITEMS+=("Trigger: $trigger")
            fi
        done
    fi
    
    # ========================================================================
    # Verify Views
    # ========================================================================
    
    if [[ ${#expected_views[@]} -gt 0 ]]; then
        print_section "Verifying Views"
        
        for view in "${expected_views[@]}"; do
            [[ -z "$view" ]] && continue
            ((TOTAL_CHECKS++))
            
            if view_exists "$view"; then
                print_success "View '$view' exists"
            else
                print_error "View '$view' does NOT exist"
                MISSING_ITEMS+=("View: $view")
            fi
        done
    fi
    
    # ========================================================================
    # Database Integrity Checks
    # ========================================================================
    
    print_section "Database Integrity Checks"
    
    # Check for orphaned foreign keys
    ((TOTAL_CHECKS++))
    local broken_fks=$(query_db "SELECT COUNT(*) FROM pg_constraint WHERE contype = 'f' AND NOT EXISTS (SELECT 1 FROM pg_class WHERE oid = confrelid);")
    broken_fks=$(safe_int_check "$broken_fks")
    if [[ "$broken_fks" -eq 0 ]]; then
        print_success "No broken foreign key constraints"
    else
        print_error "Found $broken_fks broken foreign key constraint(s)"
    fi
    
    # Check for invalid indexes
    ((TOTAL_CHECKS++))
    local invalid_indexes=$(query_db "SELECT COUNT(*) FROM pg_index WHERE indisvalid = false;")
    invalid_indexes=$(safe_int_check "$invalid_indexes")
    if [[ "$invalid_indexes" -eq 0 ]]; then
        print_success "All indexes are valid"
    else
        print_error "Found $invalid_indexes invalid index(es)"
    fi
    
    # Check for tables without primary keys (warning only)
    ((TOTAL_CHECKS++))
    local tables_no_pk=$(query_db "SELECT COUNT(*) FROM information_schema.tables t WHERE table_schema = 'public' AND table_type = 'BASE TABLE' AND NOT EXISTS (SELECT 1 FROM information_schema.table_constraints tc WHERE tc.table_schema = t.table_schema AND tc.table_name = t.table_name AND tc.constraint_type = 'PRIMARY KEY');")
    tables_no_pk=$(safe_int_check "$tables_no_pk")
    if [[ "$tables_no_pk" -eq 0 ]]; then
        print_success "All tables have primary keys"
    else
        print_warning "Found $tables_no_pk table(s) without primary keys"
    fi
    
    # ========================================================================
    # Migration Log Analysis
    # ========================================================================
    
    print_section "Migration Execution Analysis"
    
    # Check PostgreSQL log for errors during migration (if accessible)
    if [[ -f "/tmp/psql_migration.log" ]]; then
        local error_count=$(safe_int_check "$(grep -c "ERROR:" "/tmp/psql_migration.log" 2>/dev/null || echo "0")")
        if [[ "$error_count" -gt 0 ]]; then
            print_warning "Found $error_count ERROR(s) in migration log"
            print_info "Review /tmp/psql_migration.log for details"
        else
            print_success "No errors in migration log"
        fi
    else
        print_info "Migration log not found (run: psql ... -f migration.sql 2>&1 | tee /tmp/psql_migration.log)"
    fi
    
    # ========================================================================
    # Final Summary
    # ========================================================================
    
    print_header "Validation Summary"
    
    echo -e "${BOLD}Migration File:${NC} $(basename "$migration_file")"
    echo -e "${BOLD}Total Checks:${NC} $TOTAL_CHECKS"
    echo -e "${GREEN}${BOLD}✓ Passed:${NC} $PASSED_CHECKS"
    echo -e "${YELLOW}${BOLD}⚠ Warnings:${NC} $WARNING_COUNT"
    echo -e "${RED}${BOLD}✗ Failed:${NC} $FAILED_CHECKS"
    
    echo ""
    
    # Show missing items
    if [[ ${#MISSING_ITEMS[@]} -gt 0 ]]; then
        echo -e "${RED}${BOLD}Missing Schema Elements:${NC}"
        for item in "${MISSING_ITEMS[@]}"; do
            echo -e "  ${RED}•${NC} $item"
        done
        echo ""
    fi
    
    # Show verification failures
    if [[ ${#VERIFICATION_FAILURES[@]} -gt 0 ]]; then
        echo -e "${RED}${BOLD}Verification Failures:${NC}"
        for failure in "${VERIFICATION_FAILURES[@]}"; do
            echo -e "  ${RED}•${NC} $failure"
        done
        echo ""
    fi
    
    # Final verdict
    if [[ $FAILED_CHECKS -eq 0 ]]; then
        if [[ $WARNING_COUNT -eq 0 ]]; then
            echo -e "${GREEN}${BOLD}✓✓✓ MIGRATION SUCCESSFUL ✓✓✓${NC}"
            echo -e "${GREEN}${BOLD}All expected schema elements are present.${NC}\n"
            exit 0
        else
            echo -e "${YELLOW}${BOLD}⚠ MIGRATION COMPLETED WITH WARNINGS ⚠${NC}"
            echo -e "${YELLOW}${BOLD}Review warnings above.${NC}\n"
            exit 0
        fi
    else
        echo -e "${RED}${BOLD}✗✗✗ MIGRATION VERIFICATION FAILED ✗✗✗${NC}"
        echo -e "${RED}${BOLD}Expected schema elements are missing.${NC}"
        echo -e "${RED}${BOLD}Migration may have failed or been incomplete.${NC}\n"
        
        echo -e "${CYAN}${BOLD}Recommended Actions:${NC}"
        echo -e "  ${CYAN}1.${NC} Review migration errors above"
        echo -e "  ${CYAN}2.${NC} Check PostgreSQL logs for detailed errors"
        echo -e "  ${CYAN}3.${NC} Fix SQL file and re-run migration"
        echo -e "  ${CYAN}4.${NC} Consider rolling back: ${YELLOW}psql -U $DB_USER -d $DB_NAME < backup.sql${NC}\n"
        
        exit 1
    fi
}

# ============================================================================
# Script Entry Point
# ============================================================================

main "$@"