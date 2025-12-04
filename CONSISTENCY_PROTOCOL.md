# Session-to-Session Consistency Protocol

**Purpose:** Keep your architecture consistent across multiple LLM conversations  
**Status:** Deploy this immediately (before Week 2 work)

---

## Problem You're Solving

**Session 1 (Today):**
```
You: "Build the common library with validation"
LLM: ✅ Builds services/common with 77 tests
Result: validation in common, services/deposit-service minimal
```

**Session 2 (Tomorrow):** 
```
You: "Add lending-service"
Different LLM: "For faster iteration, let's add validation directly in lending-service"
Result: validation is now BOTH in common AND in lending-service
Consequence: Duplication. Maintenance nightmare. Backwards movement.
```

**How to prevent:** This protocol.

---

## The Three-Part System

### Part 1: Architecture Anchor (CANONICAL.md)

Create this file at repo root. **Do not modify it.** Reference it every session.

```markdown
# CANONICAL ARCHITECTURE

Last committed: 2025-12-03  
Status: LOCKED (follow exactly, don't deviate)

## File Structure (Immutable)

services/common/src/
  ├── lib.rs (exports: validation, logging, metrics, health, auth, error, rate_limit)
  ├── validation.rs (paymail, txid, amount, address, sql-injection, xss patterns)
  ├── logging.rs (init_logging, LogContext, structured JSON output)
  ├── metrics.rs (ServiceMetrics, Prometheus registry)
  ├── health.rs (health_check, liveness_check, readiness_check)
  ├── auth.rs (JwtManager, token generation/verification/refresh)
  ├── error.rs (ServiceError, error response mapping)
  └── rate_limit.rs (RateLimiter, sliding window algorithm)

services/[service-name]/src/
  └── main.rs (ONLY: imports from common + service-specific handlers)
       ├── No validation logic (use common)
       ├── No error types (use common)
       ├── No logging setup (use common)
       ├── No rate limiting impl (use common)
       └── No health checks impl (use common)

## Change Protocol

To modify this architecture:
1. Document decision in CHANGE_LOG.md with reasoning
2. Get explicit confirmation you want to deviate
3. Update both CANONICAL.md AND all affected services
4. Tag commit as "ARCHITECTURE-CHANGE"

DO NOT make incremental deviations across sessions.
```

### Part 2: Session Consistency Checklist

**Start every session with this.** Include it in your initial prompt to LLM.

```markdown
# Session Startup Checklist

Before we start coding:

## Architecture Verification
- [ ] I reviewed CANONICAL.md (architecture anchor)
- [ ] My task is: [describe task]
- [ ] Does this task require architecture changes? [YES/NO]
  - If YES: I understand it requires updating CANONICAL.md
  - If NO: I will follow existing architecture exactly

## Code Duplication Check
Before accepting any code suggestion, ask:
- [ ] Is this logic already in services/common/? (grep check)
  - If YES: Reject suggestion, import from common instead
  - If NO: Is it generic (used by 2+ services)?
    - If YES: Add to common, export from common
    - If NO: Add to service main.rs

## Import Discipline
When coding services/[X]/src/main.rs:
```
✅ REQUIRED imports:
use bsv_bank_common::{validate_*, init_logging, ServiceMetrics, JwtManager, *};

❌ FORBIDDEN patterns:
fn validate_paymail() { ... }  // Don't duplicate
fn log_action() { ... }        // Don't duplicate
impl ServiceError { ... }      // Don't duplicate
```

## Version Control Anchor
At end of session, commit with message:
```
git commit -m "Phase 6 Week 2: [specific work] - Architecture: CANONICAL.md"
```

This tags that session work against the canonical architecture.
```

### Part 3: Session Handoff Script

**Before finishing a session, run this.** It gives the next session clear state.

