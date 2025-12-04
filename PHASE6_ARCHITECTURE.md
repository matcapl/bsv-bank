# Phase 6 Architecture: (Aspirational) Single Source of Truth

# However this document seems to fail to take account of middleware so to be treated with caution and not slavishly followed until I have full understanding of middleware auth - let's treat it as aspiration. Let's get the files completed sufficient to pass the tests and then let's refactor to common framework when we have a moment to breathe.

**Last Updated:** December 3, 2025  
**Status:** CANONICAL - Follow this, not conflicting advice  
**Principle:** Move steadily forward, never backwards

---

## Core Decision: The Architecture You've Already Built

You've made the right choice. **Honor it. Don't second-guess it.**

```
✅ YOUR ARCHITECTURE (correct, proven, working):

core/common/
  ├── lib.rs (exports all modules)
  ├── auth.rs (JWT)
  ├── validation.rs (paymail, txid, amounts)
  ├── rate_limit.rs (sliding window)
  ├── health.rs (liveness, readiness)
  ├── logging.rs (structured logging)
  ├── metrics.rs (prometheus)
  ├── cache.rs (for efficiency)
  ├── retry.rs (after failed attempt)
  ├── config.rs (settings)
  └── error.rs (standardized errors)

core/deposit-service/
  └── main.rs (ONLY business logic + route setup)
       ├── imports bsv_bank_common::*
       ├── calls validate_paymail(), validate_amount(), etc.
       ├── calls init_logging(), ServiceMetrics, JwtManager
       └── NO duplicate validation code
       └── NO local error handling code

core/lending-service/
core/payment-channel-service/
core/blockchain-monitor/
core/transaction-builder/
core/spv-service/
  └── main.rs (SAME PATTERN as deposit-service)
```

**This is minimal, DRY, and correct. Stick with it.**

---

## Why You're Seeing Conflicting Advice

Two different contexts are recommending different things:

### Context A: "Large individual service files"
- **When:** During early prototyping or when you have 2-3 services
- **Why:** Local iteration is fast
- **Cost:** Duplication across services
- **Problem for you:** You have 7 services + 77 common tests + auth middleware

### Context B: "Minimal service files" (YOUR CHOICE)
- **When:** You're at scale (5+ services) or building incrementally
- **Why:** Single source of truth, no duplication, consistent behavior
- **Cost:** Requires common library discipline
- **Benefit for you:** ✅ This is what you need

**You picked the harder, better choice. The conflicting advice is from someone assuming you'd go with Context A.**

---

## The Rule: How to Stay Consistent Across LLM Sessions

### Rule 1: Never Duplicate What's in `core/common/`

**WRONG:**
```rust
// core/lending-service/main.rs
fn validate_paymail(paymail: &str) -> Result<()> {
    // ... duplicated validation logic ...
}
```

**RIGHT:**
```rust
// core/lending-service/main.rs
use bsv_bank_common::validate_paymail;

// Then just call it:
validate_paymail(&paymail)?;
```

**Why?** If you duplicate, you'll have to update 6 files next time you find a bug. This moves you backwards.

---

### Rule 2: Service main.rs = Business Logic Only

Each service's `main.rs` should:
- ✅ Import from `bsv_bank_common`
- ✅ Define service-specific routes
- ✅ Define service-specific handlers
- ✅ Call validation functions (from common)
- ✅ Call logging/metrics (from common)

Each service's `main.rs` should NOT:
- ❌ Redefine validation logic
- ❌ Redefine error types
- ❌ Redefine logging setup
- ❌ Redefine rate limiting
- ❌ Redefine health check handlers

---

### Rule 3: When You're Unsure, Ask: "Is this unique to THIS service?"

**Unique to lending-service?** → Put it in lending-service/main.rs
```rust
// ✅ Unique: LTV ratio validation for loans
fn validate_ltv_ratio(ltv: f64) -> Result<()> {
    if ltv < 0.0 || ltv > 1.0 {
        return Err("LTV must be 0.0-1.0".into());
    }
    Ok(())
}
```

**NOT unique? Used by multiple services?** → Put it in common
```rust
// ❌ WRONG if this logic is in lending-service/main.rs
fn validate_paymail(paymail: &str) -> Result<()> {
    // All 6 services need this. Put it in common instead.
}
```

---

### Rule 4: The "Minimal Service Template" (Copy-Paste Consistently)

Every service `main.rs` follows this exact pattern. If you're tempted to deviate, stop and ask why.

```rust
// core/SERVICE-NAME/src/main.rs

use actix_web::{web, App, HttpServer, middleware};
use bsv_bank_common::{
    init_logging, JwtManager, ServiceMetrics,
    validate_paymail, validate_txid, validate_amount,
    health_check, liveness_check, readiness_check, metrics_handler,
};
use dotenv::dotenv;
use prometheus::Registry;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    init_logging("SERVICE-NAME");

    let jwt_manager = JwtManager::new(
        &std::env::var("JWT_SECRET").expect("JWT_SECRET not set")
    );
    
    let registry = Registry::new();
    let metrics = ServiceMetrics::new(&registry, "SERVICE-NAME")
        .expect("Failed to create metrics");

    println!("Starting SERVICE-NAME on http://localhost:PORT");

    HttpServer::new(move || {
        App::new()
            .wrap(middleware::Logger::default())
            .app_data(web::Data::new(jwt_manager.clone()))
            .app_data(web::Data::new(metrics.clone()))
            
            // Health & Metrics
            .route("/health", web::get().to(health_check))
            .route("/liveness", web::get().to(liveness_check))
            .route("/readiness", web::get().to(readiness_check))
            .route("/metrics", web::get().to(metrics_handler))
            
            // Service-specific routes
            .route("/loans/create", web::post().to(create_loan))
            .route("/loans/{id}", web::get().to(get_loan))
            // ... more routes ...
    })
    .bind("0.0.0.0:PORT")?
    .run()
    .await
}

// ===== SERVICE-SPECIFIC HANDLERS ONLY =====

async fn create_loan(req: web::Json<CreateLoanRequest>) -> Result<HttpResponse> {
    // ✅ Call common validation
    validate_paymail(&req.borrower_paymail)?;
    validate_amount(req.amount_satoshis)?;
    
    // ✅ Service-specific validation
    validate_ltv_ratio(req.collateral_satoshis as f64 / req.amount_satoshis as f64)?;
    
    // ✅ Business logic
    let loan = create_loan_impl(&req)?;
    
    Ok(HttpResponse::Ok().json(loan))
}

// ===== SERVICE-SPECIFIC HELPERS =====

fn validate_ltv_ratio(ltv: f64) -> Result<()> {
    if ltv < 1.5 {
        return Err("LTV must be >= 1.5".into());
    }
    Ok(())
}

async fn create_loan_impl(req: &CreateLoanRequest) -> Result<Loan> {
    // Your business logic here
    todo!()
}
```

