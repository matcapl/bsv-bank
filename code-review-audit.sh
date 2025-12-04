#!/bin/bash
# BSV Bank - Code Review & Cleanup Audit v2.0
# Production-grade service analysis with comprehensive error handling

set -euo pipefail

# ============================================================================
# CONFIGURATION & CONSTANTS
# ============================================================================

readonly SCRIPT_VERSION="2.0.0"
readonly SCRIPT_NAME="$(basename "$0")"

# Default configuration (can be overridden via env vars)
readonly CORE_DIR="${BSV_CORE_DIR:-core}"
readonly REPORT_DIR="${BSV_REPORT_DIR:-audit-reports}"
readonly PORTS_FILE="${BSV_PORTS_FILE:-ports.md}"

# Thresholds (configurable via env vars)
readonly MIN_LINES_PLACEHOLDER="${BSV_MIN_LINES:-100}"
readonly MIN_COMMITS_ACTIVE="${BSV_MIN_COMMITS:-5}"
readonly MIN_ENDPOINTS_ACTIVE="${BSV_MIN_ENDPOINTS:-1}"
readonly WARN_TODO_COUNT="${BSV_WARN_TODOS:-3}"
readonly WARN_PANIC_COUNT="${BSV_WARN_PANICS:-5}"
readonly MIN_LINES_COMPLETE="${BSV_MIN_LINES_COMPLETE:-50}"

# Color codes
if [[ -t 1 ]] && [[ "${NO_COLOR:-}" != "1" ]]; then
    readonly RED='\033[0;31m'
    readonly YELLOW='\033[1;33m'
    readonly GREEN='\033[0;32m'
    readonly BLUE='\033[0;34m'
    readonly GRAY='\033[0;90m'
    readonly NC='\033[0m'
else
    readonly RED=''
    readonly YELLOW=''
    readonly GREEN=''
    readonly BLUE=''
    readonly GRAY=''
    readonly NC=''
fi

# Global state
VERBOSE=0
DRY_RUN=0
JSON_OUTPUT=0
ERRORS_FOUND=0

# ============================================================================
# HELPER FUNCTIONS
# ============================================================================

log_error() {
    echo -e "${RED}ERROR:${NC} $*" >&2
    ((ERRORS_FOUND++)) || true
}

log_warn() {
    echo -e "${YELLOW}WARNING:${NC} $*" >&2
}

log_info() {
    echo -e "${BLUE}INFO:${NC} $*"
}

log_success() {
    echo -e "${GREEN}✓${NC} $*"
}

log_verbose() {
    [[ $VERBOSE -eq 1 ]] && echo -e "${GRAY}[VERBOSE]${NC} $*"
}

show_help() {
    cat << EOF
Usage: $SCRIPT_NAME [OPTIONS]

Code review and cleanup audit for BSV Bank services.

OPTIONS:
    -h, --help              Show this help message
    -v, --verbose           Enable verbose output
    -V, --version           Show version information
    -d, --dry-run           Preview actions without writing files
    -j, --json              Output in JSON format
    --no-color              Disable colored output
    --core-dir DIR          Core services directory (default: core/)
    --report-dir DIR        Report output directory (default: audit-reports/)

ENVIRONMENT VARIABLES:
    BSV_CORE_DIR            Override default core directory
    BSV_REPORT_DIR          Override default report directory
    BSV_MIN_LINES           Minimum lines for non-placeholder (default: 100)
    BSV_MIN_COMMITS         Minimum commits for active service (default: 5)
    NO_COLOR                Disable colored output

EXAMPLES:
    $SCRIPT_NAME                    # Run with defaults
    $SCRIPT_NAME --verbose          # Show detailed progress
    $SCRIPT_NAME --json             # Output JSON for CI/CD
    $SCRIPT_NAME --dry-run          # Preview without creating reports

EOF
}

show_version() {
    echo "$SCRIPT_NAME version $SCRIPT_VERSION"
}

# ============================================================================
# VALIDATION FUNCTIONS
# ============================================================================