```bash
#!/bin/bash
# scripts/session-handoff.sh

echo "=== PHASE 6 SESSION HANDOFF ==="
echo ""
echo "1. Architecture Status"
echo "   Reference: CANONICAL.md"
echo "   Last verified: $(git log -1 --format=%ai | cut -d' ' -f1)"
echo ""

echo "2. Code Duplication Check"
echo "   Validation functions in common:"
grep -c "pub fn validate_" services/common/src/validation.rs
echo "   Validation functions in services (should be 0):"
for svc in lending-service payment-channel-service blockchain-monitor transaction-builder spv-service; do
  count=$(grep -c "fn validate_" services/$svc/src/main.rs 2>/dev/null || echo "0")
  if [ "$count" -gt "0" ]; then
    echo "   ⚠️  $svc: $count validation functions (possible duplication!)"
  fi
done
echo ""

echo "3. Imports Check"
echo "   Services importing from common (should be 6):"
grep -l "use bsv_bank_common::" services/*/src/main.rs 2>/dev/null | wc -l
echo ""

echo "4. Test Coverage"
echo "   Common library tests:"
cd services/common && cargo test --lib 2>&1 | grep -E "test result:|running"
echo ""

echo "5. Next Session Action Items"
if [ -f PHASE6_TODO.md ]; then
  echo "   $(head -5 PHASE6_TODO.md)"
else
  echo "   Update PHASE6_TODO.md with next steps"
fi

echo ""
echo "=== HANDOFF COMPLETE ==="
echo "Share this output with next session."
```

Run it like this:
```bash
chmod +x scripts/session-handoff.sh
./scripts/session-handoff.sh > SESSION_HANDOFF_$(date +%Y%m%d).log

# Include log in next session prompt
```

---

## Session-Specific Prompt Template

**Copy this into every LLM session. Customize the "Current Focus" section.**

```
# BSV Bank Phase 6 - Session Prompt

## Architecture Constraints (IMMUTABLE)
All shared code lives in services/common/:
- ✅ Validation (paymail, txid, amount, address)
- ✅ Logging (init_logging, structured JSON)
- ✅ Metrics (Prometheus, ServiceMetrics)
- ✅ Health checks (liveness, readiness)
- ✅ Authentication (JWT, JwtManager)
- ✅ Errors (ServiceError, response mapping)
- ✅ Rate limiting (sliding window, RateLimiter)

Each service imports these, doesn't duplicate them.

Reference: services/common/src/lib.rs (see pub use statements)

## What NOT to Do
❌ Don't add validation functions to service main.rs files
❌ Don't redefine error types
❌ Don't duplicate logging setup
❌ Don't suggest "local optimization" that breaks architecture

If you suggest something that duplicates common, I'll push back.

## Current Focus
[Describe today's task]

Example: "Add lending-service following Phase 6 architecture template"

## Previous Session State
[Paste SESSION_HANDOFF_YYYYMMDD.log here]

## Specific Question
[Your question]

---

Begin by confirming:
1. You've reviewed the architecture constraints
2. You understand the task doesn't require deviating from them
3. You're ready to follow the template pattern
```

---

## The Weekly Checklist

**Every Friday, run this to verify you haven't drifted:**

