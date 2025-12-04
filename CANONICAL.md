# CANONICAL ARCHITECTURE - Phase 6

Last locked: 2025-12-03  
Status: IMMUTABLE (follow exactly)

## Principle
- services/common/ = All shared: validation, logging, metrics, health, auth, errors, rate limiting
- services/[X]/ = Service-specific business logic only
- No duplication across 6 services
- Single source of truth for everything

## Reference
- Read: PHASE6_ARCHITECTURE.md (why this design)
- Follow: CONSISTENCY_PROTOCOL.md (session protocol)
- Run: scripts/architecture-audit.sh (weekly)

## Next Deviation = Red Flag
If you're tempted to add validation/logging/errors/health to a service main.rs, STOP.
Ask: "Is this in common already?"
- Yes? Import it.
- No? Add to common first, export, then import.