validate_environment() {
    local valid=1
    
    log_verbose "Validating environment..."
    
    # Check we're in a valid directory
    if [[ ! -d "$CORE_DIR" ]]; then
        log_error "Core directory not found: $CORE_DIR"
        log_error "Are you running from the project root?"
        valid=0
    fi
    
    # Check for git repository
    if ! git rev-parse --git-dir > /dev/null 2>&1; then
        log_warn "Not a git repository - commit history will be unavailable"
    fi
    
    # Verify report directory can be created
    if [[ $DRY_RUN -eq 0 ]]; then
        if ! mkdir -p "$REPORT_DIR" 2>/dev/null; then
            log_error "Cannot create report directory: $REPORT_DIR"
            valid=0
        fi
    fi
    
    # Count services
    local service_count=0
    if [[ -d "$CORE_DIR" ]]; then
        service_count=$(find "$CORE_DIR" -maxdepth 1 -type d -name "[!.]*" 2>/dev/null | wc -l | tr -d ' ')
    fi
    
    if [[ $service_count -eq 0 ]]; then
        log_warn "No services found in $CORE_DIR"
    else
        log_verbose "Found $service_count potential services"
    fi
    
    if [[ $valid -eq 0 ]]; then
        log_error "Environment validation failed"
        return 1
    fi
    
    log_verbose "Environment validation passed"
    return 0
}

# Safe stat wrapper for cross-platform compatibility
safe_stat_mtime() {
    local file=$1
    if [[ ! -f "$file" ]]; then
        echo "unknown"
        return
    fi
    
    # Try BSD stat (macOS)
    if stat -f "%Sm" -t "%Y-%m-%d" "$file" 2>/dev/null; then
        return
    fi
    
    # Try GNU stat (Linux)
    if stat -c "%y" "$file" 2>/dev/null | cut -d' ' -f1; then
        return
    fi
    
    # Fallback to git
    if git rev-parse --git-dir > /dev/null 2>&1; then
        git log -1 --format="%cs" "$file" 2>/dev/null || echo "unknown"
    else
        echo "unknown"
    fi
}

# Safe commit count
get_commit_count() {
    local file=$1
    if ! git rev-parse --git-dir > /dev/null 2>&1; then
        echo "0"
        return
    fi
    
    if [[ ! -f "$file" ]]; then
        echo "0"
        return
    fi
    
    git log --oneline "$file" 2>/dev/null | wc -l | tr -d ' ' || echo "0"
}

# Check if service is in startup scripts (exact name matching)
check_startup_usage() {
    local service_name=$1
    local found=0
    
    # Check start-all.sh with word boundaries
    if [[ -f "start-all.sh" ]] && grep -qw "$service_name" start-all.sh 2>/dev/null; then
        found=1
    fi
    
    # Check phase5 script with word boundaries
    if [[ -f "scripts/start-phase5-services.sh" ]] && grep -qw "$service_name" scripts/start-phase5-services.sh 2>/dev/null; then
        found=1
    fi
    
    echo "$found"
}

# ============================================================================
# ANALYSIS FUNCTIONS
# ============================================================================