**That's it. That's the whole service main.rs.**

---

## The Rollout Path (Using This Architecture)

### Week 2: Apply Template to Other 5 Services

Each service follows the same pattern. 30-40 minutes per service because:
- ✅ Cargo.toml is identical (just copy from lending-service template)
- ✅ main.rs structure is identical (just copy the template above)
- ✅ Only change: service name + service-specific handlers
- ❌ NO duplicating validation/logging/metrics code

**Total time:** 2.5-3.5 hours (not 3-4 because no "deciding what goes where")

---

## Preventing Backwards Movement Across Sessions

### Session 1 (Today):
You create `core/common/validation.rs` with paymail validation.

### Session 2 (Tomorrow):
You start working on lending-service. An LLM suggests "add validation directly in lending-service/main.rs for faster iteration."

**HOW TO STOP THIS:**

Create this file **right now**:

```
# ARCHITECTURE_DECISIONS.log

## Decision: Validation in Common Library (FINAL)
- Date: 2025-12-03
- Status: COMMITTED - Don't override
- Reason: 77 tests in common cover all validation. 6 services share this.
- Cost of duplication: 6x maintenance burden
- Rollback cost: HIGH (would need to refactor all 5 remaining services)

All validation functions MUST import from `bsv_bank_common::validation`.

No exceptions. Reference this log if tempted to duplicate.
```

Then when an LLM suggests duplicating, paste this back and say:
> "I've committed to validation in common library (see ARCHITECTURE_DECISIONS.log). I can't move backwards. Suggest alternatives that stay consistent."

---

## Anti-Patterns to Block

### ❌ "Let's keep health checks locally in each service"
**Why this is wrong:** 
- `core/common/health.rs` already has 100% test coverage
- If you reimplement it 6 times, you'll have bugs in 6 places
- Moving backwards from tested → untested

**What to do instead:** 
Import and use `health_check()` from common

### ❌ "Let's add rate limiting to just lending-service first"
**Why this is wrong:**
- All services need rate limiting
- You already built it in common
- Implementing it locally means duplicating + debugging + maintaining 6 copies

**What to do instead:**
Apply rate limiting middleware consistently to all services at once

### ❌ "Logging can be local to each service"
**Why this is wrong:**
- You've already built structured logging in common with 77 tests
- Local implementations will be inconsistent
- You'll lose correlation IDs across services

**What to do instead:**
`init_logging("service-name")` in every main.rs—same pattern everywhere

---

## How to Use This with Future LLM Sessions

**Prompt to include in every session:**

```
My Phase 6 architecture is defined in ARCHITECTURE_DECISIONS.log and PHASE6_ARCHITECTURE.md.

Key constraints:
1. All validation, logging, metrics, health checks live in core/common/
2. Each service imports from common, doesn't duplicate
3. Each service main.rs is ~80 lines: template structure + service-specific handlers only
4. No moving backwards (no re-implementing what's in common)

If you suggest adding code to a service that already exists in common, 
redirect me to import it instead.
```

---

## Measuring Success (No Backwards Movement)

Check this weekly:

```bash
# Count lines of validation code in each service
grep -c "validate_paymail\|validate_txid\|validate_amount" core/*/src/main.rs

# Should be: 0 (all imported from common)
# If > 0: You've duplicated. Refactor immediately.

# Count imports from common
grep -c "use bsv_bank_common::" core/*/src/main.rs

# Should be: 6 (one per service except common itself)
# If < 6: You've duplicated. Refactor immediately.

# Check common library exports are used
grep -c "pub fn validate_\|pub fn health_\|pub fn init_logging" core/common/src/lib.rs

# Should be >= 10 (all validation, health, logging functions)
# If lower: Check you didn't miss exporting something
```

---

## Summary: The Single Decision You Need to Make

**You've already made it correctly. Now commit to it.**

- ✅ Common library for: validation, logging, metrics, health, auth, errors, rate limiting
- ✅ Service main.rs for: business logic + route setup only
- ✅ All services follow the same ~80-line template
- ✅ No duplication, no backwards movement, consistent across 6 services

**When tempted to deviate:** Ask "Is this unique to this one service?" 

If "no" → It belongs in common. Import it instead.

If "yes" → Add it to service main.rs, then consider if it should move to common.

---

## Next Steps

1. **Commit this document** as `ARCHITECTURE_DECISIONS.log` at repo root
2. **Use the template** for lending-service first (verify pattern works)
3. **Apply template** to remaining 5 services (30 min each)
4. **Weekly audit** using the grep commands above
5. **Future LLM sessions:** Include architecture prompt (see above)

**No more conflicting advice. One decision. Move forward. ✅**
