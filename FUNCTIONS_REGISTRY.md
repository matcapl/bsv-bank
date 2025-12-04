# Template for Functions Registry Entry:

- `function_name(param: Type) -> ReturnType` 
  **Location:** `path/src/file.rs` 
  **Tag:** `[COMMON]` and / or all services that use it `[SERVICE-SPECIFIC: lending-service]` 
  **Description:** One sentence. What does it do?
  **Dependencies:** Lists what it calls (to track coupling)
  **Called-by:** Lists at least one function per file per service that it is called by, so coudl be multipe files. 
  **Distinct-from:** Explain where potential ambiguities or similar files are distinct so we can readily identify duplicates and not ever delete distinct functions doing distinct things

Example:
- `wait_for_confirmations(txid: &str, required: i32, timeout: u64) -> Result<i32>`
  **Location:** `core/common/src/blockchain_ops.rs`
  **Tag:** `[COMMON]` blockchain-monitor, payment-channel-service, transaction-builder
  **Description:** Polls blockchain until TX has N confirmations or timeout.
  **Dependencies:** reqwest, chrono (external calls)
  **Used by:** blockchain-monitor/src/main.rs main() or whatever the best practice way is to locate/specify co-ordinates for specific calls in specific functions in specific files in specific core (services) directories
  **Dictinct-from:** different from ... or aligned with ... or helper function alongside ...


1. Build workspace toml first

# core/Cargo.toml
[workspace]
members = ["common", "lending-service", "blockchain-monitor", ...]

[workspace.dependencies]
# Truly universal
actix-web = "4"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
serde = { version = "1", features = ["derive"] }
sqlx = { version = "0.7", features = ["postgres", "runtime-tokio"] }

# Blockchain-specific (used by 3+ services)
reqwest = { version = "0.11", features = ["json"] }

# Infrastructure
tracing = "0.1"
tracing-subscriber = "0.3"

each service then only declares what it uses
# core/blockchain-monitor/Cargo.toml
[dependencies]
actix-web = { workspace = true }
tokio = { workspace = true }
sqlx = { workspace = true }
reqwest = { workspace = true }

2. YES - Create FUNCTIONS_REGISTRY.md

# BSV Bank - Functions Registry

## Common Library (`core/common/src/`)

### Authentication & Security
- `validate_paymail(paymail: &str) -> Result<bool>` - core/common/src/auth.rs - Validates BSV paymail format
- `hash_password(password: &str) -> String` - core/common/src/auth.rs - Argon2 hashing for passwords

### Blockchain Operations
- `wait_for_confirmations(txid: &str, required: i32, timeout: u64) -> Result<i32>` - core/common/src/blockchain_ops.rs - Polls for TX confirmations
- `validate_transaction(tx_hex: &str) -> Result<()>` - core/common/src/blockchain_ops.rs - SPV validation
- `build_multisig_address(pubkeys: Vec<&str>, threshold: u8) -> Result<String>` - core/common/src/blockchain_ops.rs - Creates multisig address

### Configuration
- `load_config_from_env() -> BlockchainConfig` - core/common/src/config.rs - Loads blockchain config from environment

### Caching
- `CacheManager::get<T>()` - core/common/src/cache.rs - Redis get with deserialization
- `CacheManager::set<T>()` - core/common/src/cache.rs - Redis set with TTL

### Error Handling & Retry
- `retry_with_backoff(operation, max_attempts, initial_delay) -> Result<T>` - core/common/src/retry.rs - Exponential backoff retry

---

## Lending Service (`core/lending-service/src/`)

### Loan Operations
- `create_loan(amount, borrower_id, collateral_txid) -> Result<Loan>` - core/lending-service/src/loans.rs - Creates new loan record
- `calculate_interest(principal, rate, days) -> f64` - core/lending-service/src/loans.rs - Compound interest calculation
- `verify_loan_collateral(loan_id, txid) -> Result<bool>` - core/lending-service/src/blockchain.rs - Lending-specific collateral check

---

## Payment Channel Service (`core/payment-channel-service/src/`)

### Channel Operations
- `create_channel(party_a, party_b, amount_a, amount_b) -> Result<Channel>` - core/payment-channel-service/src/channels.rs - Creates payment channel
- `update_channel_state(channel_id, new_state) -> Result<()>` - core/payment-channel-service/src/channels.rs - Updates channel state
- `close_channel_on_chain(channel_id) -> Result<String>` - core/payment-channel-service/src/settlement.rs - On-chain settlement

---

## Blockchain Monitor (`core/blockchain-monitor/src/`)

### Monitoring
- `monitor_pending_operations() -> Result<()>` - core/blockchain-monitor/src/monitor.rs - Background polling loop
- `check_tx_status(txid) -> Result<TxStatus>` - core/blockchain-monitor/src/monitor.rs - Queries TX status

---

3. Decision Framework

┌─ Is this function used by 2+ services?
│  ├─ YES → Goes in core/common/src/
│  └─ NO → Continue...
│
└─ Is this specific to ONE service's business logic?
   ├─ YES → Goes in that service's src/
   └─ MAYBE (looks common but only one service uses it now)
      ├─ Document it in functions registry as SERVICE-SPECIFIC
      ├─ If another service needs it later, MOVE to common
      └─ Don't premature-optimize for "maybe"


4. Process

Flywheel is never-ending iterative circle between architecture/structure -> services/workspaces -> functions -> dependencies -> structure -> workspaces -> functions -> etc.
