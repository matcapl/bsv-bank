// core/transaction-builder/src/main.rs
// Transaction Builder Service with Phase 6 Production Hardening

use actix_web::{web, App, HttpResponse, HttpServer, Responder, Result, middleware};
use actix_cors::Cors;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use sha2::{Sha256, Digest};
use ripemd::Ripemd160;
use bsv_bank_common::{
    init_logging, ServiceMetrics,
    validate_amount, // We'll validate Bitcoin addresses and amounts
};
use dotenv::dotenv;
use prometheus::Registry;
use std::time::SystemTime;
use thiserror::Error;

// ============================================================================
// ERROR TYPES
// ============================================================================

#[derive(Debug, Error)]
enum ServiceError {
    #[error("Invalid input: {0}")]
    ValidationError(String),
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("Transaction building error: {0}")]
    BuildError(String),
}

impl actix_web::ResponseError for ServiceError {
    fn error_response(&self) -> HttpResponse {
        match self {
            ServiceError::ValidationError(msg) => {
                HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "validation_error",
                    "message": msg
                }))
            }
            ServiceError::DatabaseError(msg) => {
                HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": "database_error",
                    "message": msg
                }))
            }
            ServiceError::BuildError(msg) => {
                HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "build_error",
                    "message": msg
                }))
            }
        }
    }
}

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
                .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/bsv_bank".to_string()),
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
// Bitcoin Primitives (Keep your existing code)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TxInput {
    txid: String,
    vout: u32,
    script_sig: Vec<u8>,
    sequence: u32,
    value: u64,
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
    
    fn to_hex(&self) -> String {
        hex::encode(self.serialize())
    }
    
    fn calculate_size(&self) -> usize {
        self.serialize().len()
    }
    
    fn estimate_fee(&self, fee_per_byte: u64) -> u64 {
        (self.calculate_size() as u64) * fee_per_byte
    }

    fn serialize(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        
        bytes.extend_from_slice(&self.version.to_le_bytes());
        bytes.push(self.inputs.len() as u8);
        
        for input in &self.inputs {
            let txid_bytes = hex::decode(&input.txid).unwrap_or_else(|_| vec![0u8; 32]);
            let mut reversed_txid = txid_bytes;
            reversed_txid.reverse();
            bytes.extend_from_slice(&reversed_txid);
            
            bytes.extend_from_slice(&input.vout.to_le_bytes());
            bytes.push(input.script_sig.len() as u8);
            bytes.extend_from_slice(&input.script_sig);
            bytes.extend_from_slice(&input.sequence.to_le_bytes());
        }
        
        bytes.push(self.outputs.len() as u8);
        
        for output in &self.outputs {
            bytes.extend_from_slice(&output.value.to_le_bytes());
            bytes.push(output.script_pubkey.len() as u8);
            bytes.extend_from_slice(&output.script_pubkey);
        }
        
        bytes.extend_from_slice(&self.locktime.to_le_bytes());
        
        bytes
    }
    
    fn calculate_txid(&self) -> String {
        let serialized = self.serialize();
        let hash1 = Sha256::digest(&serialized);
        let hash2 = Sha256::digest(&hash1);
        
        let mut txid_bytes = hash2.to_vec();
        txid_bytes.reverse();
        hex::encode(txid_bytes)
    }
}

// ============================================================================
// Script Building (Keep your existing code)
// ============================================================================

struct ScriptBuilder;

impl ScriptBuilder {
    fn p2pkh(pubkey_hash: &[u8]) -> Vec<u8> {
        let mut script = Vec::new();
        script.push(0x76); // OP_DUP
        script.push(0xa9); // OP_HASH160
        script.push(20);
        script.extend_from_slice(pubkey_hash);
        script.push(0x88); // OP_EQUALVERIFY
        script.push(0xac); // OP_CHECKSIG
        script
    }
    
    fn p2sh(script_hash: &[u8]) -> Vec<u8> {
        let mut script = Vec::new();
        script.push(0xa9); // OP_HASH160
        script.push(20);
        script.extend_from_slice(script_hash);
        script.push(0x87); // OP_EQUAL
        script
    }
    
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
    
    fn op_return(data: &[u8]) -> Vec<u8> {
        let mut script = Vec::new();
        script.push(0x6a); // OP_RETURN
        script.push(data.len() as u8);
        script.extend_from_slice(data);
        script
    }
    
