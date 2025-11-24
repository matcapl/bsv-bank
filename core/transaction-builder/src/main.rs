// services/transaction-builder/src/main.rs
// Transaction Builder Service - Port 8085
// Builds and signs BSV transactions for channels

use actix_web::{web, App, HttpResponse, HttpServer, Result};
use actix_cors::Cors;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use sha2::{Sha256, Digest};
use ripemd::Ripemd160;

// ============================================================================
// Configuration
// ============================================================================

#[derive(Debug, Clone)]
struct Config {
    database_url: String,
    network: String,
    default_fee_per_byte: u64,
}

impl Config {
    fn from_env() -> Self {
        Self {
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgresql://a:a@localhost/bsv_bank".to_string()),
            network: std::env::var("NETWORK")
                .unwrap_or_else(|_| "testnet".to_string()),
            default_fee_per_byte: std::env::var("FEE_PER_BYTE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(50),
        }
    }
}

// ============================================================================
// Bitcoin Primitives
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TxInput {
    txid: String,
    vout: u32,
    script_sig: Vec<u8>,
    sequence: u32,
    value: u64, // For fee calculation
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TxOutput {
    value: u64,
    script_pubkey: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Transaction {
    version: u32,
    inputs: Vec<TxInput>,
    outputs: Vec<TxOutput>,
    locktime: u32,
}

impl Transaction {
    fn new() -> Self {
        Self {
            version: 1,
            inputs: Vec::new(),
            outputs: Vec::new(),
            locktime: 0,
        }
    }
    
    fn add_input(&mut self, txid: String, vout: u32, value: u64) {
        self.inputs.push(TxInput {
            txid,
            vout,
            script_sig: Vec::new(),
            sequence: 0xffffffff,
            value,
        });
    }
    
    fn add_output(&mut self, value: u64, script_pubkey: Vec<u8>) {
        self.outputs.push(TxOutput {
            value,
            script_pubkey,
        });
    }
    
    fn serialize(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        
        // Version
        bytes.extend_from_slice(&self.version.to_le_bytes());
        
        // Input count
        bytes.push(self.inputs.len() as u8);
        
        // Inputs
        for input in &self.inputs {
            // Previous output (txid + vout)
            bytes.extend_from_slice(&hex::decode(&input.txid).unwrap_or_default());
            bytes.extend_from_slice(&input.vout.to_le_bytes());
            
            // Script length + script
            bytes.push(input.script_sig.len() as u8);
            bytes.extend_from_slice(&input.script_sig);
            
            // Sequence
            bytes.extend_from_slice(&input.sequence.to_le_bytes());
        }
        
        // Output count
        bytes.push(self.outputs.len() as u8);
        
        // Outputs
        for output in &self.outputs {
            // Value
            bytes.extend_from_slice(&output.value.to_le_bytes());
            
            // Script length + script
            bytes.push(output.script_pubkey.len() as u8);
            bytes.extend_from_slice(&output.script_pubkey);
        }
        
        // Locktime
        bytes.extend_from_slice(&self.locktime.to_le_bytes());
        
        bytes
    }
    
    fn to_hex(&self) -> String {
        hex::encode(self.serialize())
    }
    
    fn calculate_txid(&self) -> String {
        let serialized = self.serialize();
        let hash1 = Sha256::digest(&serialized);
        let hash2 = Sha256::digest(&hash1);
        hex::encode(hash2)
    }
    
    fn calculate_size(&self) -> usize {
        self.serialize().len()
    }
    
    fn estimate_fee(&self, fee_per_byte: u64) -> u64 {
        (self.calculate_size() as u64) * fee_per_byte
    }
}

// ============================================================================
// Script Building
// ============================================================================

struct ScriptBuilder;

impl ScriptBuilder {
    // P2PKH script: OP_DUP OP_HASH160 <pubkeyhash> OP_EQUALVERIFY OP_CHECKSIG
    fn p2pkh(pubkey_hash: &[u8]) -> Vec<u8> {
        let mut script = Vec::new();
        script.push(0x76); // OP_DUP
        script.push(0xa9); // OP_HASH160
        script.push(20);   // Push 20 bytes
        script.extend_from_slice(pubkey_hash);
        script.push(0x88); // OP_EQUALVERIFY
        script.push(0xac); // OP_CHECKSIG
        script
    }
    
    // P2SH script: OP_HASH160 <scripthash> OP_EQUAL
    fn p2sh(script_hash: &[u8]) -> Vec<u8> {
        let mut script = Vec::new();
        script.push(0xa9); // OP_HASH160
        script.push(20);   // Push 20 bytes
        script.extend_from_slice(script_hash);
        script.push(0x87); // OP_EQUAL
        script
    }
    
    // 2-of-2 Multisig: OP_2 <pubkey1> <pubkey2> OP_2 OP_CHECKMULTISIG
    fn multisig_2_of_2(pubkey1: &[u8], pubkey2: &[u8]) -> Vec<u8> {
        let mut script = Vec::new();
        script.push(0x52); // OP_2
        script.push(pubkey1.len() as u8);
        script.extend_from_slice(pubkey1);
        script.push(pubkey2.len() as u8);
        script.extend_from_slice(pubkey2);
        script.push(0x52); // OP_2
        script.push(0xae); // OP_CHECKMULTISIG
        script
    }
    
    // OP_RETURN script for data
    fn op_return(data: &[u8]) -> Vec<u8> {
        let mut script = Vec::new();
        script.push(0x6a); // OP_RETURN
        script.push(data.len() as u8);
        script.extend_from_slice(data);
        script
    }
    
    // CHECKLOCKTIMEVERIFY script
    fn checklocktimeverify(locktime: u32, pubkey_hash: &[u8]) -> Vec<u8> {
        let mut script = Vec::new();
        
        // Push locktime value
        let locktime_bytes = locktime.to_le_bytes();
        script.push(locktime_bytes.len() as u8);
        script.extend_from_slice(&locktime_bytes);
        
        script.push(0xb1); // OP_CHECKLOCKTIMEVERIFY
        script.push(0x75); // OP_DROP
        
        // Standard P2PKH after timelock
        script.push(0x76); // OP_DUP
        script.push(0xa9); // OP_HASH160
        script.push(20);
        script.extend_from_slice(pubkey_hash);
        script.push(0x88); // OP_EQUALVERIFY
        script.push(0xac); // OP_CHECKSIG
        
        script
    }
}

// ============================================================================
// Address Utilities
// ============================================================================

struct AddressUtils;

impl AddressUtils {
    fn decode_address(address: &str) -> Result<Vec<u8>, String> {
        // Simplified base58 decode (for testnet addresses)
        // In production, use proper base58 library
        let decoded = bs58::decode(address)
            .into_vec()
            .map_err(|e| format!("Invalid address: {}", e))?;
        
        if decoded.len() < 25 {
            return Err("Address too short".to_string());
        }
        
        // Extract pubkey hash (skip version byte, checksum)
        Ok(decoded[1..21].to_vec())
    }
    
    fn hash160(data: &[u8]) -> Vec<u8> {
        let sha256_hash = Sha256::digest(data);
        let ripemd160_hash = Ripemd160::digest(&sha256_hash);
        ripemd160_hash.to_vec()
    }
    
    fn double_sha256(data: &[u8]) -> Vec<u8> {
        let hash1 = Sha256::digest(data);
        let hash2 = Sha256::digest(&hash1);
        hash2.to_vec()
    }
}

// ============================================================================
// Application State
// ============================================================================

struct AppState {
    db: PgPool,
    config: Config,
}

impl AppState {
    async fn new(config: Config) -> Result<Self, sqlx::Error> {
        let db = PgPool::connect(&config.database_url).await?;
        Ok(Self { db, config })
    }
}

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Deserialize)]
struct BuildP2PKHRequest {
    from_address: String,
    to_address: String,
    amount_satoshis: u64,
    fee_per_byte: Option<u64>,
    utxos: Option<Vec<UtxoInput>>,
}

#[derive(Deserialize, Clone, Serialize)]
struct UtxoInput {
    txid: String,
    vout: u32,
    satoshis: u64,
}

#[derive(Serialize)]
struct BuildTransactionResponse {
    tx_hex: String,
    txid: String,
    size_bytes: usize,
    fee_satoshis: u64,
    inputs: Vec<TxInput>,
    outputs: Vec<TxOutput>,
}

#[derive(Deserialize)]
struct CreateMultisigRequest {
    pubkeys: Vec<String>,
    required_sigs: u8,
}

#[derive(Serialize)]
struct MultisigResponse {
    address: String,
    redeem_script: String,
    script_hash: String,
}

#[derive(Deserialize)]
struct BuildFundingRequest {
    party_a: PartyInput,
    party_b: PartyInput,
    multisig_address: String,
    fee_per_byte: Option<u64>,
}

#[derive(Deserialize)]
struct PartyInput {
    address: String,
    amount: u64,
    utxos: Option<Vec<UtxoInput>>,
}

#[derive(Deserialize)]
struct BuildCommitmentRequest {
    funding_txid: String,
    funding_output: u32,
    funding_amount: u64,
    party_a_balance: u64,
    party_b_balance: u64,
    party_a_address: String,
    party_b_address: String,
    sequence_number: u32,
    timelock_blocks: u32,
    fee_per_byte: Option<u64>,
}

#[derive(Deserialize)]
struct BuildSettlementRequest {
    funding_txid: String,
    funding_output: u32,
    funding_amount: u64,
    party_a_balance: u64,
    party_b_balance: u64,
    party_a_address: String,
    party_b_address: String,
    fee_per_byte: Option<u64>,
}

#[derive(Deserialize)]
struct EstimateFeeRequest {
    tx_type: String,
    input_count: Option<usize>,
    output_count: Option<usize>,
    fee_per_byte: Option<u64>,
}

#[derive(Serialize)]
struct EstimateFeeResponse {
    tx_type: String,
    estimated_size_bytes: usize,
    fee_per_byte: u64,
    fee_satoshis: u64,
}

#[derive(Deserialize)]
struct SelectUtxosRequest {
    utxos: Vec<UtxoInput>,
    target_amount: u64,
    strategy: Option<String>,
}

#[derive(Serialize)]
struct SelectUtxosResponse {
    selected_utxos: Vec<UtxoInput>,
    total_value: u64,
    change_amount: u64,
}

// ============================================================================
// Transaction Building Logic
// ============================================================================

fn build_p2pkh_transaction(req: BuildP2PKHRequest, fee_per_byte: u64) -> Result<Transaction, String> {
    let mut tx = Transaction::new();
    
    // Decode addresses
    let from_hash = AddressUtils::decode_address(&req.from_address)?;
    let to_hash = AddressUtils::decode_address(&req.to_address)?;
    
    // Calculate required input amount (output + estimated fee)
    let estimated_size = 250; // Rough estimate for 1-in-2-out transaction
    let estimated_fee = (estimated_size as u64) * fee_per_byte;
    let total_needed = req.amount_satoshis + estimated_fee;
    
    // Select UTXOs (simplified - use provided or mock)
    let utxos = req.utxos.unwrap_or_else(|| {
        vec![UtxoInput {
            txid: format!("{:064x}", 0),
            vout: 0,
            satoshis: total_needed + 10000, // Add buffer
        }]
    });
    
    let mut total_input = 0u64;
    for utxo in &utxos {
        tx.add_input(utxo.txid.clone(), utxo.vout, utxo.satoshis);
        total_input += utxo.satoshis;
        
        if total_input >= total_needed {
            break;
        }
    }
    
    if total_input < total_needed {
        return Err("Insufficient funds".to_string());
    }
    
    // Calculate actual fee
    let actual_fee = tx.estimate_fee(fee_per_byte);
    
    // Add output to recipient
    let to_script = ScriptBuilder::p2pkh(&to_hash);
    tx.add_output(req.amount_satoshis, to_script);
    
    // Add change output if needed
    let change = total_input.saturating_sub(req.amount_satoshis + actual_fee);
    if change > 546 { // Dust threshold
        let from_script = ScriptBuilder::p2pkh(&from_hash);
        tx.add_output(change, from_script);
    }
    
    Ok(tx)
}

fn build_funding_transaction(req: BuildFundingRequest, fee_per_byte: u64) -> Result<Transaction, String> {
    let mut tx = Transaction::new();
    
    // Get UTXOs for both parties
    let utxos_a = req.party_a.utxos.ok_or("Party A UTXOs required")?;
    let utxos_b = req.party_b.utxos.ok_or("Party B UTXOs required")?;
    
    // Add inputs from both parties
    for utxo in utxos_a {
        tx.add_input(utxo.txid, utxo.vout, utxo.satoshis);
    }
    for utxo in utxos_b {
        tx.add_input(utxo.txid, utxo.vout, utxo.satoshis);
    }
    
    // Calculate fee
    let fee = tx.estimate_fee(fee_per_byte);
    
    // Add output to multisig address
    let multisig_hash = AddressUtils::decode_address(&req.multisig_address)?;
    let multisig_script = ScriptBuilder::p2sh(&multisig_hash);
    
    let total_funding = req.party_a.amount + req.party_b.amount;
    tx.add_output(total_funding - fee, multisig_script);
    
    Ok(tx)
}

fn build_commitment_transaction(req: BuildCommitmentRequest, fee_per_byte: u64) -> Result<Transaction, String> {
    let mut tx = Transaction::new();
    
    // Add input from funding transaction
    tx.add_input(req.funding_txid, req.funding_output, req.funding_amount);
    
    // Set sequence number for this commitment
    if !tx.inputs.is_empty() {
        tx.inputs[0].sequence = req.sequence_number;
    }
    
    // Calculate fee
    let fee = tx.estimate_fee(fee_per_byte);
    
    // Add output to party A with timelock
    let party_a_hash = AddressUtils::decode_address(&req.party_a_address)?;
    let party_a_script = ScriptBuilder::checklocktimeverify(req.timelock_blocks, &party_a_hash);
    tx.add_output(req.party_a_balance, party_a_script);
    
    // Add output to party B (immediate spend)
    let party_b_hash = AddressUtils::decode_address(&req.party_b_address)?;
    let party_b_script = ScriptBuilder::p2pkh(&party_b_hash);
    tx.add_output(req.party_b_balance - fee, party_b_script);
    
    Ok(tx)
}

fn build_settlement_transaction(req: BuildSettlementRequest, fee_per_byte: u64) -> Result<Transaction, String> {
    let mut tx = Transaction::new();
    
    // Add input from funding transaction
    tx.add_input(req.funding_txid, req.funding_output, req.funding_amount);
    
    // Calculate fee
    let fee = tx.estimate_fee(fee_per_byte);
    
    // Add outputs for final balances
    let party_a_hash = AddressUtils::decode_address(&req.party_a_address)?;
    let party_a_script = ScriptBuilder::p2pkh(&party_a_hash);
    tx.add_output(req.party_a_balance, party_a_script);
    
    let party_b_hash = AddressUtils::decode_address(&req.party_b_address)?;
    let party_b_script = ScriptBuilder::p2pkh(&party_b_hash);
    tx.add_output(req.party_b_balance - fee, party_b_script);
    
    Ok(tx)
}

fn select_utxos(utxos: Vec<UtxoInput>, target: u64, strategy: &str) -> Vec<UtxoInput> {
    let mut sorted_utxos = utxos;
    
    match strategy {
        "smallest" => {
            sorted_utxos.sort_by_key(|u| u.satoshis);
        }
        "largest" => {
            sorted_utxos.sort_by_key(|u| std::cmp::Reverse(u.satoshis));
        }
        _ => {
            // Default: largest first
            sorted_utxos.sort_by_key(|u| std::cmp::Reverse(u.satoshis));
        }
    }
    
    let mut selected = Vec::new();
    let mut total = 0u64;
    
    for utxo in sorted_utxos {
        selected.push(utxo.clone());
        total += utxo.satoshis;
        
        if total >= target {
            break;
        }
    }
    
    selected
}

// ============================================================================
// HTTP Handlers
// ============================================================================

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    service: String,
    network: String,
    version: String,
}

