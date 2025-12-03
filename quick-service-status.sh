#!/bin/bash
# Quick Service Status - One-line summary of each service

set -euo pipefail

echo "BSV Bank - Quick Service Status"
echo "═══════════════════════════════════════════════════════════════════════════════"
printf "%-25s %-8s %-8s %-10s %-12s %s\n" "SERVICE" "LINES" "ROUTES" "IN_START" "LAST_COMMIT" "STATUS"
echo "───────────────────────────────────────────────────────────────────────────────"

# Check we're in the right directory
if [ ! -d "core" ]; then
    echo "Error: core/ directory not found. Run from project root."
    exit 1
fi

safe_int_check() {
    local var="$1"
    # Strip newlines/CR, grab first run of digits, default to 0
    var=$(printf '%s' "$var" | tr -d '\n\r' | grep -oE '[0-9]+' | head -1 || true)
    echo "${var:-0}"
}

for service_dir in core/*/; do
    # Skip if not a directory
    [ -d "$service_dir" ] || continue
    
    # Skip if no Cargo.toml
    [ -f "$service_dir/Cargo.toml" ] || continue
    
    service=$(basename "$service_dir")
    main_rs="$service_dir/src/main.rs"
    
    # Skip if no main.rs
    [ -f "$main_rs" ] || continue
    
    # # Count lines (safe default to 0)
    # lines=$(wc -l < "$main_rs" 2>/dev/null | tr -d ' ' || echo "0")
    
    # # Count routes (safe default to 0)
    # routes=$(grep -c "\.route(" "$main_rs" 2>/dev/null || echo "0")

    # Count lines (safe default to 0)
    lines=$(wc -l < "$main_rs" 2>/dev/null || echo "0")

    # Count routes (safe default to 0)
    routes=$(grep -c "\.route(" "$main_rs" 2>/dev/null || echo "0")

    # Ensure numeric values
    lines=$(safe_int_check "$lines")
    routes=$(safe_int_check "$routes")
    
    # Check if in start scripts (exact word match)
    in_start="✗"
    if grep -qw "$service" start-all.sh 2>/dev/null || \
       grep -qw "$service" scripts/start-phase5-services.sh 2>/dev/null; then
        in_start="✓"
    fi
    
    # Last commit (safe handling)
    if git rev-parse --git-dir > /dev/null 2>&1; then
        last_commit=$(git log -1 --format="%ar" -- "$main_rs" 2>/dev/null | \
                      sed 's/ ago//' | cut -d' ' -f1-2 || echo "unknown")
    else
        last_commit="unknown"
    fi
    
    # Ensure numeric values
    lines=${lines:-0}
    routes=${routes:-0}
    
    # Determine status
    status="UNKNOWN"
    if [ "$in_start" = "✓" ]; then
        status="🟢 ACTIVE"
    elif [ "$lines" -lt 100 ] && [ "$routes" -eq 0 ]; then
        status="⚪ PLACEHOLDER"
    elif [ "$routes" -gt 0 ]; then
        status="🟡 DEVELOPED"
    else
        status="🔵 DORMANT"
    fi
    
    printf "%-25s %-8s %-8s %-10s %-12s %s\n" \
        "$service" "$lines" "$routes" "$in_start" "$last_commit" "$status"
done

echo "═══════════════════════════════════════════════════════════════════════════════"
echo ""
echo "Legend:"
echo "  🟢 ACTIVE      - In startup scripts, actively used"
echo "  🟡 DEVELOPED   - Has code/routes but not in startup"
echo "  🔵 DORMANT     - Has some code but not actively developed"
echo "  ⚪ PLACEHOLDER - Minimal code, likely not implemented"
echo ""
echo "Actions:"
echo "  • ACTIVE: Keep and maintain"
echo "  • DEVELOPED: Add to startup or document why separate"
echo "  • DORMANT: Review and decide (keep/delete/revive)"
echo "  • PLACEHOLDER: Move to /junk or delete if not planned"
echo ""