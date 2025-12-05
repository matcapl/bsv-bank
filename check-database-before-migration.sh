#!/bin/bash

# ============================================================================
# SQL Migration Validator & Impact Analyzer
# ============================================================================
# Purpose: Validates SQL migrations AND predicts their impact before execution
#          Acts as an early warning system to improve SQL files
# Usage: ./check-database-before-migration.sh <migration-file.sql>
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

if [[ "${BASH_VERSINFO[0]}" -lt 4 ]]; then
    echo "⚠️  Warning: bash 4.0+ recommended (you have ${BASH_VERSION})"
    echo "   Some features may not work. Consider: brew install bash"
fi

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
SUGGESTIONS=0

# Issues tracking
declare -a BLOCKING_ISSUES
declare -a SQL_PROBLEMS
declare -a SUGGESTIONS_LIST

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
    BLOCKING_ISSUES+=("$1")
}

print_warning() {
    echo -e "${YELLOW}⚠${NC} $1"
    ((WARNING_COUNT = WARNING_COUNT + 1))
}

print_info() {
    echo -e "${CYAN}ℹ${NC} $1"
}

print_suggestion() {
    echo -e "${MAGENTA}💡${NC} $1"
    ((SUGGESTIONS = SUGGESTIONS + 1))
    SUGGESTIONS_LIST+=("$1")
}

