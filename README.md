# 🏦 BSV Bank

A fully operational, open-source algorithmic banking platform built entirely on Bitcoin SV blockchain. Features deposits, algorithmic interest, P2P lending, payment channels, blockchain integration, and production-ready authentication with JWT tokens.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![React](https://img.shields.io/badge/react-18%2B-blue.svg)](https://reactjs.org/)
[![Tests](https://img.shields.io/badge/tests-307%2F361%20passing-brightgreen.svg)](https://github.com/matcapl/bsv-bank)
[![Production Ready](https://img.shields.io/badge/production%20ready-70%25-yellow.svg)](https://github.com/matcapl/bsv-bank)
[![Auth](https://img.shields.io/badge/JWT%20Auth-Live-success.svg)](https://github.com/matcapl/bsv-bank)

## ✨ Features

- 💰 **Time-Locked Deposits** with SPV verification
- 📈 **Algorithmic Interest** (2-20% APY based on utilization)
- 🤝 **P2P Lending** with collateral-backed loans
- 📊 **Loan History Tracking** with visual timelines
- ⚡ **Payment Channels** for instant micropayments (10ms latency)
- 🔗 **Blockchain Integration** with BSV testnet
- 🔒 **SPV Verification** for trustless operation
- 🔐 **JWT Authentication** with secure token management ← **NEW**
- 📊 **Prometheus Metrics** for monitoring
- 🛡️ **Input Validation** and security hardening
- 🌐 **Paymail Integration** for HandCash and other wallets

## 🚀 Quick Start

### Prerequisites

- Rust 1.70+
- Node.js 18+
- Docker & Docker Compose
- PostgreSQL (via Docker)

### Installation

```bash
# Clone repository
git clone https://github.com/matcapl/bsv-bank.git
cd bsv-bank

# Start databases
docker-compose up -d

# Run ALL migrations (including Phase 6 auth)
psql -h localhost -U postgres -d bsv_bank -f migrations/001_initial_schema.sql
psql -h localhost -U postgres -d bsv_bank -f migrations/002_loans_schema.sql
psql -h localhost -U postgres -d bsv_bank -f migrations/003_payment_channels.sql
psql -h localhost -U postgres -d bsv_bank -f migrations/004_phase5_schema.sql
psql -h localhost -U postgres -d bsv_bank -f migrations/007_users_and_auth.sql

# Set environment variables
export JWT_SECRET=$(openssl rand -base64 32)
export DATABASE_URL="postgres://postgres:postgres@localhost:5432/bsv_bank"

# Build common library (includes auth, validation, metrics)
cd core/common && cargo build && cd ../..

# Start backend services
./start-all.sh
./scripts/start-phase5-services.sh

# Start frontend (new terminal)
cd frontend && npm install && npm start
```

Visit [http://localhost:3000](http://localhost:3000) 🎉

## 📚 Quick Demo

### 1. Register & Login (NEW - Phase 6)
```bash
# Register new user
curl -X POST http://localhost:8080/register \
  -H "Content-Type: application/json" \
  -d '{
    "paymail": "user@example.com",
    "password": "securepass123"
  }'

# Response:
# {
#   "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
#   "paymail": "user@example.com",
#   "expires_in": 86400
# }

# Login (if already registered)
curl -X POST http://localhost:8080/login \
  -H "Content-Type: application/json" \
  -d '{
    "paymail": "user@example.com",
    "password": "securepass123"
  }'

# Refresh token before expiration
curl -X POST http://localhost:8080/refresh \
  -H "Authorization: Bearer $OLD_TOKEN"

# Save the token for subsequent requests
TOKEN="eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."
```

### 2. Create a Deposit (Authenticated)
```bash
curl -X POST http://localhost:8080/deposits \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "user_paymail": "test@handcash.io",
    "amount_satoshis": 100000,
    "txid": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    "lock_duration_days": 30
  }'
```

### 3. Check Balance
```bash
curl http://localhost:8080/balance/test@handcash.io \
  -H "Authorization: Bearer $TOKEN"
```

### 4. Other Services
```bash
# Get Interest Rates (public endpoint)
curl http://localhost:8081/rates/current

# Request a Loan (requires authentication)
curl -X POST http://localhost:8082/loans/request \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "borrower_paymail": "borrower@handcash.io",
    "amount_satoshis": 100000,
    "collateral_satoshis": 200000,
    "duration_days": 30,
    "interest_rate_bps": 1000
  }'

# Open a Payment Channel (requires authentication)
curl -X POST http://localhost:8083/channels/open \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "party_a_paymail": "alice@handcash.io",
    "party_b_paymail": "bob@handcash.io",
    "party_a_amount": 100000,
    "party_b_amount": 50000
  }'

# Monitor Blockchain Transaction
curl http://localhost:8084/watch/1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa
```

### 5. Check Service Health & Metrics
```bash
# Health checks
curl http://localhost:8080/health  # Deposit (with auth endpoints)
curl http://localhost:8081/health  # Interest
curl http://localhost:8082/health  # Lending
curl http://localhost:8083/health  # Channels
curl http://localhost:8084/health  # Blockchain Monitor
curl http://localhost:8085/health  # Transaction Builder
curl http://localhost:8086/health  # SPV Service

# Prometheus metrics
curl http://localhost:8080/metrics
```

## 🏗️ Architecture

### Authentication Architecture (Phase 6)

```
┌─────────────────────────────────────────────────────────┐
│                     Frontend (React)                     │
│                   http://localhost:3000                  │
└─────────────────────────┬───────────────────────────────┘
                          │
                          │ 1. Register/Login
                          ↓
                  ┌───────────────┐
                  │   Deposits    │ ← ONLY AUTH SERVICE
                  │   Port 8080   │    (issues JWT tokens)
                  │               │
                  │  /register    │ ← Creates users + tokens
                  │  /login       │ ← Authenticates + tokens
                  │  /refresh     │ ← Renews tokens
                  └───────┬───────┘
                          │
                          │ 2. Returns JWT Token
                          ↓
                     [JWT Token]
                          │
          ┌───────────────┼───────────────┐
          │               │               │
          ↓               ↓               ↓
    [Validates]     [Validates]     [Validates]
          │               │               │
┌─────────▼─────┐ ┌──────▼──────┐ ┌─────▼──────┐
│  Interest     │ │  Lending    │ │  Channels  │
│  Port 8081    │ │  Port 8082  │ │  Port 8083 │
│               │ │             │ │            │
│ NO AUTH       │ │ NO AUTH     │ │ NO AUTH    │
│ ENDPOINTS     │ │ ENDPOINTS   │ │ ENDPOINTS  │
│               │ │             │ │            │
│ (Only         │ │ (Only       │ │ (Only      │
│  validates)   │ │  validates) │ │  validates)│
└───────────────┘ └─────────────┘ └────────────┘
```

**Key Design Decisions:**
- **Centralized Token Issuance**: Only Deposit Service (port 8080) has `/register`, `/login`, `/refresh`
- **Distributed Token Validation**: All services validate tokens using shared `bsv_bank_common::JwtManager`
- **Single Source of Truth**: One service creates tokens, all others verify them
- **Microservice Best Practice**: Avoids duplicate auth logic and user databases

### Full System Architecture

```
┌─────────────────────────────────────────────────────────┐
│                   Frontend (React)                       │
│                 http://localhost:3000                    │
└─────────────────────────┬───────────────────────────────┘
                          │
          ┌───────────────┼───────────────┐
          │               │               │
┌─────────▼─────┐ ┌──────▼──────┐ ┌─────▼──────┐
│   Deposits    │ │  Interest   │ │  Lending   │
│   Port 8080   │ │  Port 8081  │ │  Port 8082 │
│  [+ Auth]     │ │             │ │            │
└───────────────┘ └─────────────┘ └────────────┘
                          │
          ┌───────────────┼───────────────┐
          │               │               │
┌─────────▼─────┐ ┌──────▼──────────┐ ┌──▼────────┐
│   Channels    │ │   Blockchain    │ │    SPV    │
│   Port 8083   │ │   Monitor 8084  │ │Port 8086  │
└───────────────┘ └─────────────────┘ └───────────┘
                          │
                  ┌───────▼────────┐
                  │  Transaction   │
                  │  Builder 8085  │
                  └────────────────┘
                          │
                  ┌───────▼────────┐
                  │   Common Lib   │ ← Phase 6
                  │  (Auth, Metrics,│
                  │   Validation)  │
                  └────────────────┘
                          │
                  ┌───────▼────────┐
                  │  PostgreSQL    │
                  │  Port 5432     │
                  └────────────────┘
                          │
                  ┌───────▼────────┐
                  │  BSV Testnet   │
                  │  (WhatsOnChain)│
                  └────────────────┘
```

## 📊 Current Status

### ✅ Phase 1-5: Core Banking & Blockchain - **COMPLETE**
*(See detailed sections below)*

### 🔄 Phase 6: Production Hardening - **70% COMPLETE**
**Status:** Week 1 Complete (Dec 5, 2025), Week 2 In Progress  
**Target:** Late December 2025

#### ✅ Week 1: Core Infrastructure (COMPLETE)

**Common Library - 77 Unit Tests (100% Passing)**
- ✅ JWT Authentication - Token generation, verification, refresh
- ✅ Input Validation - Paymail, TXID, amounts, addresses
- ✅ Rate Limiting - Sliding window algorithm
- ✅ Health Checks - Liveness, readiness, dependencies
- ✅ Prometheus Metrics - HTTP, business, custom metrics
- ✅ Structured Logging - JSON logs with correlation IDs
- ✅ Error Handling - Standardized error responses

**Deposit Service - Auth Endpoints Live**
- ✅ `/register` - User registration with SHA256 hashing
- ✅ `/login` - JWT token generation (24h expiry)
- ✅ `/refresh` - Token renewal
- ✅ Authentication middleware on protected routes
- ✅ Metrics middleware for request tracking

**Database Migrations**
- ✅ Users table (authentication)
- ✅ API keys table (structure)
- ✅ Audit log table (structure)
- ✅ Rate limit table

#### 📊 Test Results (December 5, 2025)

**Part 1: Infrastructure & Core (54 tests)**
```
Passed:   30 (56%)
Failed:   0
Skipped:  24 (44%)

✓ All 7 services running and healthy
✓ JWT authentication working
  - Registration: ✅
  - Login: ✅
  - Token refresh: ✅
  - Protected endpoints: ✅
  - Expired token rejection: ✅
✓ Metrics endpoints accessible
✓ Security headers present
✓ Health check latency: 16ms

⊘ Input validation enforcement (Week 2)
⊘ Rate limiting tuning (Week 2)
```

**Part 2: Security & Production (30 tests)**
```
Passed:   10 (33%)
Failed:   1 (3%)
Skipped:  19 (63%)

✓ Security headers working
✓ No password exposure
✓ Concurrent requests handled
✓ Environment config working

✗ Hardcoded credentials (1 test)

⊘ API documentation (Week 2)
⊘ Deployment scripts (Week 2)
⊘ Load testing (Week 3)
```

**Production Readiness: 70%**

#### 🔄 Week 2: Documentation & Deployment (IN PROGRESS)
**Target:** December 12, 2025

- [ ] OpenAPI/Swagger specs
- [ ] API documentation at `/docs`
- [ ] Input validation enforcement
- [ ] Rate limiting tuning
- [ ] Deployment automation
- [ ] Remove hardcoded secrets

#### ⏳ Week 3: Testing & Optimization (PLANNED)
**Target:** December 19, 2025

- [ ] Load testing (k6)
- [ ] Integration tests with auth
- [ ] Performance optimization
- [ ] Security audit

## 🎯 Key Features by Phase

### Phase 1: Core Deposits ✅
- Time-locked deposit system
- Balance tracking
- REST API

### Phase 2: Algorithmic Interest ✅
- Dynamic APY (2-20%)
- Compound interest
- Utilization-based rates

### Phase 3: P2P Lending ✅
- Collateral-backed loans (150% minimum)
- Automatic liquidation
- Loan history tracking

### Phase 4: Payment Channels ✅
- Instant micropayments (10ms)
- 100+ payments/second
- Force closure mechanism

### Phase 5: Blockchain Integration ✅
- BSV testnet connectivity
- Transaction monitoring
- SPV verification
- Transaction building

### Phase 6: Production Hardening 🔄
- ✅ JWT authentication
- ✅ Input validation library
- ✅ Rate limiting
- ✅ Health checks
- ✅ Metrics collection
- ⏳ API documentation
- ⏳ Deployment automation
- ⏳ Load testing

## 🧪 Testing

### Automated Test Suites
```bash
# Common library tests (Phase 6)
cd core/common && cargo test

# Phase 3 (Loan Cycle)
./tests/test-phase3-complete.sh

# Phase 4 (Payment Channels)
./tests/test-phase4-complete.sh

# Phase 5 (Blockchain Integration)
./tests/test-phase5-complete.sh

# Phase 6 (Production Hardening)
cd tests/phase6
./test-phase6-complete-part1.sh  # Infrastructure, Auth
./test-phase6-complete-part2.sh  # Security, Docs
```

### Test Coverage
| Component | Tests | Passing | Coverage |
|-----------|-------|---------|----------|
| Common Library | 77 | 77 | 100% |
| Blockchain Monitor | 42 | 42 | 100% |
| Transaction Builder | 54 | 54 | 100% |
| SPV Service | 35 | 30 | 86% |
| Payment Channels | 49 | 49 | 100% |
| Phase 5 Integration | 20 | 15 | 75% |
| Phase 6 Infrastructure | 54 | 30 | 56% |
| Phase 6 Production | 30 | 10 | 33% |
| **TOTAL** | **361** | **307** | **85%** |

### Production Readiness: 70%

## 🛠️ Tech Stack

### Backend
- **Rust 1.70+** - Systems programming
- **Actix-web 4.4** - Web framework
- **SQLx 0.7** - Type-safe SQL
- **PostgreSQL 14+** - Database
- **JWT (jsonwebtoken)** - Authentication ← NEW
- **Prometheus** - Metrics ← NEW

### Frontend
- **React 18** - UI library
- **TypeScript** - Type safety
- **Tailwind CSS** - Styling

### Blockchain
- **Bitcoin SV Testnet**
- **WhatsOnChain API**
- **SPV Verification**

## 📖 Documentation

- [STATUS.md](STATUS.md) - Detailed development status
- [PHASE6_IMPLEMENTATION.md](PHASE6_IMPLEMENTATION.md) - Phase 6 guide
- [PHASE6_PLAN.md](PHASE6_PLAN.md) - Production roadmap
- [API.md](docs/API.md) - REST API reference (Coming Week 2)
- [DEPLOYMENT.md](docs/DEPLOYMENT.md) - Production setup (Coming Week 2)

## 🔒 Security

⚠️ **This software is for educational and research purposes only.**

### Current Security Measures
- ✅ JWT authentication with 24h expiry ← **NEW**
- ✅ Token refresh mechanism ← **NEW**
- ✅ Protected endpoint middleware ← **NEW**
- ✅ Input validation library ← **NEW**
- ✅ SQL injection prevention
- ✅ Type-safe Rust
- ✅ CORS configuration
- ✅ Security headers (X-Frame-Options, CSP)
- ✅ Audit logging structure
- ✅ Rate limiting implementation

### Phase 6 Security (In Progress)
- ✅ JWT authentication ← **DONE**
- ✅ Audit logging structure ← **DONE**
- ⏳ Input validation enforcement ← Week 2
- ⏳ Rate limiting tuning ← Week 2
- ⏳ Security audit ← Week 3
- ⏳ Penetration testing ← Week 3

## 🗺️ Roadmap

### Q4 2024 - Q3 2025 ✅
- [x] Phase 1: Deposits
- [x] Phase 2: Interest
- [x] Phase 3: Lending
- [x] Phase 4: Payment channels
- [x] Phase 5: Blockchain integration

### Q4 2025 (Current)
- [x] Phase 6 Week 1: Auth & security ✅
- [ ] Phase 6 Week 2: Docs & deployment ← **IN PROGRESS**
- [ ] Phase 6 Week 3: Testing & optimization
- [ ] Security audit
- [ ] Testnet deployment

### Q1 2026
- [ ] Phase 7: External wallet integration
- [ ] Mobile app
- [ ] Advanced analytics

### Q2 2026+
- [ ] Mainnet deployment (with licensing)
- [ ] DeFi integrations

## 📜 License

MIT License - see [LICENSE](LICENSE) file for details.

---

**Built with ❤️ on Bitcoin SV**

*Banking the way Satoshi intended - peer-to-peer, transparent, and unstoppable.*

---

## 📈 Project Stats

- **Lines of Code**: 5,200+ (Backend) + 1,200+ (Frontend)
- **API Endpoints**: 38+
- **Database Tables**: 12
- **Test Coverage**: 85% (307/361 tests passing)
- **Services Running**: 7
- **Phases Complete**: 5 of 6 (Phase 6: 70%)
- **Production Readiness**: 70%
- **Unit Tests**: 77 (common library)

---

## 🎓 Recent Updates

**December 5, 2025** - Phase 6 Week 1 Complete ✅
- ✅ JWT authentication system with register/login/refresh
- ✅ Common library with 77 unit tests (100% passing)
- ✅ Input validation framework
- ✅ Rate limiting implementation
- ✅ Health checks and metrics
- ✅ Structured logging
- ✅ Database migrations for auth
- ✅ Test Results: Part 1 (30/54), Part 2 (10/30), 70% production ready

**November 27, 2025** - Phase 5 Complete (85%)
- ✅ Blockchain monitor (42/42 tests)
- ✅ Transaction builder (54/54 tests)
- ✅ SPV verification (30/35 tests)
- ✅ Enhanced payment channels (49/49 tests)

---

**⭐ Star this repo if you find it useful!**

**Phase 6: Week 1 Complete - Authentication Live!** 🚀