```bash
#!/bin/bash
# scripts/architecture-audit.sh

SERVICES=("deposit-service" "lending-service" "payment-channel-service" \
          "blockchain-monitor" "transaction-builder" "spv-service")

echo "=== ARCHITECTURE AUDIT ==="
echo ""

VIOLATIONS=0

# Check 1: No duplicate validation
echo "Check 1: Validation functions not duplicated"
for svc in "${SERVICES[@]}"; do
  if [ -d "services/$svc" ]; then
    count=$(grep -c "fn validate_paymail\|fn validate_txid\|fn validate_amount" \
            services/$svc/src/main.rs 2>/dev/null || echo "0")
    if [ "$count" -gt "0" ]; then
      echo "  ❌ $svc: VIOLATION ($count functions)"
      ((VIOLATIONS++))
    fi
  fi
done
[ "$VIOLATIONS" -eq "0" ] && echo "  ✅ All clean"
echo ""

# Check 2: All services import from common
echo "Check 2: Services importing from common"
for svc in "${SERVICES[@]}"; do
  if [ -d "services/$svc" ]; then
    if grep -q "use bsv_bank_common::" services/$svc/src/main.rs 2>/dev/null; then
      echo "  ✅ $svc"
    else
      echo "  ❌ $svc: NO IMPORTS"
      ((VIOLATIONS++))
    fi
  fi
done
echo ""

# Check 3: Common library is exporting everything
echo "Check 3: Common library exports"
exports=$(grep -c "pub use" services/common/src/lib.rs)
echo "  Found $exports pub use statements"
if [ "$exports" -lt "7" ]; then
  echo "  ⚠️  Less than expected (should be ~7+)"
  ((VIOLATIONS++))
fi
echo ""

if [ "$VIOLATIONS" -eq "0" ]; then
  echo "✅ ARCHITECTURE AUDIT PASSED"
  echo "Safe to commit."
else
  echo "❌ VIOLATIONS FOUND: $VIOLATIONS"
  echo "Fix before committing."
  exit 1
fi
```

Usage:
```bash
chmod +x scripts/architecture-audit.sh
./scripts/architecture-audit.sh
```

---

## Deployment Steps (Right Now)

1. **Create CANONICAL.md** at repo root (content provided above)
2. **Create PHASE6_TODO.md** listing next steps
3. **Add session-handoff.sh** to scripts/
4. **Add architecture-audit.sh** to scripts/
5. **Commit everything:**
   ```bash
   git add CANONICAL.md PHASE6_TODO.md scripts/
   git commit -m "docs: Add Phase 6 architecture consistency protocol"
   ```

6. **Use the session prompt in next LLM session**

---

## Resolving Conflicts Mid-Session

**If an LLM suggests something that violates CANONICAL.md:**

You: 
> "That suggests duplicating validation in lending-service, but CANONICAL.md specifies all validation lives in common. Either:
> 1. Import validate_paymail from common, or
> 2. Add new validation logic to common, export it, then import
> 
> Which makes sense for this use case?"

This forces the LLM back to your architecture instead of suggesting deviations.

---

## Success Metrics

By end of Phase 6 Week 2:

```
$ ./scripts/architecture-audit.sh
✅ ARCHITECTURE AUDIT PASSED

$ grep -r "fn validate_" services/lending-service services/payment-channel-service ... 
(no matches - all validation is in common)

$ git log --oneline | grep ARCHITECTURE
(only one: initial setup, no mid-course corrections needed)
```

This means **consistent architecture across all sessions, zero duplicated code, forward momentum only.**

---

## What This Prevents

| Without This | With This |
|---|---|
| Session 1: Validation in common | ✅ Validation in common |
| Session 2: "Add validation to lending" | ✅ "Import validation from common" |
| Session 3: Fix bug in 2 validation implementations | ✅ Fix bug in 1 place |
| Session 4: "Why is lending stricter than payment-channels?" | ✅ Same validation everywhere |
| Final: 3x maintenance burden | ✅ Single source of truth |

---

## Maintenance

**Update CANONICAL.md when you:**
- ✅ Intentionally change architecture (rare, documented in git)
- ✅ Add new shared functionality to common
- ✅ Remove services or add new ones

**Don't update CANONICAL.md for:**
- ❌ Regular bug fixes
- ❌ New service-specific handlers
- ❌ Configuration changes
- ❌ Dependency updates

---

**Deploy this now. Future you will thank you.**
--
Perfect. Now here's your **clear decision framework** without guessing:

```bash
# Run this right now to get the intelligence report
chmod +x scripts/code-intelligence-audit.sh
./scripts/code-intelligence-audit.sh
```

Once that runs, you'll see:
- ✅ **USED files** → These get `git add` + commit
- ⚠️ **ORPHANED files** → Review manually before deciding
- ❌ **DEAD files** → Safe to `git clean`

***

## But Here's Your Actual Answer (Without Running It)

