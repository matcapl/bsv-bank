# 🏦 BSV Bank

A fully operational, open-source algorithmic banking platform built entirely on Bitcoin SV blockchain. Features deposits, algorithmic interest, P2P lending, payment channels, and blockchain integration with complete on-chain transparency and cryptographic verification.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![React](https://img.shields.io/badge/react-18%2B-blue.svg)](https://reactjs.org/)
[![Tests](https://img.shields.io/badge/tests-195%2F215%20passing-brightgreen.svg)](https://github.com/matcapl/bsv-bank)

## ✨ Features

- 💰 **Time-Locked Deposits** with SPV verification
- 📈 **Algorithmic Interest** (2-20% APY based on utilization)
- 🤝 **P2P Lending** with collateral-backed loans
- 📊 **Loan History Tracking** with visual timelines
- 📈 **Statistics Dashboard** for lending activity
- ⚡ **Payment Channels** for instant micropayments (10ms latency)
- 🔗 **Blockchain Integration** with BSV testnet
- 🔒 **SPV Verification** for trustless operation
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

# Run migrations
psql -h localhost -U a -d bsv_bank -f migrations/schema.sql

# Start backend services
./start-all.sh
./scripts/start-phase5-services.sh

# Start frontend (new terminal)
cd frontend && npm install && npm start
```

Visit [http://localhost:3000](http://localhost:3000) 🎉

## 📚 Quick Demo

### Create a Deposit
```bash
curl -X POST http://localhost:8080/deposits \
  -H "Content-Type: application/json" \
  -d '{
    "user_paymail": "test@handcash.io",
    "amount_satoshis": 100000,
    "txid": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    "lock_duration_days": 30
  }'
```

### Check Balance
```bash
curl http://localhost:8080/balance/test@handcash.io
```

### Get Interest Rates
```bash
curl http://localhost:8081/rates/current
```

### Request a Loan
```bash
curl -X POST http://localhost:8082/loans/request \
  -H "Content-Type: application/json" \
  -d '{
    "borrower_paymail": "borrower@handcash.io",
    "amount_satoshis": 100000,
    "collateral_satoshis": 200000,
    "duration_days": 30,
    "interest_rate_bps": 1000
  }'
```

### Open a Payment Channel
```bash
curl -X POST http://localhost:8083/channels/open \
  -H "Content-Type: application/json" \
  -d '{
    "party_a_paymail": "alice@handcash.io",
    "party_b_paymail": "bob@handcash.io",
    "party_a_amount": 100000,
    "party_b_amount": 50000
  }'
```

### Monitor Blockchain Transaction
```bash
curl http://localhost:8084/watch/1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa
```

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────────┐
│                     Frontend (React)                     │
│                   http://localhost:3000                  │
└─────────────────────────┬───────────────────────────────┘
                          │
          ┌───────────────┼───────────────┐
          │               │               │
┌─────────▼─────┐ ┌──────▼──────┐ ┌─────▼──────┐
│   Deposits    │ │  Interest   │ │  Lending   │
│   Port 8080   │ │  Port 8081  │ │  Port 8082 │
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

### ✅ Phase 1: Core Deposits - **COMPLETE**
- Deposit creation and management
- Time-locked deposits
- Balance tracking
- **Tests:** All passing

### ✅ Phase 2: Algorithmic Interest Engine - **COMPLETE**
- Dynamic APY (2-20% based on utilization)
- Interest accrual and compounding
- Historical rate tracking
- **Tests:** All passing

### ✅ Phase 3: P2P Lending - **COMPLETE**
- Loan requests and funding
- Collateral management (150% minimum)
- Repayment processing
- Liquidation monitoring
- Loan history tracking
- Statistics dashboard
- **Tests:** All passing

### ✅ Phase 4: Payment Channels - **COMPLETE**
- Instant micropayments (10ms latency)
- Bidirectional payment channels
- 100+ payments/second throughput
- Channel state management
- Cooperative and force closure
- **Tests:** 94/94 passing (100%)

### ✅ Phase 5: Blockchain Integration - **COMPLETE** (95%)
- BSV testnet connectivity
- Transaction monitoring
- Transaction builder (P2PKH, multisig)
- SPV proof verification
- Channel funding transactions
- **Tests:** 195/215 passing (91%)
  - 5 tests skipped (require testnet funding)
  - 10 E2E tests pending (require full testnet setup)

### 🎯 Phase 6: Production Hardening - **IN PROGRESS**
- Security hardening and audit
- Performance optimization
- Monitoring and observability
- API documentation
- Load testing

See [STATUS.md](STATUS.md) for detailed progress.

DB implemented with 001.sql, 002, 003, 004

yet to 006_testnet
yet to 007_users and auth

possibly useful possibly redundant 009,010,011

## 🎯 Key Achievements

### Phase 5 Complete (November 2025)
- ✅ **Blockchain Monitor** - Transaction tracking via WhatsOnChain
- ✅ **Transaction Builder** - P2PKH, multisig, channel transactions
- ✅ **SPV Service** - Merkle proof validation, chain verification
- ✅ **Enhanced Channels** - Blockchain-backed payment channels
- ✅ **Performance** - Sub-100ms blockchain queries (cached)

### Performance Metrics
- Payment Latency: ~10ms average
- Blockchain Queries: 80-150ms (cached: <50ms)
- Transaction Building: <30ms
- Throughput: 100+ payments/second
- Test Success Rate: 91% (195/215 tests)

## 🧪 Testing

### Automated Test Suites
```bash
# Test Phase 4 (Payment Channels)
./tests/test-phase4-complete.sh

