# 🏦 BSV Bank - Algorithmic Banking on Bitcoin SV

[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![React](https://img.shields.io/badge/React-18+-blue.svg)](https://reactjs.org/)

**A fully operational, open-source algorithmic banking platform built entirely on Bitcoin SV blockchain.**

Features deposits, algorithmic interest, P2P lending, and micropayments with complete on-chain transparency and cryptographic verification.

## 🎯 Features

- 💰 **Time-Locked Deposits** with SPV verification
- 📈 **Algorithmic Interest** (2-20% APY based on utilization)
- 🤝 **P2P Lending** with script-enforced contracts (coming soon)
- ⚡ **Micropayments** via payment channels (coming soon)
- 🔒 **Security-First** design with on-chain proofs
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

# Start backend services
./start-all.sh

# Start frontend (new terminal)
cd frontend && npm start
```

Visit http://localhost:3000 🎉

### Quick Test
```bash
# Create a deposit
curl -X POST http://localhost:8080/deposits \
  -H "Content-Type: application/json" \
  -d '{
    "user_paymail": "test@handcash.io",
    "amount_satoshis": 100000,
    "txid": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    "lock_duration_days": 30
  }'

# Check balance
curl http://localhost:8080/balance/test@handcash.io

# Get interest rates
curl http://localhost:8081/rates/current
```

## 📚 Documentation

- [Quick Start Guide](QUICKSTART.md)
- [Project Status](STATUS.md)
- [Architecture Overview](docs/architecture.html)
- [API Documentation](docs/API.md)

## 🏗️ Architecture

Built on proven BSV ecosystem projects:
- **Galaxy** - Ultra high-performance BSV node
- **RustBus** - Microservices engine
- **nPrint** - Bitcoin Script VM
- **SPV Wallet** - Lightweight wallet infrastructure
- **HandCash** - Paymail integration

## 📊 Current Status

✅ **Working**: Deposit service, Interest engine, Frontend UI  
🚧 **In Progress**: Real wallet integration, P2P lending  
📋 **Planned**: Payment channels, Stablecoins, Mobile app

See [STATUS.md](STATUS.md) for detailed progress.

## 🤝 Contributing

Contributions welcome! Please read our [Contributing Guide](CONTRIBUTING.md) first.

## ⚠️ Disclaimer

This software is for educational purposes. Operating a custodial crypto platform requires proper licensing. See [LICENSE](LICENSE) for details.

## 📞 Support

- **Issues**: https://github.com/matcapl/bsv-bank/issues
- **Discussions**: https://github.com/matcapl/bsv-bank/discussions

## 📄 License

MIT License - see [LICENSE](LICENSE) file

---

**Built with ❤️ on Bitcoin SV**

*Banking the way Satoshi intended - peer-to-peer, transparent, and unstoppable.*