add_sql_problem() {
    SQL_PROBLEMS+=("$1")
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

index_exists() {
    local index="$1"
    local result=$(query_db "SELECT EXISTS (SELECT FROM pg_indexes WHERE schemaname = 'public' AND indexname = '$index');")
    [[ "$result" == "t" ]]
}

constraint_exists() {
    local table="$1"
    local constraint="$2"
    local result=$(query_db "SELECT EXISTS (SELECT FROM information_schema.table_constraints WHERE table_schema = 'public' AND table_name = '$table' AND constraint_name = '$constraint');")
    [[ "$result" == "t" ]]
}

function_exists() {
    local func="$1"
    local result=$(query_db "SELECT EXISTS (SELECT FROM pg_proc p JOIN pg_namespace n ON p.pronamespace = n.oid WHERE n.nspname = 'public' AND p.proname = '$func');")
    [[ "$result" == "t" ]]
}

get_row_count() {
    local table="$1"
    query_db "SELECT COUNT(*) FROM \"$table\";" 2>/dev/null || echo "0"
}

get_table_columns() {
    local table="$1"
    query_db "SELECT column_name FROM information_schema.columns WHERE table_schema = 'public' AND table_name = '$table' ORDER BY ordinal_position;" | tr '\n' ',' | sed 's/,$//'
}

# ============================================================================
# SQL File Analysis Functions
# ============================================================================

# Extract CREATE TABLE statements with full definitions
extract_create_tables() {
    local file="$1"
    awk '/CREATE TABLE/,/;/' "$file" | grep -iE "CREATE TABLE" | \
        sed -E 's/.*CREATE TABLE[[:space:]]+(IF NOT EXISTS[[:space:]]+)?([a-z_]+).*/\2/I' | \
        sort -u | grep -v "^$" || true
}

# Extract all ALTER TABLE operations
extract_alter_operations() {
    local file="$1"
    grep -in "ALTER TABLE" "$file" || true
}

# Extract column additions from DO blocks
extract_do_block_columns() {
    local file="$1"
    awk '/DO \$\$/,/END \$\$/' "$file" | grep -iE "ADD COLUMN" | \
        sed -E 's/.*ALTER TABLE[[:space:]]+([a-z_]+).*ADD COLUMN[[:space:]]+([a-z_]+)[[:space:]]+([A-Z]+).*/\1:\2:\3/I' || true
}

# Extract all table references
extract_all_tables_mentioned() {
    local file="$1"
    {
        grep -ioE "(CREATE|ALTER|DROP|INSERT INTO|UPDATE|DELETE FROM|REFERENCES)[[:space:]]+TABLE[[:space:]]+([IF NOT EXISTS][[:space:]]+)?[a-z_]+" "$file" | awk '{print $NF}'
        grep -ioE "table_name[[:space:]]*=[[:space:]]*'[a-z_]+'" "$file" | sed -E "s/.*'([a-z_]+)'.*/\1/"
        grep -ioE "FROM[[:space:]]+[a-z_]+" "$file" | awk '{print $2}'
    } | sort -u | grep -v "^$" | grep -v "TABLE" | grep -v "EXISTS" || true
}

# # ============================================================================
# # Fixed Type Normalization Function
# # ============================================================================
# normalize_pg_type() {
#     local type="$1"
#     type=$(echo "$type" | tr '[:upper:]' '[:lower:]')
    
#     # Normalize common PostgreSQL type aliases
#     type="${type//varchar/character varying}"
#     type="${type//timestamptz/timestamp with time zone}"
#     type="${type//timestamp without time zone/timestamp}"
#     type="${type//int8/bigint}"
#     type="${type//int4/integer}"
#     type="${type//int2/smallint}"
#     type="${type//bool/boolean}"
    
#     echo "$type"
# }

# # ============================================================================
# # Fixed Index Extraction (skip comments)
# # ============================================================================
# extract_indexes() {
#     local file="$1"
#     grep -iE "CREATE[[:space:]]+(UNIQUE[[:space:]]+)?INDEX" "$file" | \
#         grep -v "^[[:space:]]*--" | \
#         grep -v "^--" | \
#         sed -E 's/.*CREATE[[:space:]]+(UNIQUE[[:space:]]+)?INDEX[[:space:]]+(IF[[:space:]]+NOT[[:space:]]+EXISTS[[:space:]]+)?([a-z_0-9]+).*/\3/I' | \
#         grep -v "^$" | \
#         grep -vE "^(INDEX|IF|NOT|EXISTS|UNIQUE|CREATE)$" | \
#         sort -u || true
# }

# # ============================================================================
# # Fixed Column Type Comparison
# # ============================================================================
# # In column verification:
# if column_exists "$table" "$column"; then
#     local actual_type=$(get_column_type "$table" "$column")
#     local nullable=$(get_column_nullable "$table" "$column")
    
#     # Normalize both types
#     local expected_normalized=$(normalize_pg_type "$expected_type")
#     local actual_normalized=$(normalize_pg_type "$actual_type")
    
#     if [[ "$actual_normalized" == *"$expected_normalized"* ]] || \
#        [[ "$expected_normalized" == *"$actual_normalized"* ]]; then
#         print_success "Column '$table.$column' exists (type: $actual_type, nullable: $nullable)"
#     else
#         print_error "Column '$table.$column' type mismatch: expected $expected_type, got $actual_type"
#     fi
# fi

# # ============================================================================
# # Fixed Integer Check (handles newlines)
# # ============================================================================
# safe_int_check() {
#     local var="$1"
#     var=$(echo "$var" | tr -d '\n\r' | grep -oE '[0-9]+' | head -1)
#     echo "${var:-0}"
# }

# # Usage:
# error_count=$(safe_int_check "$(grep -c "ERROR:" "/tmp/psql_migration.log" 2>/dev/null || echo "0")")

# if [[ $error_count -gt 0 ]]; then
#     print_warning "Found $error_count ERROR(s) in migration log"
# fi

# Extract indexes
extract_indexes() {
    local file="$1"
    grep -iE "CREATE.*INDEX" "$file" | \
        sed -E 's/.*CREATE.*INDEX[[:space:]]+(IF NOT EXISTS[[:space:]]+)?([a-z_]+)[[:space:]]+ON[[:space:]]+([a-z_]+).*/\3:\2/I' || true
}

# Extract foreign keys
extract_foreign_keys() {
    local file="$1"
    grep -in "FOREIGN KEY\|REFERENCES" "$file" | grep -v "^--" || true
}

# Extract constraints
extract_constraints() {
    local file="$1"
    grep -iE "CONSTRAINT[[:space:]]+[a-z_]+" "$file" | \
        sed -E 's/.*CONSTRAINT[[:space:]]+([a-z_]+).*/\1/I' | sort -u || true
}

# Detect dangerous operations
detect_dangerous_ops() {
    local file="$1"
    local line_num=1
    local dangerous=()
    
    while IFS= read -r line; do
        if echo "$line" | grep -iq "DROP TABLE.*CASCADE"; then
            dangerous+=("Line $line_num: DROP TABLE CASCADE (DESTRUCTIVE)")
        elif echo "$line" | grep -iq "DROP TABLE"; then
            dangerous+=("Line $line_num: DROP TABLE")
        fi
        
        if echo "$line" | grep -iq "TRUNCATE"; then
            dangerous+=("Line $line_num: TRUNCATE (DATA LOSS)")
        fi
        
        if echo "$line" | grep -iq "DELETE FROM" && ! echo "$line" | grep -iq "WHERE"; then
            dangerous+=("Line $line_num: DELETE without WHERE (FULL TABLE DELETE)")
        fi
        
        if echo "$line" | grep -iq "ALTER TABLE.*DROP COLUMN"; then
            dangerous+=("Line $line_num: DROP COLUMN (PERMANENT DATA LOSS)")
        fi
        
        if echo "$line" | grep -iq "DROP DATABASE"; then
            dangerous+=("Line $line_num: DROP DATABASE (CATASTROPHIC)")
        fi
        
        ((line_num++))
    done < "$file"
    
    printf '%s\n' "${dangerous[@]:-}"
}

# Check for missing IF NOT EXISTS
check_if_not_exists_usage() {
    local file="$1"
    local issues=()
    
    while IFS= read -r line; do
        if echo "$line" | grep -iq "CREATE TABLE" && ! echo "$line" | grep -iq "IF NOT EXISTS"; then
            issues+=("CREATE TABLE without IF NOT EXISTS: $line")
        fi
        if echo "$line" | grep -iq "CREATE INDEX" && ! echo "$line" | grep -iq "IF NOT EXISTS"; then
            issues+=("CREATE INDEX without IF NOT EXISTS: $line")
        fi
    done < "$file"
    
    printf '%s\n' "${issues[@]}"
}

# Check for missing ON CONFLICT clauses in INSERT
check_insert_conflicts() {
    local file="$1"
    local issues=()
    
    while IFS= read -r line; do
        if echo "$line" | grep -iq "INSERT INTO" && ! echo "$line" | grep -iq "ON CONFLICT"; then
            issues+=("INSERT without ON CONFLICT handling: $line")
        fi
    done < "$file"
    
    printf '%s\n' "${issues[@]}"
}

# Validate SQL syntax using PostgreSQL
validate_sql_syntax() {
    local file="$1"
    local temp_db="syntax_check_$$"
    
    # Create temporary database for syntax check
    PGPASSWORD="${POSTGRES_PASSWORD:-}" createdb -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" "$temp_db" 2>/dev/null || return 1
    
    # Try to execute in transaction (will rollback)
    local output=$(PGPASSWORD="${POSTGRES_PASSWORD:-}" psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$temp_db" \
        -v ON_ERROR_STOP=1 \
        -c "BEGIN; $(cat "$file"); ROLLBACK;" 2>&1 || echo "SYNTAX_ERROR")
    
    # Cleanup
    PGPASSWORD="${POSTGRES_PASSWORD:-}" dropdb -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" "$temp_db" 2>/dev/null || true
    
    if echo "$output" | grep -q "SYNTAX_ERROR\|ERROR:"; then
        echo "$output"
        return 1
    fi
    
    return 0
}

# Check for unsafe ALTER COLUMN SET NOT NULL
check_unsafe_not_null() {
    local file="$1"
    local -a issues=()
    local line_num=1
    
    while IFS= read -r line; do
        # Check for ALTER COLUMN SET NOT NULL
        if echo "$line" | grep -iq "ALTER.*COLUMN.*SET NOT NULL"; then
            local col_name=$(echo "$line" | sed -E 's/.*ALTER.*COLUMN[[:space:]]+([a-z_]+).*/\1/I')
            
            # Check if there's a preceding UPDATE or conditional check
            local prev_lines=$(head -n $((line_num - 1)) "$file" | tail -n 10)
            
            if ! echo "$prev_lines" | grep -iq "UPDATE.*SET.*$col_name\|IF EXISTS.*$col_name"; then
                issues+=("Line $line_num: SET NOT NULL on '$col_name' without prior NULL check or UPDATE")
            fi
        fi
        ((line_num++))
    done < "$file"
    
    printf '%s\n' ${issues[@]+"${issues[@]}"}
}

# Check for common SQL anti-patterns
check_antipatterns() {
    local file="$1"
    local issues=()
    
    # Check for SELECT *
    if grep -iq "SELECT \*" "$file"; then
        issues+=("Uses SELECT * (consider explicit column lists)")
    fi
    
    # Check for missing indexes on foreign keys
    if grep -iq "REFERENCES" "$file" && ! grep -iq "CREATE INDEX.*foreign" "$file"; then
        issues+=("Foreign keys without corresponding indexes (performance issue)")
    fi
    
    # Check for VARCHAR without length
    if grep -iqE "VARCHAR[[:space:]]*[^(]" "$file"; then
        issues+=("VARCHAR without explicit length")
    fi
    
    # Check for missing DEFAULT values on NOT NULL columns
    if grep -iq "NOT NULL" "$file" && grep -iqE "ADD COLUMN.*NOT NULL" "$file" && ! grep -iq "DEFAULT" "$file"; then
        issues+=("Adding NOT NULL column without DEFAULT (will fail if table has data)")
    fi
    
    # Check for SERIAL in new code (should use IDENTITY or explicit sequences)
    if grep -iq "SERIAL" "$file"; then
        issues+=("Uses SERIAL type (consider using IDENTITY columns instead)")
    fi
    
    printf '%s\n' "${issues[@]}"
}

# ============================================================================
# Main Validation Logic
# ============================================================================

main() {
    local migration_file="${1:-}"
    
    print_header "SQL Migration Validator & Impact Analyzer"
    
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
    
    # ========================================================================
    # PHASE 1: SQL FILE ANALYSIS (No database required)
    # ========================================================================
    
    print_header "Phase 1: SQL File Analysis"
    
    print_section "Basic File Information"
    local file_size=$(du -h "$migration_file" | cut -f1)
    local line_count=$(wc -l < "$migration_file")
    local has_plpgsql=$(grep -c "DO \$\$" "$migration_file" 2>/dev/null || echo "0")
    local has_transactions=$(grep -ic "BEGIN\|COMMIT\|ROLLBACK" "$migration_file" 2>/dev/null || echo "0")
    
    print_info "File size: ${BOLD}$file_size${NC}"
    print_info "Lines: ${BOLD}$line_count${NC}"
    print_info "PL/pgSQL blocks: ${BOLD}$has_plpgsql${NC}"
    print_info "Transaction statements: ${BOLD}$has_transactions${NC}"
    
    # Parse SQL structure
    print_section "SQL Structure Analysis"
    
    local tables=($(extract_create_tables "$migration_file"))
    local all_tables=($(extract_all_tables_mentioned "$migration_file"))
    local indexes=($(extract_indexes "$migration_file"))
    local constraints=($(extract_constraints "$migration_file"))
    local do_columns=($(extract_do_block_columns "$migration_file"))
    
    print_info "Tables to create: ${BOLD}${#tables[@]}${NC}"
    if [[ ${#tables[@]} -gt 0 ]]; then
        for table in "${tables[@]}"; do
            echo -e "  ${CYAN}→${NC} $table"
        done
    fi
    
    print_info "All tables referenced: ${BOLD}${#all_tables[@]}${NC}"
    print_info "Indexes to create: ${BOLD}${#indexes[@]}${NC}"
    print_info "Constraints defined: ${BOLD}${#constraints[@]}${NC}"
    
    # Check for dangerous operations
    print_section "Dangerous Operations Check"
    ((TOTAL_CHECKS++))
    
    local dangerous_ops=()
    while IFS= read -r line; do
        [[ -n "$line" ]] && dangerous_ops+=("$line")
    done < <(detect_dangerous_ops "$migration_file" || true)
    
    if [[ ${#dangerous_ops[@]} -gt 0 ]]; then
        print_error "Found ${#dangerous_ops[@]} dangerous operation(s):"
        for op in "${dangerous_ops[@]:-}"; do
            echo -e "  ${RED}⚠${NC} $op"
            add_sql_problem "Dangerous: $op"
        done
    else
        print_success "No dangerous operations detected"
    fi
    
    # Check for missing safety clauses
    print_section "Safety Clause Check"
    ((TOTAL_CHECKS++))
    
    local missing_if_not_exists=()
    while IFS= read -r line; do
        [[ -n "$line" ]] && missing_if_not_exists+=("$line")
    done < <(check_if_not_exists_usage "$migration_file" || true)
    
    if [[ ${#missing_if_not_exists[@]} -gt 0 ]]; then
        print_warning "Found ${#missing_if_not_exists[@]} statement(s) without safety clauses"
        local count=0
        for item in "${missing_if_not_exists[@]:-}"; do
            if [[ $count -lt 5 ]]; then  # Only show first 5
                echo -e "  ${YELLOW}→${NC} ${item:0:100}..."
            fi
            ((count++))
        done
        if [[ $count -gt 5 ]]; then
            echo -e "  ${YELLOW}...${NC} and $((count - 5)) more"
        fi
        add_sql_problem "Missing IF NOT EXISTS clauses"
        print_suggestion "Add 'IF NOT EXISTS' to CREATE statements for idempotency"
    else
        print_success "All CREATE statements use IF NOT EXISTS"
    fi
    
    # Check INSERT conflict handling
    ((TOTAL_CHECKS++))
    local missing_conflict=()
    while IFS= read -r line; do
        [[ -n "$line" ]] && missing_conflict+=("$line")
    done < <(check_insert_conflicts "$migration_file" || true)
    
    if [[ ${#missing_conflict[@]} -gt 0 ]]; then
        print_warning "Found ${#missing_conflict[@]} INSERT(s) without conflict handling"
        print_suggestion "Add 'ON CONFLICT DO NOTHING' or 'DO UPDATE' to INSERT statements"
    else
        print_success "INSERT statements handle conflicts"
    fi
    
    # Check for anti-patterns
    print_section "SQL Best Practices Check"
    ((TOTAL_CHECKS++))
    
    local antipatterns=()
    while IFS= read -r line; do
        [[ -n "$line" ]] && antipatterns+=("$line")
    done < <(check_antipatterns "$migration_file" || true)
    
    if [[ ${#antipatterns[@]} -gt 0 ]]; then
        print_warning "Found ${#antipatterns[@]} potential issue(s):"
        for pattern in "${antipatterns[@]:-}"; do
            echo -e "  ${YELLOW}→${NC} $pattern"
            print_suggestion "Review: $pattern"
        done
    else
        print_success "No common anti-patterns detected"
    fi
    
    # Check for unsafe NOT NULL additions
    print_section "Unsafe ALTER COLUMN Check"
    ((TOTAL_CHECKS++))
    
    local unsafe_not_null=()
    while IFS= read -r line; do
        [[ -n "$line" ]] && unsafe_not_null+=("$line")
    done < <(check_unsafe_not_null "$migration_file" || true)
    
    if [[ ${#unsafe_not_null[@]} -gt 0 ]]; then
        print_error "Found ${#unsafe_not_null[@]} unsafe NOT NULL constraint(s):"
        for issue in "${unsafe_not_null[@]:-}"; do
            echo -e "  ${RED}✗${NC} $issue"
            add_sql_problem "$issue"
        done
        print_suggestion "Wrap ALTER COLUMN SET NOT NULL in conditional block checking column exists"
        print_suggestion "Ensure UPDATE runs before SET NOT NULL, or will fail on missing column"
    else
        print_success "ALTER COLUMN operations are safe"
    fi
    
    # ========================================================================
    # PHASE 2: DATABASE CONNECTIVITY
    # ========================================================================
    
    print_header "Phase 2: Database Connectivity"
    
    print_section "Connection Test"
    ((TOTAL_CHECKS++))
    
    if check_db_exists; then
        print_success "Database '$DB_NAME' is accessible"
    else
        print_error "Cannot connect to database '$DB_NAME'"
        echo -e "\n${RED}${BOLD}Cannot proceed with database checks.${NC}"
        echo -e "Fix SQL file issues above, then ensure database is running.\n"
        exit 1
    fi
    
    # Database health
    local db_size=$(query_db "SELECT pg_size_pretty(pg_database_size('$DB_NAME'));")
    local table_count=$(query_db "SELECT count(*) FROM information_schema.tables WHERE table_schema = 'public';")
    local active_connections=$(query_db "SELECT count(*) FROM pg_stat_activity WHERE datname = '$DB_NAME';")
    
    print_info "Database size: ${BOLD}$db_size${NC}"
    print_info "Existing tables: ${BOLD}$table_count${NC}"
    print_info "Active connections: ${BOLD}$active_connections${NC}"
    
    # ========================================================================
    # PHASE 3: IMPACT ANALYSIS
    # ========================================================================
    
    print_header "Phase 3: Migration Impact Analysis"
    
    # Analyze table creation impact
    print_section "Table Creation Impact"
    
    for table in "${tables[@]}"; do
        [[ -z "$table" ]] && continue
        ((TOTAL_CHECKS++))
        
        if table_exists "$table"; then
            local row_count=$(get_row_count "$table")
            local existing_cols=$(get_table_columns "$table")
            
            print_warning "Table '$table' already exists"
            print_info "  Current state: $row_count row(s)"
            print_info "  Columns: $existing_cols"
            print_info "  Impact: CREATE will be skipped (IF NOT EXISTS)"
        else
            print_success "Table '$table' will be created fresh"
            print_info "  Impact: New table, no data affected"
        fi
    done
    
    # Analyze column additions
    if [[ ${#do_columns[@]} -gt 0 ]]; then
        print_section "Column Addition Impact"
        
        for entry in "${do_columns[@]}"; do
            [[ -z "$entry" ]] && continue
            ((TOTAL_CHECKS++))
            
            IFS=':' read -r table column col_type <<< "$entry"
            
            if ! table_exists "$table"; then
                print_info "Column '$column' → table '$table' (will be created)"
            elif column_exists "$table" "$column"; then
                local existing_type=$(get_column_type "$table" "$column")
                print_warning "Column '$table.$column' already exists"
                print_info "  Existing type: $existing_type"
                print_info "  Migration type: $col_type"
                
                if [[ "$existing_type" != "$(echo "$col_type" | tr '[:upper:]' '[:lower:]')" ]]; then
                    print_error "Type mismatch! Existing: $existing_type vs New: $col_type"
                    add_sql_problem "Column type mismatch: $table.$column"
                fi
                
                print_info "  Impact: ADD will be skipped"
            else
                local row_count=$(get_row_count "$table")
                print_success "Column '$table.$column' will be added"
                print_info "  Type: $col_type"
                print_info "  Impact: $row_count existing row(s) will get this column"
                
                # Check if nullable or has default
                if grep -iq "NOT NULL" "$migration_file" && grep -iq "$column" "$migration_file"; then
                    if ! grep -iq "DEFAULT" "$migration_file"; then
                        print_error "Adding NOT NULL column without DEFAULT to table with $row_count rows"
                        print_suggestion "Add DEFAULT value or make column nullable initially"
                    fi
                fi
            fi
        done
    fi
    
    # Analyze index creation
    if [[ ${#indexes[@]} -gt 0 ]]; then
        print_section "Index Creation Impact"
        
        for entry in "${indexes[@]}"; do
            [[ -z "$entry" ]] && continue
            ((TOTAL_CHECKS++))
            
            IFS=':' read -r table index <<< "$entry"
            
            if index_exists "$index"; then
                print_warning "Index '$index' already exists"
                print_info "  Impact: CREATE will be skipped"
            else
                if table_exists "$table"; then
                    local row_count=$(get_row_count "$table")
                    print_success "Index '$index' will be created on '$table'"
                    print_info "  Impact: Will index $row_count existing row(s)"
                    
                    if [[ $row_count -gt 100000 ]]; then
                        print_warning "Large table ($row_count rows) - index creation may take time"
                        print_suggestion "Consider creating index CONCURRENTLY to avoid blocking"
                    fi
                else
                    print_success "Index '$index' will be created on new table '$table'"
                    print_info "  Impact: No existing data to index"
                fi
            fi
        done
    fi
    
    # Analyze foreign key impact
    local fk_lines=($(extract_foreign_keys "$migration_file"))
    
    if [[ ${#fk_lines[@]} -gt 0 ]]; then
        print_section "Foreign Key Impact"
        
        print_info "Found ${#fk_lines[@]} foreign key definition(s)"
        
        # Extract referenced tables
        for fk_line in "${fk_lines[@]}"; do
            local ref_table=$(echo "$fk_line" | grep -oiE "REFERENCES[[:space:]]+[a-z_]+" | awk '{print $2}')
            
            if [[ -n "$ref_table" ]] && ! table_exists "$ref_table"; then
                ((TOTAL_CHECKS++))
                if [[ " ${tables[@]} " =~ " ${ref_table} " ]]; then
                    print_warning "Referenced table '$ref_table' created in same migration"
                    print_suggestion "Ensure '$ref_table' is created before foreign key definition"
                else
                    print_error "Referenced table '$ref_table' does not exist"
                    add_sql_problem "Missing referenced table: $ref_table"
                fi
            fi
        done
    fi
    
    # Check for locks
    print_section "Lock & Transaction Check"
    ((TOTAL_CHECKS++))
    
    local locks=$(query_db "SELECT count(*) FROM pg_locks WHERE granted = false;")
    if [[ "$locks" -gt 0 ]]; then
        print_warning "Found $locks blocked lock(s) in database"
        print_suggestion "Wait for locks to clear before migration"
    else
        print_success "No blocking locks detected"
    fi
    
    ((TOTAL_CHECKS++))
    local long_running=$(query_db "SELECT count(*) FROM pg_stat_activity WHERE state = 'active' AND now() - query_start > interval '5 minutes';")
    if [[ "$long_running" -gt 0 ]]; then
        print_warning "Found $long_running long-running transaction(s)"
        print_suggestion "Wait for these to complete before migration"
    else
        print_success "No long-running transactions"
    fi
    
    # ========================================================================
    # PHASE 4: RECOMMENDATIONS
    # ========================================================================
    
    print_header "Phase 4: Recommendations & Next Steps"
    
    print_section "Backup Recommendation"
    
    local backup_file="backup_$(basename "$migration_file" .sql)_$(date +%Y%m%d_%H%M%S).sql"
    
    print_info "${BOLD}Before running this migration:${NC}"
    echo -e "  ${GREEN}1.${NC} Create backup:"
    echo -e "     ${CYAN}pg_dump -h $DB_HOST -U $DB_USER -d $DB_NAME > $backup_file${NC}"
    echo -e "  ${GREEN}2.${NC} Test in a transaction (dry run):"
    echo -e "     ${CYAN}psql -h $DB_HOST -U $DB_USER -d $DB_NAME -c 'BEGIN; \\i $migration_file; ROLLBACK;'${NC}"
    echo -e "  ${GREEN}3.${NC} Apply migration:"
    echo -e "     ${CYAN}psql -h $DB_HOST -U $DB_USER -d $DB_NAME -f $migration_file${NC}"
    
    # SQL improvements
    if [[ ${#SUGGESTIONS_LIST[@]} -gt 0 ]]; then
        print_section "Suggested SQL Improvements"
        
        for suggestion in "${SUGGESTIONS_LIST[@]}"; do
            echo -e "  ${MAGENTA}💡${NC} $suggestion"
        done
    fi
    
    # ========================================================================
    # FINAL SUMMARY
    # ========================================================================
    
    print_header "Validation Summary"
    
    echo -e "${BOLD}Migration File:${NC} $(basename "$migration_file")"
    echo -e "${BOLD}Total Checks:${NC} $TOTAL_CHECKS"
    echo -e "${GREEN}${BOLD}✓ Passed:${NC} $PASSED_CHECKS"
    echo -e "${YELLOW}${BOLD}⚠ Warnings:${NC} $WARNING_COUNT"
    echo -e "${RED}${BOLD}✗ Errors:${NC} $FAILED_CHECKS"
    echo -e "${MAGENTA}${BOLD}💡 Suggestions:${NC} $SUGGESTIONS"
    
    echo ""
    
    # Detailed problem summary
    if [[ ${#SQL_PROBLEMS[@]} -gt 0 ]]; then
        echo -e "${RED}${BOLD}SQL Problems Detected:${NC}"
        for problem in "${SQL_PROBLEMS[@]}"; do
            echo -e "  ${RED}•${NC} $problem"
        done
        echo ""
    fi
    
    # Final verdict
    if [[ $FAILED_CHECKS -eq 0 ]]; then
        if [[ $WARNING_COUNT -eq 0 ]]; then
            echo -e "${GREEN}${BOLD}✓✓✓ MIGRATION READY ✓✓✓${NC}"
            echo -e "${GREEN}${BOLD}All checks passed! Safe to apply.${NC}\n"
            exit 0
        else
            echo -e "${YELLOW}${BOLD}⚠ MIGRATION READY WITH WARNINGS ⚠${NC}"
            echo -e "${YELLOW}${BOLD}Review warnings above. Migration should be safe.${NC}\n"
            exit 0
        fi
    else
        echo -e "${RED}${BOLD}✗✗✗ MIGRATION NOT READY ✗✗✗${NC}"
        echo -e "${RED}${BOLD}Fix ${FAILED_CHECKS} error(s) before applying.${NC}\n"
        
        if [[ ${#BLOCKING_ISSUES[@]} -gt 0 ]]; then
            echo -e "${RED}${BOLD}Blocking Issues:${NC}"
            for issue in "${BLOCKING_ISSUES[@]}"; do
                echo -e "  ${RED}1.${NC} $issue"
            done
            echo ""
        fi
        
        exit 1
    fi
}

# ============================================================================
# Script Entry Point
# ============================================================================

main "$@"