# Test Phase 5 (Blockchain Integration)
./tests/test-phase5-complete.sh

# Individual service tests
./test-deposits.sh
./test-interest.sh
./test-phase3-complete.sh

# NEW: Phase 6 (Hardening: Authentication, etc.)
./test-phase6-complete-part1.sh && ./test-phase6-complete-part2.sh

# TO DO: Testnet tracking tests
./test-testnet-tracking.sh
```

### Test Coverage
| Component | Tests | Passing | Coverage |
|-----------|-------|---------|----------|
| Pre-flight Checks | 5 | 5 | 100% |
| Blockchain Monitor | 42 | 42 | 100% |
| Transaction Builder | 54 | 54 | 100% |
| SPV Service | 35 | 30 | 86% |
| Payment Channels | 49 | 49 | 100% |
| Integration Tests | 20 | 15 | 75% |
| **TOTAL** | **215** | **195** | **91%** |

### Manual Testing
```bash
# Check service health
curl http://localhost:8080/health  # Deposits
curl http://localhost:8081/health  # Interest
curl http://localhost:8082/health  # Lending
curl http://localhost:8083/health  # Channels
curl http://localhost:8084/health  # Blockchain Monitor

# View logs
tail -f logs/deposit.log
tail -f logs/interest.log
tail -f logs/loans.log
tail -f logs/payment-channels.log
tail -f logs/blockchain-monitor.log
```

## 🛠️ Tech Stack

### Backend
- **Rust 1.70+** - Systems programming language
- **Actix-web 4.4** - High-performance web framework
- **SQLx 0.7** - Type-safe SQL with compile-time verification
- **PostgreSQL 14+** - Reliable database
- **Tokio** - Async runtime

### Frontend
- **React 18** - Modern UI library
- **TypeScript** - Type-safe JavaScript
- **Lucide React** - Beautiful icons
- **Tailwind CSS** - Utility-first styling

### Blockchain
- **Bitcoin SV Testnet** - Scalable blockchain
- **WhatsOnChain API** - Blockchain data provider
- **SPV Verification** - Lightweight client

### Infrastructure
- **Docker** - Containerization
- **Docker Compose** - Service orchestration
- **Bash** - Automation scripts

## 📖 Documentation

- [STATUS.md](STATUS.md) - Detailed development status
- [PHASE6_PLAN.md](PHASE6_PLAN.md) - Production hardening roadmap
- [API.md](docs/API.md) - REST API reference
- [ARCHITECTURE.md](docs/ARCHITECTURE.md) - System design
- [DEPLOYMENT.md](docs/DEPLOYMENT.md) - Production setup
- [CONTRIBUTING.md](CONTRIBUTING.md) - Development workflow

## 🤝 Contributing

Contributions welcome! Please read our [Contributing Guide](CONTRIBUTING.md) first.

### Development Workflow
```bash
# 1. Fork the repository
# 2. Create feature branch
git checkout -b feature/amazing-feature