analyze_service() {
    local service_path=$1
    local service_name
    service_name=$(basename "$service_path")
    
    log_verbose "Analyzing service: $service_name"
    
    local output=""
    output+="━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n"
    output+="Service: $service_name\n"
    output+="Path: $service_path\n\n"
    
    # Validate Cargo.toml exists
    if [[ ! -f "$service_path/Cargo.toml" ]]; then
        output+="${GRAY}⊘ Not a Rust service (no Cargo.toml)${NC}\n\n"
        echo -e "$output"
        return
    fi
    
    local main_rs="$service_path/src/main.rs"
    
    # Validate main.rs exists
    if [[ ! -f "$main_rs" ]]; then
        output+="${RED}✗ No main.rs found${NC}\n"
        output+="Recommendation: ${GRAY}Remove or complete implementation${NC}\n\n"
        echo -e "$output"
        return
    fi
    
    # Metrics calculation
    local lines_of_code
    lines_of_code=$(wc -l < "$main_rs" 2>/dev/null | tr -d ' ')
    
    local endpoint_count=0
    # Count both .route() and attribute routes
    local route_macro_count
    local route_attr_count

    # On macOS, grep -c PATTERN file || echo 0 does not work the way you expect inside a command substitution, because grep -c never exits with a non-zero status if the file exists—even if the pattern is not found.
    # route_macro_count=$(grep -c "\.route(" "$main_rs" 2>/dev/null || echo 0)
    # route_attr_count=$(grep -cE "#\[(get|post|put|delete|patch)" "$main_rs" 2>/dev/null || echo 0)
    route_macro_count=$(grep -c "\.route(" "$main_rs" 2>/dev/null || printf "0")
    route_macro_count=$(printf "%s" "$route_macro_count" | tr -d '\n' | tr -d '\r')

    route_attr_count=$(grep -cE "#\[(get|post|put|delete|patch)" "$main_rs" 2>/dev/null || printf "0")
    route_attr_count=$(printf "%s" "$route_attr_count" | tr -d '\n' | tr -d '\r')

    endpoint_count=$((route_macro_count + route_attr_count))
    
    local has_database=0
    if grep -qE "PgPool|sqlx|Database" "$main_rs" 2>/dev/null; then
        has_database=1
    fi
    
    local has_handlers=0
    if [[ -d "$service_path/src/handlers" ]] || grep -q "mod handlers" "$main_rs" 2>/dev/null; then
        has_handlers=1
    fi
    
    local last_modified
    last_modified=$(safe_stat_mtime "$main_rs")
    
    local commit_count
    commit_count=$(get_commit_count "$main_rs")
    
    local in_startup
    in_startup=$(check_startup_usage "$service_name")
    
    local port_assigned=0
    local assigned_port="none"
    if [[ -f "$PORTS_FILE" ]] && grep -qw "$service_name" "$PORTS_FILE" 2>/dev/null; then
        port_assigned=1
        assigned_port=$(grep -w "$service_name" "$PORTS_FILE" 2>/dev/null | grep -oE '[0-9]{4,5}' | head -1 || echo "unknown")
    fi
    
    # Determine status
    local status="UNKNOWN"
    local status_color="$GRAY"
    local reason=""
    
    if [[ $in_startup -eq 1 ]]; then
        status="ACTIVE"
        status_color="$GREEN"
        reason="In startup scripts"
    elif [[ $lines_of_code -lt $MIN_LINES_PLACEHOLDER ]] && [[ $endpoint_count -eq 0 ]]; then
        status="PLACEHOLDER"
        status_color="$GRAY"
        reason="Minimal code ($lines_of_code lines), no endpoints"
    elif [[ $commit_count -le 2 ]] && [[ $lines_of_code -lt 200 ]]; then
        status="PLACEHOLDER"
        status_color="$GRAY"
        reason="Few commits ($commit_count), minimal code"
    elif [[ $commit_count -gt $MIN_COMMITS_ACTIVE ]] && [[ $endpoint_count -ge $MIN_ENDPOINTS_ACTIVE ]]; then
        status="DEVELOPED"
        status_color="$BLUE"
        reason="Has commits ($commit_count) and endpoints ($endpoint_count), not in startup"
    else
        status="DORMANT"
        status_color="$YELLOW"
        reason="Has some code but not actively used"
    fi
    
    # Build output
    output+="Status: ${status_color}${status}${NC} - $reason\n\n"
    
    output+="Metrics:\n"
    output+="  Lines of Code: $lines_of_code\n"
    output+="  Endpoints: $endpoint_count\n"
    output+="  Has Database: $([ $has_database -eq 1 ] && echo "Yes" || echo "No")\n"
    output+="  Has Handlers: $([ $has_handlers -eq 1 ] && echo "Yes" || echo "No")\n"
    output+="  Git Commits: $commit_count\n"
    output+="  Last Modified: $last_modified\n\n"
    
    output+="Integration:\n"
    output+="  In startup scripts: $([ $in_startup -eq 1 ] && echo "✓ Yes" || echo "✗ No")\n"
    output+="  Port Assigned: $([ $port_assigned -eq 1 ] && echo "✓ $assigned_port" || echo "✗ No")\n\n"
    
    # Check for TODOs/FIXMEs (only in source files, not comments)
    local todos=0
    if [[ -d "$service_path/src/" ]]; then
        # More careful TODO counting - exclude commented lines
        todos=$(find "$service_path/src/" -type f -name "*.rs" -exec grep -E "^\s*(TODO|FIXME)" {} \; 2>/dev/null | wc -l | tr -d ' ')
    fi
    
    if [[ $todos -gt 0 ]]; then
        output+="${YELLOW}⚠ $todos TODO/FIXME items found${NC}\n"
    fi
    
    # Recommendation
    output+="Recommendation:\n"
    case $status in
        PLACEHOLDER)
            output+="  ${GRAY}→ Consider deleting or moving to /junk if not planned${NC}\n"
            ;;
        ACTIVE)
            output+="  ${GREEN}→ Keep and maintain - actively used${NC}\n"
            ;;
        DEVELOPED)
            output+="  ${BLUE}→ Add to startup scripts or document why disabled${NC}\n"
            ;;
        DORMANT)
            output+="  ${YELLOW}→ Review if still needed - has code but not used${NC}\n"
            ;;
    esac
    
    output+="\n"
    
    echo -e "$output"
}

