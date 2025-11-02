# BSV Bank - Algorithmic Banking on Bitcoin SV

<p align="center">
  <img src="https://img.shields.io/badge/Bitcoin%20SV-Native-orange" />
  <img src="https://img.shields.io/badge/Rust-Microservices-red" />
  <img src="https://img.shields.io/badge/React-Frontend-blue" />
  <img src="https://img.shields.io/badge/License-MIT-green" />
</p>

A fully operational, open-source algorithmic banking platform built entirely on Bitcoin SV blockchain. Features deposits, interest accrual, P2P lending, and micropayments with complete on-chain transparency.

## 🎯 Core Features

- **💰 Deposits**: Time-locked deposits with SPV verification
- **📈 Algorithmic Interest**: Utilization-based rates (2-20% APY)
- **🤝 P2P Lending**: Script-enforced loan contracts with collateral
- **⚡ Micropayments**: Sub-cent transactions with instant settlement
- **🔒 Security**: Multi-sig custody, on-chain proofs, cryptographic verification
- **🌐 Paymail Integration**: HandCash, Money Button, and SPV wallet support

## 🏗️ Architecture

### Technology Stack

```
┌─────────────────────────────────────────────────────────┐
│                     Frontend Layer                       │
│  React + Tailwind + HandCash SDK + Paymail API          │
└────────────────┬────────────────────────────────────────┘
                 │
┌────────────────▼────────────────────────────────────────┐
│                   API Gateway (Rust)                     │
│            RustBus Microservices Engine                  │
└─────┬──────────┬──────────┬──────────┬─────────────────┘
      │          │          │          │
┌─────▼─────┐ ┌─▼────────┐ ┌▼────────┐ ┌▼────────────────┐
│ Deposit   │ │ Interest │ │ Lending │ │ Wallet Manager  │
│ Service   │ │ Engine   │ │ Service │ │                 │
└─────┬─────┘ └──┬───────┘ └─┬───────┘ └─┬───────────────┘
      │          │           │            │
┌─────▼──────────▼───────────▼────────────▼───────────────┐
│              Bitcoin SV Blockchain Layer                 │
│    Galaxy Node + nPrint Script VM + SPV Wallet          │
└──────────────────────────────────────────────────────────┘
```

### Leveraged BSV Ecosystem Projects

| Project | Purpose | Repository |
|---------|---------|------------|
| **Galaxy** | Ultra high-performance BSV node | `murphsicles/Galaxy` |
| **RustBus** | Microservices communication engine | `murphsicles/RustBus` |
| **nPrint** | Bitcoin Script VM in Rust | `murphsicles/nPrint` |
| **Mohrt** | Wallet & payment channel libraries | `bitcoin-sv/mohrt` |
| **SPV Wallet** | Lightweight wallet infrastructure | `bitcoin-sv/spv-wallet-web-backend` |
| **HandCash API** | Paymail wallet integration | `HandCash/external-wallet-demo` |
| **Polynym** | Paymail resolver | `uptimesv/polynym` |
| **Gateway** | Payment server | `p2ppsr/gateway` |
| **Vlite** | Vector database for indexing | `sdan/vlite` |

## 🚀 Quick Start

### Prerequisites

- Rust 1.70+
- Node.js 18+
- Docker & Docker Compose
- BSV testnet node or Galaxy node access

### Installation

```bash
# Clone repository
git clone https://github.com/yourorg/bsv-bank.git
cd bsv-bank

# Install Rust dependencies
cd core/deposit-service
cargo build --release

# Install frontend dependencies
cd ../../frontend
npm install

# Set up environment
cp .env.example .env
# Edit .env with your configuration
```

### Configuration

Create `.env` file:

```env
# Galaxy Node
GALAXY_NODE_URL=http://localhost:8332
GALAXY_RPC_USER=bsvuser
GALAXY_RPC_PASSWORD=bsvpass

# Database
DATABASE_URL=postgresql://localhost/bsv_bank
REDIS_URL=redis://localhost:6379

# Security
JWT_SECRET=your-secret-key-here
ENCRYPTION_KEY=your-encryption-key

# Paymail
HANDCASH_APP_ID=your-handcash-app-id
HANDCASH_APP_SECRET=your-app-secret

# Compliance
KYC_PROVIDER=sumsub
KYC_API_KEY=your-sumsub-key
```

### Running Services

#### Option 1: Docker Compose (Recommended)

```bash
docker-compose up -d
```