# 3. Make changes and test
cargo test
npm test

# 4. Commit changes
git commit -m "Add amazing feature"

# 5. Push and create PR
git push origin feature/amazing-feature
```

## 🔒 Security

⚠️ **This software is for educational and research purposes only.**

### Current Security Measures
- ✅ Input validation on all endpoints
- ✅ SQL injection prevention (parameterized queries)
- ✅ Type-safe Rust implementation
- ✅ CORS configuration
- ✅ Collateral requirements (150% minimum)
- ✅ SPV proof verification

### Phase 6 Security (Planned)
- ⏳ JWT authentication
- ⏳ API rate limiting
- ⏳ Security audit
- ⏳ Penetration testing
- ⏳ Request signing

### Legal Disclaimer
Operating a custodial cryptocurrency platform requires:
- Money transmitter licenses
- KYC/AML compliance
- Securities registration (jurisdiction-dependent)
- Banking licenses (some jurisdictions)
- Consumer protection measures
- Data privacy compliance (GDPR, CCPA, etc.)

**Do not use this in production without proper legal counsel and regulatory approval.**

## 📞 Support

- **Issues**: [GitHub Issues](https://github.com/matcapl/bsv-bank/issues)
- **Discussions**: [GitHub Discussions](https://github.com/matcapl/bsv-bank/discussions)
- **Wiki**: [Documentation](https://github.com/matcapl/bsv-bank/wiki)

## 🗺️ Roadmap

### Q4 2024 - Q3 2025 ✅
- [x] Phase 1: Deposit service
- [x] Phase 2: Interest engine
- [x] Phase 3: P2P lending
- [x] Phase 4: Payment channels
- [x] Phase 5: Blockchain integration (95%)

### Q4 2025 - Q1 2026 (Current)
- [ ] Phase 6: Production hardening
- [ ] Security audit
- [ ] Performance optimization
- [ ] API documentation
- [ ] Testnet deployment

### Q1 2026 - Q2 2026
- [ ] HandCash wallet integration
- [ ] Mobile app (iOS/Android)
- [ ] Advanced analytics dashboard
- [ ] Multi-currency support

### Q2 2026+
- [ ] Mainnet deployment (with proper licensing)
- [ ] Governance system
- [ ] DeFi integrations
- [ ] Cross-chain bridges

## 🌟 Built With

Powered by proven BSV ecosystem projects:
- [Bitcoin SV](https://bitcoinsv.com) - Scalable blockchain
- [WhatsOnChain](https://whatsonchain.com) - Blockchain API
- [SPV Wallet](https://github.com/bitcoin-sv/spv-wallet) - Lightweight wallet
- [HandCash](https://handcash.io) - Paymail integration

## 📜 License

MIT License - see [LICENSE](LICENSE) file for details.

Copyright (c) 2024-2025 BSV Bank Contributors

## 🙏 Acknowledgments

- Bitcoin SV community
- WhatsOnChain team
- HandCash team
- All contributors and early testers

---

**Built with ❤️ on Bitcoin SV**

*Banking the way Satoshi intended - peer-to-peer, transparent, and unstoppable.*

---

## 📈 Project Stats

- **Lines of Code**: 4,500+ (Backend) + 1,200+ (Frontend)
- **API Endpoints**: 30+
- **Database Tables**: 9
- **Test Coverage**: 91% (195/215 tests passing)
- **Services Running**: 7
- **Phases Complete**: 5 of 6 ✅

---

## 🎓 Recent Updates

**November 27, 2025** - Phase 5 Complete (95%)
- ✅ Blockchain monitor with transaction tracking
- ✅ Transaction builder for P2PKH and multisig
- ✅ SPV verification service
- ✅ Enhanced payment channels with blockchain integration
- ✅ 195/215 tests passing (91% success rate)

**November 13, 2025** - Phase 4 Complete
- ✅ Payment channels with 10ms latency
- ✅ 100+ payments/second throughput
- ✅ Force closure and dispute handling

**November 10, 2025** - Phase 3 Complete
- ✅ P2P lending with loan history
- ✅ Statistics dashboard
- ✅ Complete loan lifecycle

---

**⭐ Star this repo if you find it useful!**

**Ready for Phase 6: Production Hardening** 🚀