# Check file completeness
check_file_completeness() {
    local file=$1
    local file_type=$2
    
    if [[ ! -f "$file" ]]; then
        return
    fi
    
    log_verbose "Checking completeness: $file"
    
    local output=""
    output+="Checking: $file\n"
    
    local lines
    lines=$(wc -l < "$file" 2>/dev/null | tr -d ' ')
    
    # Count actual TODOs (not in strings or comments)
    local has_todo
    has_todo=$(grep -cE "^\s*(//|/\*).*TODO|FIXME" "$file" 2>/dev/null || echo 0)
    
    # Count panics (excluding test code)
    local has_panic
    has_panic=$(grep -v "#\[test\]" "$file" 2>/dev/null | grep -cE "panic!|\.unwrap\(\)|\.expect\(" || echo 0)
    
    # Check for tests
    local has_tests
    has_tests=$(grep -cE "#\[test\]|#\[cfg\(test\)\]" "$file" 2>/dev/null || echo 0)
    
    output+="  Lines: $lines\n"
    
    # Type-specific checks
    case $file_type in
        main)
            local has_health
            has_health=$(grep -c "/health" "$file" 2>/dev/null || echo 0)
            local has_security_headers
            has_security_headers=$(grep -cE "X-Frame-Options|DefaultHeaders" "$file" 2>/dev/null || echo 0)
            
            output+="  Health Endpoint: $([ $has_health -gt 0 ] && echo "✓" || echo "✗")\n"
            output+="  Security Headers: $([ $has_security_headers -gt 0 ] && echo "✓" || echo "✗")\n"
            ;;
        middleware)
            local has_service_trait
            has_service_trait=$(grep -cE "Service|Transform" "$file" 2>/dev/null || echo 0)
            output+="  Implements Middleware: $([ $has_service_trait -gt 0 ] && echo "✓" || echo "✗")\n"
            ;;
        handler)
            local has_async
            has_async=$(grep -c "async fn" "$file" 2>/dev/null || echo 0)
            output+="  Async Handlers: $([ $has_async -gt 0 ] && echo "✓" || echo "✗")\n"
            ;;
    esac
    
    output+="  TODO/FIXME: $has_todo\n"
    output+="  Panic/Unwrap: $has_panic\n"
    output+="  Has Tests: $([ $has_tests -gt 0 ] && echo "✓ ($has_tests)" || echo "✗")\n"
    
    # Assessment
    local complete=1
    if [[ $lines -lt $MIN_LINES_COMPLETE ]]; then
        output+="  ${YELLOW}⚠ Short file - may be incomplete${NC}\n"
        complete=0
    fi
    
    if [[ $has_todo -gt $WARN_TODO_COUNT ]]; then
        output+="  ${YELLOW}⚠ Multiple TODOs ($has_todo) - needs completion${NC}\n"
        complete=0
    fi
    
    if [[ $has_panic -gt $WARN_PANIC_COUNT ]]; then
        output+="  ${YELLOW}⚠ Many panic/unwrap calls ($has_panic) - improve error handling${NC}\n"
        complete=0
    fi
    
    if [[ $complete -eq 1 ]]; then
        output+="  ${GREEN}✓ Appears complete${NC}\n"
    fi
    
    output+="\n"
    
    echo -e "$output"
}

# ============================================================================
# MAIN EXECUTION
# ============================================================================