This starts:
- Deposit service (port 8080)
- Interest engine (port 8081)
- Lending service (port 8082)
- Frontend (port 3000)
- PostgreSQL database
- Redis cache

#### Option 2: Manual Start

```bash
# Terminal 1: Start deposit service
cd core/deposit-service
cargo run --release

# Terminal 2: Start frontend
cd frontend
npm start

# Access application at http://localhost:3000
```

## 📖 Usage Guide

### 1. Connect Wallet

```javascript
// Frontend integration example
import { HandCashConnect } from '@handcash/handcash-connect';

const handCash = new HandCashConnect({
  appId: process.env.REACT_APP_HANDCASH_APP_ID,
});

const account = await handCash.getAccount();
const paymail = account.publicProfile.paymail;
```

### 2. Create Deposit

```javascript
// Make deposit via API
const response = await fetch('http://localhost:8080/deposits', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    user_paymail: 'user@handcash.io',
    amount_satoshis: 1000000, // 0.01 BSV
    txid: transaction.id,
    lock_duration_days: 30
  })
});
```

### 3. Check Balance

```bash
curl http://localhost:8080/balance/user@handcash.io
```

Response:
```json
{
  "balance_satoshis": 1000000,
  "accrued_interest_satoshis": 8219,
  "total_available_satoshis": 1008219,
  "current_apy": 8.5,
  "active_deposits": 1
}
```

### 4. Withdraw Funds

```javascript
const withdrawal = await fetch('http://localhost:8080/withdrawals', {
  method: 'POST',
  body: JSON.stringify({
    deposit_id: 'DEP_abc123',
    destination_address: '1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa',
    signature: signedMessage
  })
});
```

## 🧪 Testing

### Unit Tests

```bash
# Rust services
cd core/deposit-service
cargo test

# Frontend
cd frontend
npm test
```

### Integration Tests

```bash
# Start test environment
docker-compose -f docker-compose.test.yml up -d

# Run integration suite
cargo test --features integration

# Test specific service
cargo test --test deposit_service_integration
```

### Security Audit

```bash
# Run Rust security audit
cargo audit

# Static analysis
cargo clippy -- -D warnings

# Frontend security scan
npm audit

# Smart contract validation
node scripts/validate-scripts.js
```

## 🔐 Security Considerations

### Key Management

- **User Keys**: Never stored on server, kept in client-side wallet only
- **Service Keys**: Encrypted at rest using AES-256-GCM
- **Multi-Sig**: 2-of-3 escrow for deposits > 1 BSV
- **HSM Integration**: Production deployments should use Hardware Security Modules

### Script Security

All Bitcoin Scripts undergo:
1. Static analysis via `ScriptValidator`
2. Opcode whitelist verification
3. Stack depth validation (max 1000)
4. No dangerous opcodes (OP_CAT, etc.)

### API Security

- JWT authentication with 1-hour expiration
- Rate limiting: 100 requests/minute per IP
- CORS restricted to known domains
- Input validation and sanitization
- SQL injection prevention via prepared statements

## 📊 Interest Rate Algorithm

Based on Aave/Compound utilization model:

```
Interest Rate = Base Rate + (Utilization × Slope)

Where:
- Base Rate: 2% APY (minimum lender yield)
- Utilization: Total Borrowed / Total Supplied
- Slope1: 10% (0-80% utilization)
- Slope2: 100% (80-100% utilization)

Examples:
- 20% utilization → 4% APY
- 60% utilization → 8% APY
- 90% utilization → 18% APY
```

### Interest Distribution

- Calculated every block (~10 minutes)
- Committed to blockchain via OP_RETURN
- Distributed daily at 00:00 UTC
- Compounding enabled (reinvest interest)

## ⚖️ Regulatory Compliance

### Required Licenses

| Jurisdiction | License Type | Status |
|--------------|-------------|---------|
| UK (FCA) | CASP Registration | Required for EU/UK users |
| EU | MiCA CASP Authorization | Required from 2025 |
| Curacao (CBCS) | VASP Registration | Recommended for non-EU |
| USA | MSB + State Licenses | Required for US operations |

### Compliance Features

✅ **KYC/AML**: Sumsub integration for identity verification  
✅ **Travel Rule**: FATF-compliant for transfers > $1000  
✅ **Audit Trail**: Immutable on-chain transaction records  
✅ **Reserve Proof**: Monthly cryptographic proof of reserves  
✅ **Suspicious Activity**: Automated SAR filing capability  

### Implementation Checklist

