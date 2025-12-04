#!/bin/bash
# scripts/code-intelligence-audit.sh
# v2.4: Code + Redundancy Audit with LLM Prompt Generator (FULLY FIXED)
# Purpose: Find dead code, generate actionable LLM prompt for analysis

set -e

OUTPUT_DIR="audit-reports"
mkdir -p "$OUTPUT_DIR"

TIMESTAMP=$(date +%Y%m%d-%H%M%S)
REPORT="$OUTPUT_DIR/code-intelligence-audit-$TIMESTAMP.txt"

{
    echo "════════════════════════════════════════════════════════════════"
    echo "  BSV BANK - CODE INTELLIGENCE AUDIT (v2.4)"
    echo "  With: Duplication check + LLM Redundancy Prompt"
    echo "════════════════════════════════════════════════════════════════"
    echo ""
    echo "Date: $(date)"
    echo "Repository: $(pwd)"
    echo "Scope: Untracked .rs files in core/*/src/ (excluding build artifacts)"
    echo ""
    
    # ===================================================================
    echo "1. UNTRACKED SOURCE FILES ANALYSIS"
    echo "═══════════════════════════════════════════════════════════════"
    echo ""
    
    UNTRACKED=$(git ls-files --others --exclude-standard -- 'core/*/src/*.rs' 'core/common/src/*.rs' 2>/dev/null || true)
    
    if [ -z "$UNTRACKED" ]; then
        echo "✓ No untracked .rs files found in core/*/src/"
    else
        echo "Found untracked source files:"
        echo ""
        
        TOTAL_FILES=$(echo "$UNTRACKED" | wc -l)
        
        echo "$UNTRACKED" | while read -r file; do
            if [ -f "$file" ]; then
                lines=$(wc -l < "$file")
                functions=$(grep -E "^\s*(pub\s+)?(async\s+)?(fn|struct|enum|impl)\s+[a-zA-Z_]" "$file" 2>/dev/null | wc -l)
                todos=$(grep -cE "TODO|FIXME|XXX|unimplemented" "$file" 2>/dev/null || true)
                
                filename_no_ext="${file%.*}"
                module_name=$(basename "$filename_no_ext")
                
                if grep -r "use.*$module_name\|mod\s*$module_name" core/ --include="*.rs" --exclude-dir=target 2>/dev/null | grep -v "^$file:" >/dev/null 2>&1; then
                    import_status="✅ USED"
                else
                    import_status="⚠️  ORPHANED"
                fi
                
                echo "📄 $file"
                echo "   Size: $lines lines | Functions/Types: $functions | TODOs: $todos | Status: $import_status"
                echo ""
            fi
        done
        
        echo "Total untracked files: $TOTAL_FILES"
    fi
    
    echo ""
    echo ""
    
    # ===================================================================
    echo "2. DECISION GUIDE"
    echo "═══════════════════════════════════════════════════════════════"
    echo ""
    
    echo "For each untracked file above:"
    echo ""
    echo "✅ KEEP & git add IF:"
    echo "   • Status: USED (already imported)"
    echo "   • Size: >30 lines with real code"
    echo "   • No TODOs (or only 1-2 minor)"
    echo ""
    echo "⚠️  ANALYZE WITH LLM IF:"
    echo "   • Status: ORPHANED"
    echo "   • Size: 10-100 lines (substantive code)"
    echo "   • ← Use Section 3 below for LLM prompt"
    echo ""
    echo "❌ DISCARD IF:"
    echo "   • Size: <10 lines (placeholder)"
    echo "   • Status: ORPHANED + many TODOs"
    echo "   • Only comments/examples"
    echo ""
    
    echo ""
    echo ""
    
    # ===================================================================
    echo "3. ORPHANED FILES (candidates for consolidation or removal)"
    echo "═══════════════════════════════════════════════════════════════"
    echo ""
    
    find core/*/src -name "*.rs" -type f 2>/dev/null | while read -r file; do
        if git ls-files --cached --error-unmatch "$file" >/dev/null 2>&1; then
            continue
        fi
        
        filename_no_ext="${file%.*}"
        module_name=$(basename "$filename_no_ext")
        
        if [[ "$module_name" == "main" ]] || [[ "$module_name" == "lib" ]]; then
            continue
        fi
        
        refs=$(grep -r "use.*$module_name\|mod\s*$module_name" core/ --include="*.rs" --exclude-dir=target 2>/dev/null | grep -v "^$file:" | wc -l)
        
        if [ "$refs" -eq 0 ]; then
            lines=$(wc -l < "$file")
            if [ "$lines" -gt 5 ]; then
                echo "  ⚠️  $file ($lines lines, 0 refs)"
            fi
        fi
    done
    
    echo ""
    echo ""
    
    # ===================================================================
    echo "4. LLM PROMPT: ANALYZE FOR REDUNDANCY & CONSOLIDATION"
    echo "═══════════════════════════════════════════════════════════════"
    echo ""
    
    echo "Copy everything between the markers to your LLM."
    echo "The LLM will identify overlaps, consolidation opportunities, and decisions."
    echo ""
    
    echo "---START PROMPT---"
    echo ""
    echo "Review these Rust source files from my BSV Bank project."
    echo "Analyze for:"
    echo ""
    echo "1. REDUNDANCY - Functions/logic duplicated across files"
    echo "2. CONSOLIDATION - What should move to core/common/src/"
    echo "3. INTEGRATION - Which modules should be imported vs. merged"
    echo "4. REMOVAL - What's dead code or placeholder"
    echo ""
    echo "Project architecture:"
    echo "  • core/common/src/ = Shared utilities (validation, auth, logging, metrics, errors)"
    echo "  • core/*/src/main.rs = Service entry points (should import from common, add business logic)"
    echo ""
    echo "Untracked files to analyze:"
    echo ""
    
    echo "$UNTRACKED" | while read -r file; do
        if [ -f "$file" ]; then
            lines=$(wc -l < "$file")
            echo ""
            echo "FILE: $file ($lines lines)"
            echo ""
            echo "\`\`\`rust"
            if [ "$lines" -le 60 ]; then
                cat "$file"
            else
                echo "[... FIRST 30 LINES ...]"
                head -30 "$file"
                echo ""
                line_count=$(wc -l < "$file")
                omitted=$((line_count - 40))
                echo "[... ($omitted lines omitted) ...]"
                echo ""
                echo "[... LAST 10 LINES ...]"
                tail -10 "$file"
            fi
            echo "\`\`\`"
            echo ""
        fi
    done
    
    echo "---END PROMPT---"
    echo ""
    
    echo "After LLM analysis, it will recommend:"
    echo ""
    echo "  ✅ KEEP & INTEGRATE: File is useful → git add + import"
    echo "  ⚠️  REFACTOR: Consolidate into common/ → merge files"
    echo "  ❌ DISCARD: Dead code/placeholder → rm file"
    echo ""
    
    echo ""
    echo "════════════════════════════════════════════════════════════════"
    echo "End of report"
    echo "════════════════════════════════════════════════════════════════"

} | tee "$REPORT"

echo ""
echo "✓ Report saved to: $REPORT"
echo ""
echo "📖 Read LLM_PROMPT_GUIDE.md for workflow"
echo ""
echo "Next: Find Section 4 in report, copy prompt to your LLM"