async fn health_check(data: web::Data<AppState>) -> Result<HttpResponse> {
    Ok(HttpResponse::Ok().json(HealthResponse {
        status: "healthy".to_string(),
        service: "transaction-builder".to_string(),
        network: data.config.network.clone(),
        version: "0.1.0".to_string(),
    }))
}

async fn build_p2pkh(
    data: web::Data<AppState>,
    req: web::Json<BuildP2PKHRequest>,
) -> Result<HttpResponse> {
    let fee_per_byte = req.fee_per_byte.unwrap_or(data.config.default_fee_per_byte);
    
    match build_p2pkh_transaction(req.into_inner(), fee_per_byte) {
        Ok(tx) => {
            let response = BuildTransactionResponse {
                txid: tx.calculate_txid(),
                tx_hex: tx.to_hex(),
                size_bytes: tx.calculate_size(),
                fee_satoshis: tx.estimate_fee(fee_per_byte),
                inputs: tx.inputs.clone(),
                outputs: tx.outputs.clone(),
            };
            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Failed to build transaction",
            "details": e
        })))
    }
}

async fn create_multisig(
    req: web::Json<CreateMultisigRequest>,
) -> Result<HttpResponse> {
    if req.pubkeys.len() != 2 || req.required_sigs != 2 {
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Only 2-of-2 multisig supported"
        })));
    }
    
    let pubkey1 = match hex::decode(&req.pubkeys[0]) {
        Ok(pk) => pk,
        Err(_) => {
            return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid pubkey 1"
            })));
        }
    };
    
    let pubkey2 = match hex::decode(&req.pubkeys[1]) {
        Ok(pk) => pk,
        Err(_) => {
            return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid pubkey 2"
            })));
        }
    };
    
    let redeem_script = ScriptBuilder::multisig_2_of_2(&pubkey1, &pubkey2);
    let script_hash = AddressUtils::hash160(&redeem_script);
    
    // Create P2SH address (testnet prefix: 0xc4)
    let mut address_bytes = vec![0xc4];
    address_bytes.extend_from_slice(&script_hash);
    let checksum = &AddressUtils::double_sha256(&address_bytes)[0..4];
    address_bytes.extend_from_slice(checksum);
    
    let address = bs58::encode(address_bytes).into_string();
    
    Ok(HttpResponse::Ok().json(MultisigResponse {
        address,
        redeem_script: hex::encode(redeem_script),
        script_hash: hex::encode(script_hash),
    }))
}