main() {
    # Parse command line arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            -h|--help)
                show_help
                exit 0
                ;;
            -V|--version)
                show_version
                exit 0
                ;;
            -v|--verbose)
                VERBOSE=1
                shift
                ;;
            -d|--dry-run)
                DRY_RUN=1
                shift
                ;;
            -j|--json)
                JSON_OUTPUT=1
                shift
                ;;
            --no-color)
                NO_COLOR=1
                shift
                ;;
            --core-dir)
                CORE_DIR="$2"
                shift 2
                ;;
            --report-dir)
                REPORT_DIR="$2"
                shift 2
                ;;
            *)
                log_error "Unknown option: $1"
                show_help
                exit 1
                ;;
        esac
    done
    
    # Validate environment
    if ! validate_environment; then
        log_error "Environment validation failed"
        exit 1
    fi
    
    # Setup report file
    local report_file=""
    if [[ $DRY_RUN -eq 0 ]]; then
        mkdir -p "$REPORT_DIR"
        chmod 700 "$REPORT_DIR" 2>/dev/null || true
        report_file="$REPORT_DIR/code-review-$(date +%Y%m%d-%H%M%S).txt"
        touch "$report_file"
        chmod 600 "$report_file" 2>/dev/null || true
    fi
    
    # Header
    local output=""
    output+="════════════════════════════════════════════════════════════\n"
    output+="  BSV BANK - CODE REVIEW & CLEANUP AUDIT v$SCRIPT_VERSION\n"
    output+="════════════════════════════════════════════════════════════\n"
    output+="\nDate: $(date)\n"
    output+="Core Directory: $CORE_DIR\n"
    if [[ $DRY_RUN -eq 1 ]]; then
        output+="Mode: DRY RUN (no files will be written)\n"
    fi
    output+="\n"
    
    echo -e "$output"
    [[ -n "$report_file" ]] && echo -e "$output" >> "$report_file"
    
    # Section 1: Service Analysis
    output=""
    output+="════════════════════════════════════════════════════════════\n"
    output+="1. SERVICE INVENTORY & USAGE ANALYSIS\n"
    output+="════════════════════════════════════════════════════════════\n\n"
    
    echo -e "$output"
    [[ -n "$report_file" ]] && echo -e "$output" >> "$report_file"
    
    # Analyze services
    local service_count=0
    while IFS= read -r -d '' service_dir; do
        if [[ -f "$service_dir/Cargo.toml" ]]; then
            local analysis
            analysis=$(analyze_service "$service_dir")
            echo "$analysis"
            [[ -n "$report_file" ]] && echo "$analysis" >> "$report_file"
            ((service_count++)) || true
        fi
    done < <(find "$CORE_DIR" -maxdepth 1 -type d ! -name "." -print0 2>/dev/null)
    
    if [[ $service_count -eq 0 ]]; then
        log_warn "No services found to analyze"
    else
        log_success "Analyzed $service_count services"
    fi
    
    # Section 2: File Completeness
    output=""
    output+="════════════════════════════════════════════════════════════\n"
    output+="2. FILE COMPLETENESS ANALYSIS\n"
    output+="════════════════════════════════════════════════════════════\n\n"
    
    echo -e "$output"
    [[ -n "$report_file" ]] && echo -e "$output" >> "$report_file"
    
    # Check main.rs files
    output="━━━ Main.rs Files ━━━\n"
    echo -e "$output"
    [[ -n "$report_file" ]] && echo -e "$output" >> "$report_file"
    
    while IFS= read -r -d '' file; do
        local analysis
        analysis=$(check_file_completeness "$file" "main")
        echo "$analysis"
        [[ -n "$report_file" ]] && echo "$analysis" >> "$report_file"
    done < <(find "$CORE_DIR" -type f -name "main.rs" ! -path "*/target/*" -print0 2>/dev/null)
    
    # Summary
    output=""
    output+="════════════════════════════════════════════════════════════\n"
    output+="AUDIT COMPLETE\n"
    output+="════════════════════════════════════════════════════════════\n\n"
    
    if [[ -n "$report_file" ]]; then
        output+="Full report: $report_file\n\n"
    fi
    
    if [[ $ERRORS_FOUND -gt 0 ]]; then
        output+="${YELLOW}⚠ $ERRORS_FOUND errors/warnings found${NC}\n"
    else
        output+="${GREEN}✓ No errors found${NC}\n"
    fi
    
    echo -e "$output"
    [[ -n "$report_file" ]] && echo -e "$output" >> "$report_file"
    
    # Exit with appropriate code
    if [[ $ERRORS_FOUND -gt 0 ]]; then
        exit 1
    fi
    
    exit 0
}

# Run main function
main "$@"