- [ ] Integrate KYC provider (Sumsub, Onfido, etc.)
- [ ] Implement Travel Rule messaging (TRISA, TRP)
- [ ] Set up compliance monitoring dashboard
- [ ] Obtain legal opinion on jurisdictional requirements
- [ ] Apply for necessary VASP registrations
- [ ] Configure transaction limits and reporting thresholds
- [ ] Establish AML/CTF policies and procedures
- [ ] Conduct third-party security audit

## 👥 Team Requirements

### Essential Roles

1. **Bitcoin Protocol Engineer**
   - Expert in Bitcoin Script and UTXO model
   - Experience with payment channels and SPV
   - Galaxy/nPrint integration

2. **Backend Engineer (Rust)**
   - Microservices architecture
   - RustBus framework
   - Database optimization

3. **Frontend Developer (React)**
   - Wallet SDK integration
   - Paymail UX design
   - Real-time updates

4. **Security Engineer**
   - Smart contract auditing
   - Penetration testing
   - Key management systems

5. **Compliance Officer**
   - VASP regulations (FCA, MiCA, FATF)
   - AML/CTF procedures
   - Licensing applications

6. **Product Manager**
   - Fintech experience
   - User research
   - Feature prioritization

## 🗺️ Roadmap

### Phase 1: Core Infrastructure ✅ (Weeks 1-4)
- [x] Galaxy node integration
- [x] SPV wallet setup
- [x] Basic deposit/withdrawal scripts
- [x] Interest rate engine

### Phase 2: MVP Launch 🔄 (Weeks 5-8)
- [ ] Frontend deployment
- [ ] Interest distribution automation
- [ ] KYC integration
- [ ] Security audit
- [ ] Testnet launch

### Phase 3: P2P Lending (Weeks 9-12)
- [ ] Loan contract scripts
- [ ] Collateral management
- [ ] Liquidation engine
- [ ] Lending marketplace UI

### Phase 4: Advanced Features (Weeks 13-16)
- [ ] Stablecoin layer
- [ ] Payment channels
- [ ] Mobile apps
- [ ] Mainnet launch

### Phase 5: Scaling (Q2 2026)
- [ ] Multi-currency support
- [ ] Derivatives (options, futures)
- [ ] Institutional API
- [ ] Global expansion

## 🤝 Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Development Workflow

1. Fork repository
2. Create feature branch (`git checkout -b feature/amazing-feature`)
3. Commit changes (`git commit -m 'Add amazing feature'`)
4. Push to branch (`git push origin feature/amazing-feature`)
5. Open Pull Request

### Code Standards

- **Rust**: Follow Rust style guide, run `cargo fmt` and `cargo clippy`
- **JavaScript**: ESLint + Prettier, Airbnb style guide
- **Testing**: Minimum 80% code coverage
- **Documentation**: All public APIs must have JSDoc/Rustdoc

## 📄 License

This project is licensed under the MIT License - see [LICENSE](LICENSE) file.

## ⚠️ Disclaimer

**This software is provided for educational and research purposes only.**

Operating a custodial cryptocurrency banking platform requires proper licensing and regulatory compliance in your jurisdiction. This codebase does NOT constitute:
- Legal or financial advice
- A guarantee of regulatory compliance
- Production-ready software without proper auditing

**YOU ARE SOLELY RESPONSIBLE FOR:**
- Obtaining necessary licenses (VASP, MSB, etc.)
- Implementing required compliance measures
- Security audits and penetration testing
- Legal consultation with qualified attorneys
- Risk management and insurance

**RISKS INCLUDE:**
- Smart contract vulnerabilities
- Regulatory enforcement actions
- User fund loss
- Criminal prosecution for operating without license

Always consult with legal and compliance professionals before deploying to production.

## 📞 Support

- **Documentation**: https://docs.bsv-bank.io
- **Discord**: https://discord.gg/bsv-bank
- **Email**: support@bsv-bank.io
- **Security Issues**: security@bsv-bank.io (PGP key available)

## 🙏 Acknowledgments

Built on the shoulders of giants:

- **Murphsicles**: Galaxy node, RustBus, nPrint - foundational BSV infrastructure
- **Bitcoin SV Association**: SPV Wallet, technical standards
- **HandCash**: Paymail innovation and wallet APIs
- **Aave & Compound**: Interest rate model inspiration
- **OpenZeppelin**: Security best practices

---

**Built with ❤️ on Bitcoin SV**

*"Banking the way Satoshi intended - peer-to-peer, transparent, and unstoppable."*