async fn build_funding(
    data: web::Data<AppState>,
    req: web::Json<BuildFundingRequest>,
) -> Result<HttpResponse> {
    let fee_per_byte = req.fee_per_byte.unwrap_or(data.config.default_fee_per_byte);
    
    match build_funding_transaction(req.into_inner(), fee_per_byte) {
        Ok(tx) => {
            let response = BuildTransactionResponse {
                txid: tx.calculate_txid(),
                tx_hex: tx.to_hex(),
                size_bytes: tx.calculate_size(),
                fee_satoshis: tx.estimate_fee(fee_per_byte),
                inputs: tx.inputs.clone(),
                outputs: tx.outputs.clone(),
            };
            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Failed to build funding transaction",
            "details": e
        })))
    }
}

async fn build_commitment(
    data: web::Data<AppState>,
    req: web::Json<BuildCommitmentRequest>,
) -> Result<HttpResponse> {
    let fee_per_byte = req.fee_per_byte.unwrap_or(data.config.default_fee_per_byte);
    
    match build_commitment_transaction(req.into_inner(), fee_per_byte) {
        Ok(tx) => {
            let response = BuildTransactionResponse {
                txid: tx.calculate_txid(),
                tx_hex: tx.to_hex(),
                size_bytes: tx.calculate_size(),
                fee_satoshis: tx.estimate_fee(fee_per_byte),
                inputs: tx.inputs.clone(),
                outputs: tx.outputs.clone(),
            };
            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Failed to build commitment transaction",
            "details": e
        })))
    }
}