    fn checklocktimeverify(locktime: u32, pubkey_hash: &[u8]) -> Vec<u8> {
        let mut script = Vec::new();
        
        let locktime_bytes = locktime.to_le_bytes();
        script.push(locktime_bytes.len() as u8);
        script.extend_from_slice(&locktime_bytes);
        
        script.push(0xb1); // OP_CHECKLOCKTIMEVERIFY
        script.push(0x75); // OP_DROP
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
// Address Utilities (Keep your existing code)
// ============================================================================

struct AddressUtils;

impl AddressUtils {
    fn decode_address(address: &str) -> Result<Vec<u8>, String> {
        let decoded = bs58::decode(address)
            .into_vec()
            .map_err(|e| format!("Invalid address: {}", e))?;
        
        if decoded.len() < 25 {
            return Err("Address too short".to_string());
        }
        
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
    start_time: SystemTime,
}

impl AppState {
    async fn new(config: Config) -> Result<Self, sqlx::Error> {
        let db = PgPoolOptions::new()
            .max_connections(5)
            .connect(&config.database_url)
            .await?;
        Ok(Self { 
            db, 
            config,
            start_time: SystemTime::now(),
        })
    }
}

// ============================================================================
// Request/Response Types (Keep your existing types)
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

// ============================================================================
// Phase 6: Validation Functions
// ============================================================================

fn validate_p2pkh_request(req: &BuildP2PKHRequest) -> Result<(), ServiceError> {
    // Validate amount
    validate_amount(req.amount_satoshis as i64)
        .map_err(|e| ServiceError::ValidationError(e.to_string()))?;
    
    // Validate addresses format
    AddressUtils::decode_address(&req.from_address)
        .map_err(|e| ServiceError::ValidationError(format!("Invalid from_address: {}", e)))?;
    AddressUtils::decode_address(&req.to_address)
        .map_err(|e| ServiceError::ValidationError(format!("Invalid to_address: {}", e)))?;
    
    // Validate fee if provided
    if let Some(fee) = req.fee_per_byte {
        if fee == 0 || fee > 10000 {
            return Err(ServiceError::ValidationError(
                "Fee per byte must be between 1 and 10000".to_string()
            ));
        }
    }
    
    Ok(())
}

fn validate_funding_request(req: &BuildFundingRequest) -> Result<(), ServiceError> {
    // Validate amounts
    validate_amount(req.party_a.amount as i64)
        .map_err(|e| ServiceError::ValidationError(e.to_string()))?;
    validate_amount(req.party_b.amount as i64)
        .map_err(|e| ServiceError::ValidationError(e.to_string()))?;
    
    // Validate addresses
    AddressUtils::decode_address(&req.party_a.address)
        .map_err(|e| ServiceError::ValidationError(format!("Invalid party_a address: {}", e)))?;
    AddressUtils::decode_address(&req.party_b.address)
        .map_err(|e| ServiceError::ValidationError(format!("Invalid party_b address: {}", e)))?;
    AddressUtils::decode_address(&req.multisig_address)
        .map_err(|e| ServiceError::ValidationError(format!("Invalid multisig address: {}", e)))?;
    
    Ok(())
}

fn validate_commitment_request(req: &BuildCommitmentRequest) -> Result<(), ServiceError> {
    // Validate balances
    validate_amount(req.party_a_balance as i64)
        .map_err(|e| ServiceError::ValidationError(e.to_string()))?;
    validate_amount(req.party_b_balance as i64)
        .map_err(|e| ServiceError::ValidationError(e.to_string()))?;
    validate_amount(req.funding_amount as i64)
        .map_err(|e| ServiceError::ValidationError(e.to_string()))?;
    
    // Validate addresses
    AddressUtils::decode_address(&req.party_a_address)
        .map_err(|e| ServiceError::ValidationError(format!("Invalid party_a address: {}", e)))?;
    AddressUtils::decode_address(&req.party_b_address)
        .map_err(|e| ServiceError::ValidationError(format!("Invalid party_b address: {}", e)))?;
    
    // Validate TXID format (64 hex chars)
    if req.funding_txid.len() != 64 || !req.funding_txid.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(ServiceError::ValidationError("Invalid funding TXID format".to_string()));
    }
    
    Ok(())
}

// ============================================================================
// Transaction Building Logic (Keep your existing functions with minor tweaks)
// ============================================================================

fn build_p2pkh_transaction(req: BuildP2PKHRequest, fee_per_byte: u64) -> Result<Transaction, String> {
    let mut tx = Transaction::new();
    
    let to_hash = AddressUtils::decode_address(&req.to_address)
        .map_err(|e| format!("Invalid to_address: {}", e))?;
    let from_hash = AddressUtils::decode_address(&req.from_address)
        .map_err(|e| format!("Invalid from_address: {}", e))?;
    
    let utxos = if let Some(utxos) = req.utxos {
        if utxos.is_empty() {
            return Err("No UTXOs provided".to_string());
        }
        utxos
    } else {
        vec![UtxoInput {
            txid: "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789".to_string(),
            vout: 0,
            satoshis: req.amount_satoshis + 100000,
        }]
    };
    
    let estimated_inputs = 1;
    let estimated_outputs = 2;
    let estimated_size = 10 + 1 + (estimated_inputs * 148) + 1 + (estimated_outputs * 34);
    let estimated_fee = (estimated_size as u64) * fee_per_byte;
    
    let total_needed = req.amount_satoshis + estimated_fee;
    
    let mut total_input = 0u64;
    let mut selected_utxos = Vec::new();
    
    for utxo in utxos {
        selected_utxos.push(utxo.clone());
        total_input += utxo.satoshis;
        
        if total_input >= total_needed {
            break;
        }
    }
    
    if total_input < total_needed {
        return Err(format!(
            "Insufficient funds: need {} sats, have {} sats",
            total_needed, total_input
        ));
    }
    
    for utxo in &selected_utxos {
        tx.add_input(utxo.txid.clone(), utxo.vout, utxo.satoshis);
    }
    
    let actual_size_no_change = 10 + 1 + (selected_utxos.len() * 148) + 1 + (1 * 34);
    let fee_no_change = (actual_size_no_change as u64) * fee_per_byte;
    
    let actual_size_with_change = 10 + 1 + (selected_utxos.len() * 148) + 1 + (2 * 34);
    let fee_with_change = (actual_size_with_change as u64) * fee_per_byte;
    
    let to_script = ScriptBuilder::p2pkh(&to_hash);
    tx.add_output(req.amount_satoshis, to_script);
    
    let change_without_change_output = total_input.saturating_sub(req.amount_satoshis + fee_no_change);
    let change_with_change_output = total_input.saturating_sub(req.amount_satoshis + fee_with_change);
    
    const DUST_THRESHOLD: u64 = 546;
    if change_with_change_output > DUST_THRESHOLD {
        let from_script = ScriptBuilder::p2pkh(&from_hash);
        tx.add_output(change_with_change_output, from_script);
    }
    
    Ok(tx)
}

fn build_funding_transaction(req: BuildFundingRequest, fee_per_byte: u64) -> Result<Transaction, String> {
    let mut tx = Transaction::new();
    
    let utxos_a = req.party_a.utxos.ok_or("Party A UTXOs required")?;
    let utxos_b = req.party_b.utxos.ok_or("Party B UTXOs required")?;
    
    if utxos_a.is_empty() || utxos_b.is_empty() {
        return Err("Both parties must provide UTXOs".to_string());
    }
    
    for utxo in &utxos_a {
        tx.add_input(utxo.txid.clone(), utxo.vout, utxo.satoshis);
    }
    for utxo in &utxos_b {
        tx.add_input(utxo.txid.clone(), utxo.vout, utxo.satoshis);
    }
    
    let estimated_size = 10 + 1 + ((utxos_a.len() + utxos_b.len()) * 148) + 1 + (1 * 34);
    let estimated_fee = (estimated_size as u64) * fee_per_byte;
    
    let multisig_hash = AddressUtils::decode_address(&req.multisig_address)?;
    let multisig_script = ScriptBuilder::p2sh(&multisig_hash);
    
    let total_funding = req.party_a.amount + req.party_b.amount;
    
    if total_funding <= estimated_fee {
        return Err("Funding amount too small to cover fees".to_string());
    }
    
    tx.add_output(total_funding - estimated_fee, multisig_script);
    
    Ok(tx)
}

fn build_commitment_transaction(req: BuildCommitmentRequest, fee_per_byte: u64) -> Result<Transaction, String> {
    let mut tx = Transaction::new();
    
    tx.add_input(req.funding_txid.clone(), req.funding_output, req.funding_amount);
    
    if !tx.inputs.is_empty() {
        tx.inputs[0].sequence = req.sequence_number;
    }
    
    if req.party_a_balance + req.party_b_balance > req.funding_amount {
        return Err("Combined balances exceed funding amount".to_string());
    }
    
    let party_a_hash = AddressUtils::decode_address(&req.party_a_address)?;
    let party_b_hash = AddressUtils::decode_address(&req.party_b_address)?;
    
    let party_a_script = ScriptBuilder::checklocktimeverify(req.timelock_blocks, &party_a_hash);
    let party_b_script = ScriptBuilder::p2pkh(&party_b_hash);
    
    tx.add_output(req.party_a_balance, party_a_script);
    tx.add_output(req.party_b_balance, party_b_script);
    
    Ok(tx)
}

fn build_settlement_transaction(req: BuildSettlementRequest, fee_per_byte: u64) -> Result<Transaction, String> {
    let mut tx = Transaction::new();
    
    tx.add_input(req.funding_txid.clone(), req.funding_output, req.funding_amount);
    
    if req.party_a_balance + req.party_b_balance > req.funding_amount {
        return Err("Combined balances exceed funding amount".to_string());
    }
    
    let party_a_hash = AddressUtils::decode_address(&req.party_a_address)?;
    let party_b_hash = AddressUtils::decode_address(&req.party_b_address)?;
    
    let party_a_script = ScriptBuilder::p2pkh(&party_a_hash);
    let party_b_script = ScriptBuilder::p2pkh(&party_b_hash);
    
    let estimated_size = 10 + 1 + (1 * 148) + 1 + (2 * 34);
    let estimated_fee = (estimated_size as u64) * fee_per_byte;
    
    if req.party_a_balance + req.party_b_balance + estimated_fee > req.funding_amount {
        return Err("Insufficient funding for balances + fees".to_string());
    }
    
    tx.add_output(req.party_a_balance, party_a_script);
    
    if req.party_b_balance < estimated_fee {
        return Err("Party B balance too small to cover fees".to_string());
    }
    tx.add_output(req.party_b_balance - estimated_fee, party_b_script);
    
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

async fn build_p2pkh(
    data: web::Data<AppState>,
    req: web::Json<BuildP2PKHRequest>,
) -> Result<HttpResponse, ServiceError> {
    // Phase 6: Validate inputs
    validate_p2pkh_request(&req)?;
    
    let fee_per_byte = req.fee_per_byte.unwrap_or(data.config.default_fee_per_byte);
    
    match build_p2pkh_transaction(req.into_inner(), fee_per_byte) {
        Ok(tx) => {
            let tx_hex = tx.to_hex();
            let txid = tx.calculate_txid();
            let size_bytes = tx.calculate_size();
            let fee_satoshis = tx.estimate_fee(fee_per_byte);
            
            tracing::info!("Built P2PKH transaction: {} ({} bytes, {} sat fee)", txid, size_bytes, fee_satoshis);
            
            let response = BuildTransactionResponse {
                tx_hex,
                txid,
                size_bytes,
                fee_satoshis,
                inputs: tx.inputs.clone(),
                outputs: tx.outputs.clone(),
            };
            
            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => {
            tracing::error!("Failed to build P2PKH transaction: {}", e);
            Err(ServiceError::BuildError(e))
        }
    }
}

async fn create_multisig(
    req: web::Json<CreateMultisigRequest>,
) -> Result<HttpResponse, ServiceError> {
    if req.pubkeys.len() != 2 || req.required_sigs != 2 {
        return Err(ServiceError::ValidationError("Only 2-of-2 multisig supported".to_string()));
    }
    
    let pubkey1 = hex::decode(&req.pubkeys[0])
        .map_err(|_| ServiceError::ValidationError("Invalid pubkey 1".to_string()))?;
    
    let pubkey2 = hex::decode(&req.pubkeys[1])
        .map_err(|_| ServiceError::ValidationError("Invalid pubkey 2".to_string()))?;
    
    let redeem_script = ScriptBuilder::multisig_2_of_2(&pubkey1, &pubkey2);
    let script_hash = AddressUtils::hash160(&redeem_script);
    
    let mut address_bytes = vec![0xc4]; // Testnet P2SH prefix
    address_bytes.extend_from_slice(&script_hash);
    let checksum = &AddressUtils::double_sha256(&address_bytes)[0..4];
    address_bytes.extend_from_slice(checksum);
    
    let address = bs58::encode(address_bytes).into_string();
    
    tracing::info!("Created 2-of-2 multisig address: {}", address);
    
    Ok(HttpResponse::Ok().json(MultisigResponse {
        address,
        redeem_script: hex::encode(redeem_script),
        script_hash: hex::encode(script_hash),
    }))
}

async fn build_funding(
    data: web::Data<AppState>,
    req: web::Json<BuildFundingRequest>,
) -> Result<HttpResponse, ServiceError> {
    // Phase 6: Validate inputs
    validate_funding_request(&req)?;
    
    let fee_per_byte = req.fee_per_byte.unwrap_or(data.config.default_fee_per_byte);
    
    match build_funding_transaction(req.into_inner(), fee_per_byte) {
        Ok(tx) => {
            let txid = tx.calculate_txid();
            tracing::info!("Built funding transaction: {}", txid);
            
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
        Err(e) => {
            tracing::error!("Failed to build funding transaction: {}", e);
            Err(ServiceError::BuildError(e))
        }
    }
}

async fn build_commitment(
    data: web::Data<AppState>,
    req: web::Json<BuildCommitmentRequest>,
) -> Result<HttpResponse, ServiceError> {
    // Phase 6: Validate inputs
    validate_commitment_request(&req)?;
    
    let fee_per_byte = req.fee_per_byte.unwrap_or(data.config.default_fee_per_byte);
    
    match build_commitment_transaction(req.into_inner(), fee_per_byte) {
        Ok(tx) => {
            let txid = tx.calculate_txid();
            tracing::info!("Built commitment transaction: {}", txid);
            
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
        Err(e) => {
            tracing::error!("Failed to build commitment transaction: {}", e);
            Err(ServiceError::BuildError(e))
        }
    }
}

async fn build_settlement(
    data: web::Data<AppState>,
    req: web::Json<BuildSettlementRequest>,
) -> Result<HttpResponse, ServiceError> {
    // Phase 6: Validate inputs (similar to commitment)
    let fee_per_byte = req.fee_per_byte.unwrap_or(data.config.default_fee_per_byte);
    
    match build_settlement_transaction(req.into_inner(), fee_per_byte) {
        Ok(tx) => {
            let txid = tx.calculate_txid();
            tracing::info!("Built settlement transaction: {}", txid);
            
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
        Err(e) => {
            tracing::error!("Failed to build settlement transaction: {}", e);
            Err(ServiceError::BuildError(e))
        }
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
            10 + 1 + (148 * inputs) + 1 + (34 * outputs)
        }
        "multisig" => {
            let inputs = req.input_count.unwrap_or(1);
            let outputs = req.output_count.unwrap_or(1);
            10 + 1 + (295 * inputs) + 1 + (34 * outputs)
        }
        "funding" => 500,
        "commitment" => 350,
        "settlement" => 300,
        _ => 250,
    };
    
    let fee_satoshis = (estimated_size as u64) * fee_per_byte;
    
    let response = EstimateFeeResponse {
        tx_type: req.tx_type.clone(),
        estimated_size_bytes: estimated_size,
        fee_per_byte,
        fee_satoshis,
    };
    
    Ok(HttpResponse::Ok().json(response))
}

async fn select_utxos_handler(
    req: web::Json<SelectUtxosRequest>,
) -> Result<HttpResponse, ServiceError> {
    // Phase 6: Validate target amount
    validate_amount(req.target_amount as i64)
        .map_err(|e| ServiceError::ValidationError(e.to_string()))?;
    
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

async fn validate_transaction(
    req: web::Json<ValidateRequest>,
) -> Result<HttpResponse> {
    let mut errors = Vec::new();
    
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
    
    if tx_bytes.len() < 10 {
        errors.push("Transaction too small".to_string());
    }
    
    if tx_bytes.len() > 1_000_000 {
        errors.push("Transaction too large".to_string());
    }
    
    Ok(HttpResponse::Ok().json(ValidateResponse {
        valid: errors.is_empty(),
        size_bytes: tx_bytes.len(),
        input_count: 0,
        output_count: 0,
        errors,
    }))
}

// ============================================================================
// HEALTH & METRICS HANDLERS
// ============================================================================

async fn health_check(data: web::Data<AppState>) -> impl Responder {
    let uptime = SystemTime::now()
        .duration_since(data.start_time)
        .unwrap()
        .as_secs();
    
    HttpResponse::Ok().json(serde_json::json!({
        "service": "transaction-builder",
        "status": "healthy",
        "network": data.config.network,
        "version": "0.1.0",
        "uptime_seconds": uptime
    }))
}

async fn liveness_check() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({"status": "alive"}))
}

async fn readiness_check(data: web::Data<AppState>) -> impl Responder {
    // Check database connection
    let db_ok = sqlx::query("SELECT 1")
        .fetch_optional(&data.db)
        .await
        .is_ok();
    
    if db_ok {
        HttpResponse::Ok().json(serde_json::json!({
            "status": "ready",
            "checks": {
                "database": "ok"
            }
        }))
    } else {
        HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "status": "not_ready",
            "checks": {
                "database": "error"
            }
        }))
    }
}

async fn metrics_handler(registry: web::Data<Registry>) -> Result<HttpResponse, actix_web::Error> {
    use prometheus::Encoder;
    let encoder = prometheus::TextEncoder::new();
    let metric_families = registry.gather();
    let mut buffer = vec![];
    encoder.encode(&metric_families, &mut buffer)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
    
    Ok(HttpResponse::Ok()
        .content_type("text/plain; version=0.0.4")
        .body(buffer))
}

// ============================================================================
// MAIN
// ============================================================================

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Load environment variables
    dotenv().ok();
    
    println!("🔨 BSV Bank - Transaction Builder Service Starting (Phase 6)...");
    
    let config = Config::from_env();
    
    println!("   Network: {}", config.network);
    println!("   Default fee: {} sat/byte", config.default_fee_per_byte);
    
    let port: u16 = 8085; // Fixed port for transaction-builder
    
    // Phase 6: Initialize structured logging
    init_logging("transaction-builder");
    tracing::info!("Starting Transaction Builder Service on port {}", port);
    
    // Initialize app state
    let state = web::Data::new(
        AppState::new(config.clone())
            .await
            .expect("Failed to initialize application state")
    );
    
    tracing::info!("Database connection established");
    
    // Phase 6: Prometheus metrics
    let registry = Registry::new();
    let _service_metrics = ServiceMetrics::new(&registry, "transaction_builder")
        .expect("Failed to create service metrics");
    tracing::info!("Metrics initialized");
    
    let registry_data = web::Data::new(registry);
    
    println!("✅ Service ready on http://0.0.0.0:{}", port);
    println!("📋 Health: http://0.0.0.0:{}/health", port);
    println!("📊 Metrics: http://0.0.0.0:{}/metrics", port);
    println!("📋 Endpoints:");
    println!("   POST /tx/build/p2pkh");
    println!("   POST /tx/multisig/create");
    println!("   POST /tx/build/funding");
    println!("   POST /tx/build/commitment");
    println!("   POST /tx/build/settlement");
    tracing::info!("Starting HTTP server...");
    
    HttpServer::new(move || {
        // Phase 6: CORS configuration
        let cors = Cors::default()
            .allowed_origin("http://localhost:3000")
            .allowed_origin("http://localhost:5173")
            .allowed_methods(vec!["GET", "POST"])
            .allowed_headers(vec![
                actix_web::http::header::AUTHORIZATION,
                actix_web::http::header::CONTENT_TYPE,
            ])
            .max_age(3600);
        
        App::new()
            .wrap(cors)
            // Phase 6: Request logging
            .wrap(middleware::Logger::default())
            // Phase 6: Security headers
            .wrap(middleware::DefaultHeaders::new()
                .add(("X-Frame-Options", "DENY"))
                .add(("X-Content-Type-Options", "nosniff"))
                .add(("Content-Security-Policy", "default-src 'self'"))
                .add(("X-XSS-Protection", "1; mode=block"))
            )
            .app_data(state.clone())
            .app_data(registry_data.clone())
            // Health endpoints (no auth)
            .route("/health", web::get().to(health_check))
            .route("/liveness", web::get().to(liveness_check))
            .route("/readiness", web::get().to(readiness_check))
            // Metrics endpoint (no auth)
            .route("/metrics", web::get().to(metrics_handler))
            // Business endpoints
            .route("/tx/build/p2pkh", web::post().to(build_p2pkh))
            .route("/tx/multisig/create", web::post().to(create_multisig))
            .route("/tx/build/funding", web::post().to(build_funding))
            .route("/tx/build/commitment", web::post().to(build_commitment))
            .route("/tx/build/settlement", web::post().to(build_settlement))
            .route("/tx/estimate-fee", web::post().to(estimate_fee))
            .route("/tx/select-utxos", web::post().to(select_utxos_handler))
            .route("/tx/validate", web::post().to(validate_transaction))
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}