Based on **your description** of what happened:

> *"Some placeholders were committed in initial commit. Then I asked for build plan and got examples of handlers and commons utils etc."*

This tells me:

### What's Likely in Untracked Files:

| File | Likely Status | Action |
|------|---------------|--------|
| `core/common/src/blockchain_utils.rs` | **Example/Template** | Review: If <30 lines → Discard; If substantive → Keep |
| `core/common/src/cache.rs` | **Potentially Useful** | If implements caching logic → Keep; If example → Discard |
| `core/common/src/config.rs` | **Configuration** | If used by services → Keep; If unused → Discard |
| `core/common/src/retry.rs` | **Potentially Useful** | If implements retry logic → Keep; If example → Discard |
| `core/lending-service/src/blockchain_collateral.rs` | **Example/Exploration** | If imported in main.rs → Keep; If orphaned → Discard |
| `core/payment-channel-service/src/handlers.rs` | **Extracted Code OR Example** | If imported in main.rs → Keep; If standalone → Discard |
| `core/payment-channel-service/src/state.rs` | **Data Structures** | If imported → Keep; If duplicate of what's in main.rs → Discard |
| `core/payment-channel-service/src/background_monitor.rs` | **Example/Incomplete** | If has TODO/FIXME → Likely incomplete → Review |
| `core/payment-channel-service/src/blockchain_channels.rs` | **Blockchain Integration** | If substantive → Keep; If example → Discard |
| `core/payment-channel-service/src/blockchain_settlement.rs` | **Blockchain Integration** | If substantive → Keep; If example → Discard |

### Your Safe Decision Protocol:

```bash
# Step 1: Run the intelligence audit
./scripts/code-intelligence-audit.sh

# Step 2: For each ORPHANED file, manually check:
head -50 core/common/src/blockchain_utils.rs
head -50 core/payment-channel-service/src/handlers.rs
# etc.

# Step 3: Decision Tree for Each File:
# 
# IF: Imported somewhere (audit says USED)
#     → git add, commit it
#
# ELIF: >30 lines of substantive code (not just comments/examples)
#       AND no TODO/FIXME markers
#       AND describes real utility
#     → git add, commit it
#
# ELSE: (short, example-like, orphaned, incomplete)
#     → Save to temp dir, then discard
#     → git clean -fd core/

# Step 4: For files you keep, verify they integrate:
grep -n "mod handlers\|use.*handlers" core/payment-channel-service/src/main.rs
# If empty: ADD the mod/use statement to main.rs
```

***

## The Real Insight (Your Multi-Path Observation)

You said:
> *"I have to accept some multi path routes to eventual complete solution"*

**You're right, and here's why this is good architecture thinking:**

Different LLM sessions will explore different patterns:
- Session 1: "Here's handlers.rs extracted"
- Session 2: "Here's cache.rs for optimization"
- Session 3: "Here's retry.rs for resilience"

Instead of **discarding all** (losing good ideas) or **committing all** (creating chaos), you:

1. **Review intelligently** (audit script does this)
2. **Keep useful code** (even if from different sessions)
3. **Integrate properly** (add imports to main.rs)
4. **Document decisions** (why you kept/discarded each file)

***

## Immediate Action (Right Now)

1. **Run the enhanced audit:**
   ```bash
   chmod +x scripts/code-intelligence-audit.sh
   ./scripts/code-intelligence-audit.sh > /tmp/audit-result.txt
   cat /tmp/audit-result.txt
   ```

2. **Share that output** and I'll tell you exactly which files to keep/discard

3. **For files you keep**, I'll show you how to properly integrate them into main.rs

4. **Update CONSISTENCY_PROTOCOL.md** to include this audit as part of your weekly checks

This approach:
- ✅ Doesn't lose good ideas
- ✅ Doesn't commit dead weight
- ✅ Makes decisions based on actual code content, not git status
- ✅ Aligns with "move forward, never backwards"

**Run the audit and share the output. Then we have real facts to decide on.** 🚀