async fn build_settlement(
    data: web::Data<AppState>,
    req: web::Json<BuildSettlementRequest>,
) -> Result<HttpResponse> {
    let fee_per_byte = req.fee_per_byte.unwrap_or(data.config.default_fee_per_byte);
    
    match build_settlement_transaction(req.into_inner(), fee_per_byte) {
        Ok(tx) => {
            let response = BuildTransactionResponse {
                txid: tx.calculate_txid(),
                tx_hex: tx.to_hex(),
                size_bytes: tx.calculate_size(),
                fee_satoshis: tx.estimate_fee(fee_per_byte),
                inputs: tx.inputs.clone(),
                outputs: tx.outputs.clone(),
            };
            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Failed to build settlement transaction",
            "details": e
        })))
    }
}

async fn estimate_fee(
    data: web::Data<AppState>,
    req: web::Json<EstimateFeeRequest>,
) -> Result<HttpResponse> {
    let fee_per_byte = req.fee_per_byte.unwrap_or(data.config.default_fee_per_byte);
    
    let estimated_size = match req.tx_type.as_str() {
        "p2pkh" => {
            let inputs = req.input_count.unwrap_or(1);
            let outputs = req.output_count.unwrap_or(2);
            148 * inputs + 34 * outputs + 10
        }
        "multisig" => {
            let inputs = req.input_count.unwrap_or(1);
            let outputs = req.output_count.unwrap_or(1);
            295 * inputs + 34 * outputs + 10
        }
        "funding" => 500,
        "commitment" => 350,
        "settlement" => 300,
        _ => 250,
    };
    
    Ok(HttpResponse::Ok().json(EstimateFeeResponse {
        tx_type: req.tx_type.clone(),
        estimated_size_bytes: estimated_size,
        fee_per_byte,
        fee_satoshis: (estimated_size as u64) * fee_per_byte,
    }))
}

async fn select_utxos_handler(
    req: web::Json<SelectUtxosRequest>,
) -> Result<HttpResponse> {
    let strategy = req.strategy.as_deref().unwrap_or("largest");
    let selected = select_utxos(req.utxos.clone(), req.target_amount, strategy);
    
    let total_value: u64 = selected.iter().map(|u| u.satoshis).sum();
    let change = total_value.saturating_sub(req.target_amount);
    
    Ok(HttpResponse::Ok().json(SelectUtxosResponse {
        selected_utxos: selected,
        total_value,
        change_amount: change,
    }))
}

#[derive(Deserialize)]
struct ValidateRequest {
    tx_hex: String,
}

#[derive(Serialize)]
struct ValidateResponse {
    valid: bool,
    size_bytes: usize,
    input_count: usize,
    output_count: usize,
    errors: Vec<String>,
}

async fn validate_transaction(
    req: web::Json<ValidateRequest>,
) -> Result<HttpResponse> {
    let mut errors = Vec::new();
    
    // Try to decode hex
    let tx_bytes = match hex::decode(&req.tx_hex) {
        Ok(bytes) => bytes,
        Err(e) => {
            errors.push(format!("Invalid hex: {}", e));
            return Ok(HttpResponse::Ok().json(ValidateResponse {
                valid: false,
                size_bytes: 0,
                input_count: 0,
                output_count: 0,
                errors,
            }));
        }
    };
    
    // Basic validation
    if tx_bytes.len() < 10 {
        errors.push("Transaction too small".to_string());
    }
    
    if tx_bytes.len() > 1_000_000 {
        errors.push("Transaction too large".to_string());
    }
    
    Ok(HttpResponse::Ok().json(ValidateResponse {
        valid: errors.is_empty(),
        size_bytes: tx_bytes.len(),
        input_count: 0, // Would need proper parsing
        output_count: 0,
        errors,
    }))
}

// ============================================================================
// Main Application
// ============================================================================

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    
    let config = Config::from_env();
    println!("🚀 Starting Transaction Builder Service");
    println!("   Network: {}", config.network);
    println!("   Default fee: {} sat/byte", config.default_fee_per_byte);
    
    let state = web::Data::new(
        AppState::new(config.clone())
            .await
            .expect("Failed to initialize application state")
    );
    
    println!("✓ Server starting on http://127.0.0.1:8085");
    
    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);
        
        App::new()
            .wrap(cors)
            .app_data(state.clone())
            .route("/health", web::get().to(health_check))
            .route("/tx/build/p2pkh", web::post().to(build_p2pkh))
            .route("/tx/multisig/create", web::post().to(create_multisig))
            .route("/tx/build/funding", web::post().to(build_funding))
            .route("/tx/build/commitment", web::post().to(build_commitment))
            .route("/tx/build/settlement", web::post().to(build_settlement))
            .route("/tx/estimate-fee", web::post().to(estimate_fee))
            .route("/tx/select-utxos", web::post().to(select_utxos_handler))
            .route("/tx/validate", web::post().to(validate_transaction))
    })
    .bind("127.0.0.1:8085")?
    .run()
    .await
}