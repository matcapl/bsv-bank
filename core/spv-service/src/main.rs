// // core/spv-service/src/main.rs
// // SPV Verification Service - Port 8086
// // Verifies Bitcoin transactions using Simplified Payment Verification

// use actix_web::{web, App, HttpResponse, HttpServer, Result};
// use actix_cors::Cors;
// use serde::{Deserialize, Serialize};
// use sqlx::{PgPool, Row};
// use sha2::{Sha256, Digest};
// use chrono::{DateTime, Utc};
// // use common::{auth, validation};

// // ============================================================================
// // Configuration
// // ============================================================================

// #[derive(Debug, Clone)]
// struct Config {
//     database_url: String,
//     woc_api_base: String,
//     network: String,
//     min_confirmations: u32,
// }

// impl Config {
//     fn from_env() -> Self {
//         Self {
//             database_url: std::env::var("DATABASE_URL")
//                 .unwrap_or_else(|_| "postgresql://a:a@localhost/bsv_bank".to_string()),
//             woc_api_base: std::env::var("WOC_API_BASE")
//                 .unwrap_or_else(|_| "https://api.whatsonchain.com/v1/bsv/test".to_string()),
//             network: std::env::var("NETWORK")
//                 .unwrap_or_else(|_| "testnet".to_string()),
//             min_confirmations: std::env::var("MIN_CONFIRMATIONS")
//                 .ok()
//                 .and_then(|s| s.parse().ok())
//                 .unwrap_or(1),
//         }
//     }
// }

// // ============================================================================
// // Data Types
// // ============================================================================

// #[derive(Debug, Clone, Serialize, Deserialize)]
// struct BlockHeader {
//     height: i32,
//     hash: String,
//     version: i32,
//     prev_block: String,
//     merkle_root: String,
//     timestamp: i64,
//     bits: i32,
//     nonce: i64,
//     difficulty: f64,
// }

// #[derive(Debug, Clone, Serialize, Deserialize)]
// struct MerkleProof {
//     txid: String,
//     block_hash: String,
//     block_height: Option<i32>,
//     merkle_root: String,
//     siblings: Vec<String>,
//     tx_index: i32,
// }

// #[derive(Debug, Serialize, Deserialize)]
// struct VerificationResult {
//     txid: String,
//     verified: bool,
//     confirmations: i32,
//     block_hash: Option<String>,
//     block_height: Option<i32>,
//     merkle_verified: bool,
//     sufficient_confirmations: bool,
// }

// #[derive(Debug, Serialize, Deserialize)]
// struct ChainValidation {
//     from_height: i32,
//     to_height: i32,
//     valid: bool,
//     blocks_validated: i32,
//     errors: Vec<String>,
// }

// #[derive(Debug, Serialize, Deserialize)]
// struct ReorgDetection {
//     detected: bool,
//     reorg_depth: Option<i32>,
//     old_chain_tip: Option<String>,
//     new_chain_tip: String,
//     affected_blocks: Vec<i32>,
// }

// // WhatsOnChain API response types
// #[derive(Debug, Deserialize)]
// struct WocBlockHeader {
//     height: i32,
//     hash: String,
//     version: i32,
//     #[serde(rename = "previousblockhash")]
//     previous_block_hash: Option<String>,
//     merkleroot: String,
//     time: i64,
//     bits: String,
//     nonce: i64,
//     difficulty: f64,
// }

// #[derive(Debug, Deserialize)]
// struct WocMerkleProof {
//     #[serde(rename = "merkleRoot")]
//     merkle_root: String,
//     siblings: Vec<String>,
//     index: i32,
// }

// // ============================================================================
// // Application State
// // ============================================================================

// struct AppState {
//     db: PgPool,
//     config: Config,
//     client: reqwest::Client,
// }

// impl AppState {
//     async fn new(config: Config) -> Result<Self, sqlx::Error> {
//         let db = PgPool::connect(&config.database_url).await?;
//         let client = reqwest::Client::new();
//         Ok(Self { db, config, client })
//     }
// }

// // ============================================================================
// // WhatsOnChain API Client
// // ============================================================================

// impl AppState {
//     async fn woc_get_block_header(&self, height_or_hash: &str) -> Result<WocBlockHeader, String> {
//         let url = format!("{}/block/{}/header", self.config.woc_api_base, height_or_hash);
        
//         let response = self.client
//             .get(&url)
//             .send()
//             .await
//             .map_err(|e| format!("WoC API error: {}", e))?;
        
//         if !response.status().is_success() {
//             return Err(format!("WoC API returned status: {}", response.status()));
//         }
        
//         response
//             .json::<WocBlockHeader>()
//             .await
//             .map_err(|e| format!("Failed to parse WoC response: {}", e))
//     }
    
//     async fn woc_get_merkle_proof(&self, txid: &str) -> Result<WocMerkleProof, String> {
//         let url = format!("{}/tx/{}/proof", self.config.woc_api_base, txid);
        
//         let response = self.client
//             .get(&url)
//             .send()
//             .await
//             .map_err(|e| format!("WoC API error: {}", e))?;
        
//         if !response.status().is_success() {
//             return Err(format!("WoC API returned status: {}", response.status()));
//         }
        
//         response
//             .json::<WocMerkleProof>()
//             .await
//             .map_err(|e| format!("Failed to parse WoC response: {}", e))
//     }
    
//     async fn woc_get_chain_tip(&self) -> Result<i32, String> {
//         let url = format!("{}/chain/info", self.config.woc_api_base);
        
//         #[derive(Deserialize)]
//         struct ChainInfo {
//             blocks: i32,
//         }
        
//         let response = self.client
//             .get(&url)
//             .send()
//             .await
//             .map_err(|e| format!("WoC API error: {}", e))?;
        
//         let info: ChainInfo = response
//             .json()
//             .await
//             .map_err(|e| format!("Failed to parse response: {}", e))?;
        
//         Ok(info.blocks)
//     }
// }

// // ============================================================================
// // Database Operations
// // ============================================================================

// impl AppState {
//     async fn save_block_header(&self, header: &BlockHeader) -> Result<(), sqlx::Error> {
//         sqlx::query(
//             r#"
//             INSERT INTO block_headers 
//                 (height, hash, version, prev_block, merkle_root, 
//                  timestamp, bits, nonce, difficulty, created_at)
//             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW())
//             ON CONFLICT (height) DO UPDATE SET
//                 hash = $2,
//                 version = $3,
//                 prev_block = $4,
//                 merkle_root = $5,
//                 timestamp = $6,
//                 bits = $7,
//                 nonce = $8,
//                 difficulty = $9
//             "#
//         )
//         .bind(header.height)
//         .bind(&header.hash)
//         .bind(header.version)
//         .bind(&header.prev_block)
//         .bind(&header.merkle_root)
//         .bind(header.timestamp)
//         .bind(header.bits)
//         .bind(header.nonce)
//         .bind(header.difficulty)
//         .execute(&self.db)
//         .await?;
        
//         Ok(())
//     }
    
//     async fn get_block_header(&self, height: i32) -> Result<Option<BlockHeader>, sqlx::Error> {
//         let row = sqlx::query(
//             r#"
//             SELECT height, hash, version, prev_block, merkle_root,
//                    timestamp, bits, nonce, difficulty
//             FROM block_headers
//             WHERE height = $1
//             "#
//         )
//         .bind(height)
//         .fetch_optional(&self.db)
//         .await?;
        
//         if let Some(row) = row {
//             Ok(Some(BlockHeader {
//                 height: row.try_get("height")?,
//                 hash: row.try_get("hash")?,
//                 version: row.try_get("version")?,
//                 prev_block: row.try_get("prev_block")?,
//                 merkle_root: row.try_get("merkle_root")?,
//                 timestamp: row.try_get("timestamp")?,
//                 bits: row.try_get("bits")?,
//                 nonce: row.try_get("nonce")?,
//                 difficulty: row.try_get("difficulty")?,
//             }))
//         } else {
//             Ok(None)
//         }
//     }
    
//     async fn save_merkle_proof(&self, proof: &MerkleProof) -> Result<(), sqlx::Error> {
//         sqlx::query(
//             r#"
//             INSERT INTO merkle_proofs 
//                 (txid, block_hash, block_height, merkle_root, 
//                  siblings, tx_index, verified, verified_at, created_at)
//             VALUES ($1, $2, $3, $4, $5, $6, true, NOW(), NOW())
//             ON CONFLICT (txid) DO UPDATE SET
//                 block_hash = $2,
//                 block_height = $3,
//                 merkle_root = $4,
//                 siblings = $5,
//                 tx_index = $6,
//                 verified = true,
//                 verified_at = NOW()
//             "#
//         )
//         .bind(&proof.txid)
//         .bind(&proof.block_hash)
//         .bind(proof.block_height)
//         .bind(&proof.merkle_root)
//         .bind(serde_json::to_value(&proof.siblings).unwrap())
//         .bind(proof.tx_index)
//         .execute(&self.db)
//         .await?;
        
//         Ok(())
//     }
    
//     async fn get_merkle_proof(&self, txid: &str) -> Result<Option<MerkleProof>, sqlx::Error> {
//         let row = sqlx::query(
//             r#"
//             SELECT txid, block_hash, block_height, merkle_root,
//                    siblings, tx_index
//             FROM merkle_proofs
//             WHERE txid = $1 AND verified = true
//             "#
//         )
//         .bind(txid)
//         .fetch_optional(&self.db)
//         .await?;
        
//         if let Some(row) = row {
//             let siblings_json: serde_json::Value = row.try_get("siblings")?;
//             let siblings: Vec<String> = serde_json::from_value(siblings_json)
//                 .unwrap_or_default();
            
//             Ok(Some(MerkleProof {
//                 txid: row.try_get("txid")?,
//                 block_hash: row.try_get("block_hash")?,
//                 block_height: row.try_get("block_height")?,
//                 merkle_root: row.try_get("merkle_root")?,
//                 siblings,
//                 tx_index: row.try_get("tx_index")?,
//             }))
//         } else {
//             Ok(None)
//         }
//     }
// }

// // ============================================================================
// // SPV Verification Logic
// // ============================================================================

// fn double_sha256(data: &[u8]) -> Vec<u8> {
//     let hash1 = Sha256::digest(data);
//     let hash2 = Sha256::digest(&hash1);
//     hash2.to_vec()
// }

// fn verify_merkle_proof(
//     tx_hash: &str,
//     siblings: &[String],
//     index: i32,
//     merkle_root: &str,
// ) -> bool {
//     let mut hash = match hex::decode(tx_hash) {
//         Ok(h) => h,
//         Err(_) => return false,
//     };
    
//     let mut current_index = index;
    
//     for sibling in siblings {
//         let sibling_bytes = match hex::decode(sibling) {
//             Ok(b) => b,
//             Err(_) => return false,
//         };
        
//         let combined = if current_index % 2 == 0 {
//             // Hash is on the left
//             [hash.as_slice(), sibling_bytes.as_slice()].concat()
//         } else {
//             // Hash is on the right
//             [sibling_bytes.as_slice(), hash.as_slice()].concat()
//         };
        
//         hash = double_sha256(&combined);
//         current_index /= 2;
//     }
    
//     let computed_root = hex::encode(hash);
//     computed_root == merkle_root
// }

// fn verify_block_header_hash(header: &BlockHeader) -> bool {
//     // Serialize header
//     let mut serialized = Vec::new();
//     serialized.extend_from_slice(&header.version.to_le_bytes());
    
//     // Previous block hash (reversed)
//     if let Ok(prev_hash) = hex::decode(&header.prev_block) {
//         serialized.extend_from_slice(&prev_hash);
//     } else {
//         return false;
//     }
    
//     // Merkle root (reversed)
//     if let Ok(merkle) = hex::decode(&header.merkle_root) {
//         serialized.extend_from_slice(&merkle);
//     } else {
//         return false;
//     }
    
//     serialized.extend_from_slice(&(header.timestamp as u32).to_le_bytes());
//     serialized.extend_from_slice(&(header.bits as u32).to_le_bytes());
//     serialized.extend_from_slice(&(header.nonce as u32).to_le_bytes());
    
//     // Calculate hash
//     let hash = double_sha256(&serialized);
//     let computed_hash = hex::encode(hash);
    
//     computed_hash == header.hash
// }

// fn validate_header_chain(headers: &[BlockHeader]) -> Result<(), String> {
//     if headers.is_empty() {
//         return Ok(());
//     }
    
//     for i in 1..headers.len() {
//         let prev_header = &headers[i - 1];
//         let curr_header = &headers[i];
        
//         // Check height sequence
//         if curr_header.height != prev_header.height + 1 {
//             return Err(format!(
//                 "Height discontinuity at {}: expected {}, got {}",
//                 curr_header.height,
//                 prev_header.height + 1,
//                 curr_header.height
//             ));
//         }
        
//         // Check previous block hash
//         if curr_header.prev_block != prev_header.hash {
//             return Err(format!(
//                 "Previous block hash mismatch at height {}",
//                 curr_header.height
//             ));
//         }
        
//         // Verify block hash
//         if !verify_block_header_hash(curr_header) {
//             return Err(format!(
//                 "Invalid block hash at height {}",
//                 curr_header.height
//             ));
//         }
        
//         // Check timestamp progression
//         if curr_header.timestamp <= prev_header.timestamp {
//             return Err(format!(
//                 "Timestamp not increasing at height {}",
//                 curr_header.height
//             ));
//         }
//     }
    
//     Ok(())
// }

// // ============================================================================
// // HTTP Handlers
// // ============================================================================

// #[derive(Serialize)]
// struct HealthResponse {
//     status: String,
//     service: String,
//     network: String,
//     version: String,
// }

// async fn health_check(data: web::Data<AppState>) -> Result<HttpResponse> {
//     Ok(HttpResponse::Ok().json(HealthResponse {
//         status: "healthy".to_string(),
//         service: "spv-verification".to_string(),
//         network: data.config.network.clone(),
//         version: "0.1.0".to_string(),
//     }))
// }

// #[derive(Deserialize)]
// struct VerifyTxRequest {
//     txid: String,
// }

// async fn verify_transaction(
//     data: web::Data<AppState>,
//     req: web::Json<VerifyTxRequest>,
// ) -> Result<HttpResponse> {
//     // Get Merkle proof from WhatsOnChain
//     let woc_proof = match data.woc_get_merkle_proof(&req.txid).await {
//         Ok(p) => p,
//         Err(e) => {
//             return Ok(HttpResponse::Ok().json(VerificationResult {
//                 txid: req.txid.clone(),
//                 verified: false,
//                 confirmations: 0,
//                 block_hash: None,
//                 block_height: None,
//                 merkle_verified: false,
//                 sufficient_confirmations: false,
//             }));
//         }
//     };
    
//     // Verify Merkle proof
//     let merkle_verified = verify_merkle_proof(
//         &req.txid,
//         &woc_proof.siblings,
//         woc_proof.index,
//         &woc_proof.merkle_root,
//     );
    
//     // Get block height for confirmation count
//     let _chain_tip = data.woc_get_chain_tip().await.unwrap_or(0);
//     let confirmations = 0; // Would need block height from proof
    
//     // Save proof to database
//     if merkle_verified {
//         let proof = MerkleProof {
//             txid: req.txid.clone(),
//             block_hash: "".to_string(), // Would need from API
//             block_height: None,
//             merkle_root: woc_proof.merkle_root,
//             siblings: woc_proof.siblings,
//             tx_index: woc_proof.index,
//         };
//         let _ = data.save_merkle_proof(&proof).await;
//     }
    
//     Ok(HttpResponse::Ok().json(VerificationResult {
//         txid: req.txid.clone(),
//         verified: merkle_verified,
//         confirmations,
//         block_hash: None,
//         block_height: None,
//         merkle_verified,
//         sufficient_confirmations: confirmations >= data.config.min_confirmations as i32,
//     }))
// }

// #[derive(Deserialize)]
// struct GetProofRequest {
//     txid: String,
// }

// async fn get_merkle_proof_handler(
//     data: web::Data<AppState>,
//     req: web::Json<GetProofRequest>,
// ) -> Result<HttpResponse> {
//     // Check database first
//     if let Ok(Some(proof)) = data.get_merkle_proof(&req.txid).await {
//         return Ok(HttpResponse::Ok().json(proof));
//     }
    
//     // Fetch from WhatsOnChain
//     match data.woc_get_merkle_proof(&req.txid).await {
//         Ok(woc_proof) => {
//             let proof = MerkleProof {
//                 txid: req.txid.clone(),
//                 block_hash: "".to_string(),
//                 block_height: None,
//                 merkle_root: woc_proof.merkle_root,
//                 siblings: woc_proof.siblings,
//                 tx_index: woc_proof.index,
//             };
            
//             // Save to database
//             let _ = data.save_merkle_proof(&proof).await;
            
//             Ok(HttpResponse::Ok().json(proof))
//         }
//         Err(_e) => Ok(HttpResponse::NotFound().json(serde_json::json!({
//             "error": "Merkle proof not found"
//         })))
//     }
// }

// async fn get_block_header_handler(
//     data: web::Data<AppState>,
//     height: web::Path<i32>,
// ) -> Result<HttpResponse> {
//     // Check database first
//     if let Ok(Some(header)) = data.get_block_header(*height).await {
//         return Ok(HttpResponse::Ok().json(header));
//     }
    
//     // Fetch from WhatsOnChain
//     match data.woc_get_block_header(&height.to_string()).await {
//         Ok(woc_header) => {
//             let header = BlockHeader {
//                 height: woc_header.height,
//                 hash: woc_header.hash,
//                 version: woc_header.version,
//                 prev_block: woc_header.previous_block_hash.unwrap_or_default(),
//                 merkle_root: woc_header.merkleroot,
//                 timestamp: woc_header.time,
//                 bits: woc_header.bits.parse().unwrap_or(0),
//                 nonce: woc_header.nonce,
//                 difficulty: woc_header.difficulty,
//             };
            
//             // Save to database
//             let _ = data.save_block_header(&header).await;
            
//             Ok(HttpResponse::Ok().json(header))
//         }
//         Err(e) => Ok(HttpResponse::NotFound().json(serde_json::json!({
//             "error": "Block header not found",
//             "details": e
//         })))
//     }
// }

// #[derive(Deserialize)]
// struct ValidateChainQuery {
//     from: i32,
//     to: i32,
// }

// async fn validate_chain(
//     data: web::Data<AppState>,
//     query: web::Query<ValidateChainQuery>,
// ) -> Result<HttpResponse> {
//     let mut headers = Vec::new();
//     let mut errors = Vec::new();
    
//     // Fetch headers
//     for height in query.from..=query.to {
//         match data.woc_get_block_header(&height.to_string()).await {
//             Ok(woc_header) => {
//                 let header = BlockHeader {
//                     height: woc_header.height,
//                     hash: woc_header.hash,
//                     version: woc_header.version,
//                     prev_block: woc_header.previous_block_hash.unwrap_or_default(),
//                     merkle_root: woc_header.merkleroot,
//                     timestamp: woc_header.time,
//                     bits: woc_header.bits.parse().unwrap_or(0),
//                     nonce: woc_header.nonce,
//                     difficulty: woc_header.difficulty,
//                 };
//                 headers.push(header);
//             }
//             Err(e) => {
//                 errors.push(format!("Failed to fetch block {}: {}", height, e));
//             }
//         }
//     }
    
//     // Validate chain
//     let validation_result = validate_header_chain(&headers);
    
//     let valid = match validation_result {
//         Ok(_) => true,
//         Err(e) => {
//             errors.push(e);
//             false
//         }
//     };
    
//     Ok(HttpResponse::Ok().json(ChainValidation {
//         from_height: query.from,
//         to_height: query.to,
//         valid,
//         blocks_validated: headers.len() as i32,
//         errors,
//     }))
// }

// #[derive(Serialize)]
// struct ChainHeightResponse {
//     height: i32,
// }

// async fn get_chain_height(data: web::Data<AppState>) -> Result<HttpResponse> {
//     match data.woc_get_chain_tip().await {
//         Ok(height) => Ok(HttpResponse::Ok().json(ChainHeightResponse { height })),
//         Err(e) => Ok(HttpResponse::InternalServerError().json(serde_json::json!({
//             "error": "Failed to get chain height",
//             "details": e
//         })))
//     }
// }

// #[derive(Deserialize)]
// struct CheckReorgsQuery {
//     lookback: Option<i32>,
// }

// async fn check_reorgs(
//     data: web::Data<AppState>,
//     query: web::Query<CheckReorgsQuery>,
// ) -> Result<HttpResponse> {
//     let _lookback = query.lookback.unwrap_or(10);
    
//     // Get current chain tip
//     let chain_tip = match data.woc_get_chain_tip().await {
//         Ok(h) => h,
//         Err(e) => {
//             return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
//                 "error": "Failed to get chain tip",
//                 "details": e
//             })));
//         }
//     };
    
//     // Check for reorgs (simplified - would need more sophisticated logic)
//     let reorg = ReorgDetection {
//         detected: false,
//         reorg_depth: None,
//         old_chain_tip: None,
//         new_chain_tip: format!("{}", chain_tip),
//         affected_blocks: Vec::new(),
//     };
    
//     Ok(HttpResponse::Ok().json(reorg))
// }

// #[derive(Serialize)]
// struct DifficultyResponse {
//     current_difficulty: f64,
//     target: String,
//     block_height: i32,
// }

// async fn get_difficulty(data: web::Data<AppState>) -> Result<HttpResponse> {
//     let chain_tip = data.woc_get_chain_tip().await.unwrap_or(0);
    
//     match data.woc_get_block_header(&chain_tip.to_string()).await {
//         Ok(header) => Ok(HttpResponse::Ok().json(DifficultyResponse {
//             current_difficulty: header.difficulty,
//             target: header.bits,
//             block_height: header.height,
//         })),
//         Err(e) => Ok(HttpResponse::InternalServerError().json(serde_json::json!({
//             "error": "Failed to get difficulty",
//             "details": e
//         })))
//     }
// }

// // ============================================================================
// // Main Application
// // ============================================================================

// #[actix_web::main]
// async fn main() -> std::io::Result<()> {
//     env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    
//     let config = Config::from_env();
//     println!("🚀 Starting SPV Verification Service");
//     println!("   Network: {}", config.network);
//     println!("   Min confirmations: {}", config.min_confirmations);
    
//     let state = web::Data::new(
//         AppState::new(config.clone())
//             .await
//             .expect("Failed to initialize application state")
//     );
    
//     println!("✓ Server starting on http://127.0.0.1:8086");
    
//     HttpServer::new(move || {
//         let cors = Cors::default()
//             .allow_any_origin()
//             .allow_any_method()
//             .allow_any_header()
//             .max_age(3600);
        
//         App::new()
//             .wrap(cors)
//             // Phase 6: Add security headers
//             .wrap(middleware::DefaultHeaders::new()
//                 .add(("X-Frame-Options", "DENY"))
//                 .add(("X-Content-Type-Options", "nosniff"))
//                 .add(("Content-Security-Policy", "default-src 'self'"))
//             )
//             .app_data(state.clone())
//             .route("/health", web::get().to(health_check))
//             .route("/verify/tx", web::post().to(verify_transaction))
//             .route("/verify/merkle-proof", web::post().to(get_merkle_proof_handler))
//             .route("/chain/headers/{height}", web::get().to(get_block_header_handler))
//             .route("/chain/height", web::get().to(get_chain_height))
//             .route("/chain/validate", web::get().to(validate_chain))
//             .route("/chain/reorgs", web::get().to(check_reorgs))
//             .route("/chain/difficulty", web::get().to(get_difficulty))
//     })
//     .bind("127.0.0.1:8086")?
//     .run()
//     .await
// }

// core/spv-service/src/main.rs
// SPV Verification Service - Port 8086
// Phase 6 Production Hardening

use actix_web::{web, App, HttpResponse, HttpServer, Result, middleware};
use actix_cors::Cors;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use sha2::{Sha256, Digest};
use chrono::{DateTime, Utc};
use bsv_bank_common::{
    init_logging, ServiceMetrics,
    validate_txid,
};
use dotenv::dotenv;
use prometheus::Registry;
use std::time::SystemTime;
use thiserror::Error;

// ============================================================================
// ERROR TYPES (Phase 6)
// ============================================================================

#[derive(Debug, Error)]
enum ServiceError {
    #[error("Invalid input: {0}")]
    ValidationError(String),
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("WhatsOnChain API error: {0}")]
    ApiError(String),
    #[error("Verification failed: {0}")]
    VerificationError(String),
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
            ServiceError::ApiError(msg) => {
                HttpResponse::BadGateway().json(serde_json::json!({
                    "error": "api_error",
                    "message": msg
                }))
            }
            ServiceError::VerificationError(msg) => {
                HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "verification_error",
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
    woc_api_base: String,
    network: String,
    min_confirmations: u32,
}

impl Config {
    fn from_env() -> Self {
        Self {
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgresql://a:a@localhost/bsv_bank".to_string()),
            woc_api_base: std::env::var("WOC_API_BASE")
                .unwrap_or_else(|_| "https://api.whatsonchain.com/v1/bsv/test".to_string()),
            network: std::env::var("NETWORK")
                .unwrap_or_else(|_| "testnet".to_string()),
            min_confirmations: std::env::var("MIN_CONFIRMATIONS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(1),
        }
    }
}

// ============================================================================
// Data Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BlockHeader {
    height: i32,
    hash: String,
    version: i32,
    prev_block: String,
    merkle_root: String,
    timestamp: i64,
    bits: i32,
    nonce: i64,
    difficulty: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MerkleProof {
    txid: String,
    block_hash: String,
    block_height: Option<i32>,
    merkle_root: String,
    siblings: Vec<String>,
    tx_index: i32,
}

#[derive(Debug, Serialize, Deserialize)]
struct VerificationResult {
    txid: String,
    verified: bool,
    confirmations: i32,
    block_hash: Option<String>,
    block_height: Option<i32>,
    merkle_verified: bool,
    sufficient_confirmations: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChainValidation {
    from_height: i32,
    to_height: i32,
    valid: bool,
    blocks_validated: i32,
    errors: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ReorgDetection {
    detected: bool,
    reorg_depth: Option<i32>,
    old_chain_tip: Option<String>,
    new_chain_tip: String,
    affected_blocks: Vec<i32>,
}

// WhatsOnChain API response types
#[derive(Debug, Deserialize)]
struct WocBlockHeader {
    height: i32,
    hash: String,
    version: i32,
    #[serde(rename = "previousblockhash")]
    previous_block_hash: Option<String>,
    merkleroot: String,
    time: i64,
    bits: String,
    nonce: i64,
    difficulty: f64,
}

#[derive(Debug, Deserialize)]
struct WocMerkleProof {
    #[serde(rename = "merkleRoot")]
    merkle_root: String,
    siblings: Vec<String>,
    index: i32,
}

// ============================================================================
// Application State
// ============================================================================

struct AppState {
    db: PgPool,
    config: Config,
    client: reqwest::Client,
    start_time: SystemTime,
}

impl AppState {
    async fn new(config: Config) -> Result<Self, sqlx::Error> {
        let db = PgPool::connect(&config.database_url).await?;
        let client = reqwest::Client::new();
        Ok(Self {
            db,
            config,
            client,
            start_time: SystemTime::now(),
        })
    }
}

// ============================================================================
// WhatsOnChain API Client
// ============================================================================

impl AppState {
    async fn woc_get_block_header(&self, height_or_hash: &str) -> Result<WocBlockHeader, ServiceError> {
        let url = format!("{}/block/{}/header", self.config.woc_api_base, height_or_hash);
        
        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| ServiceError::ApiError(e.to_string()))?;
        
        if !response.status().is_success() {
            return Err(ServiceError::ApiError(format!("Status: {}", response.status())));
        }
        
        response
            .json::<WocBlockHeader>()
            .await
            .map_err(|e| ServiceError::ApiError(e.to_string()))
    }
    
    async fn woc_get_merkle_proof(&self, txid: &str) -> Result<WocMerkleProof, ServiceError> {
        let url = format!("{}/tx/{}/proof", self.config.woc_api_base, txid);
        
        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| ServiceError::ApiError(e.to_string()))?;
        
        if !response.status().is_success() {
            return Err(ServiceError::ApiError(format!("Status: {}", response.status())));
        }
        
        response
            .json::<WocMerkleProof>()
            .await
            .map_err(|e| ServiceError::ApiError(e.to_string()))
    }
    
    async fn woc_get_chain_tip(&self) -> Result<i32, ServiceError> {
        let url = format!("{}/chain/info", self.config.woc_api_base);
        
        #[derive(Deserialize)]
        struct ChainInfo {
            blocks: i32,
        }
        
        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| ServiceError::ApiError(e.to_string()))?;
        
        let info: ChainInfo = response
            .json()
            .await
            .map_err(|e| ServiceError::ApiError(e.to_string()))?;
        
        Ok(info.blocks)
    }
}

// ============================================================================
// Database Operations
// ============================================================================

impl AppState {
    async fn save_block_header(&self, header: &BlockHeader) -> Result<(), ServiceError> {
        sqlx::query(
            r#"
            INSERT INTO block_headers 
                (height, hash, version, prev_block, merkle_root, 
                 timestamp, bits, nonce, difficulty, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW())
            ON CONFLICT (height) DO UPDATE SET
                hash = $2, version = $3, prev_block = $4, merkle_root = $5,
                timestamp = $6, bits = $7, nonce = $8, difficulty = $9
            "#
        )
        .bind(header.height)
        .bind(&header.hash)
        .bind(header.version)
        .bind(&header.prev_block)
        .bind(&header.merkle_root)
        .bind(header.timestamp)
        .bind(header.bits)
        .bind(header.nonce)
        .bind(header.difficulty)
        .execute(&self.db)
        .await
        .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;
        
        Ok(())
    }
    
    async fn get_block_header(&self, height: i32) -> Result<Option<BlockHeader>, ServiceError> {
        let row = sqlx::query(
            r#"
            SELECT height, hash, version, prev_block, merkle_root,
                   timestamp, bits, nonce, difficulty
            FROM block_headers WHERE height = $1
            "#
        )
        .bind(height)
        .fetch_optional(&self.db)
        .await
        .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;
        
        if let Some(row) = row {
            Ok(Some(BlockHeader {
                height: row.try_get("height").map_err(|e| ServiceError::DatabaseError(e.to_string()))?,
                hash: row.try_get("hash").map_err(|e| ServiceError::DatabaseError(e.to_string()))?,
                version: row.try_get("version").map_err(|e| ServiceError::DatabaseError(e.to_string()))?,
                prev_block: row.try_get("prev_block").map_err(|e| ServiceError::DatabaseError(e.to_string()))?,
                merkle_root: row.try_get("merkle_root").map_err(|e| ServiceError::DatabaseError(e.to_string()))?,
                timestamp: row.try_get("timestamp").map_err(|e| ServiceError::DatabaseError(e.to_string()))?,
                bits: row.try_get("bits").map_err(|e| ServiceError::DatabaseError(e.to_string()))?,
                nonce: row.try_get("nonce").map_err(|e| ServiceError::DatabaseError(e.to_string()))?,
                difficulty: row.try_get("difficulty").map_err(|e| ServiceError::DatabaseError(e.to_string()))?,
            }))
        } else {
            Ok(None)
        }
    }
    
    async fn save_merkle_proof(&self, proof: &MerkleProof) -> Result<(), ServiceError> {
        sqlx::query(
            r#"
            INSERT INTO merkle_proofs 
                (txid, block_hash, block_height, merkle_root, siblings, tx_index, 
                 verified, verified_at, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, true, NOW(), NOW())
            ON CONFLICT (txid) DO UPDATE SET
                block_hash = $2, block_height = $3, merkle_root = $4,
                siblings = $5, tx_index = $6, verified = true, verified_at = NOW()
            "#
        )
        .bind(&proof.txid)
        .bind(&proof.block_hash)
        .bind(proof.block_height)
        .bind(&proof.merkle_root)
        .bind(serde_json::to_value(&proof.siblings).unwrap())
        .bind(proof.tx_index)
        .execute(&self.db)
        .await
        .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;
        
        Ok(())
    }
    
    async fn get_merkle_proof(&self, txid: &str) -> Result<Option<MerkleProof>, ServiceError> {
        let row = sqlx::query(
            r#"
            SELECT txid, block_hash, block_height, merkle_root, siblings, tx_index
            FROM merkle_proofs WHERE txid = $1 AND verified = true
            "#
        )
        .bind(txid)
        .fetch_optional(&self.db)
        .await
        .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;
        
        if let Some(row) = row {
            let siblings_json: serde_json::Value = row.try_get("siblings")
                .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;
            let siblings: Vec<String> = serde_json::from_value(siblings_json).unwrap_or_default();
            
            Ok(Some(MerkleProof {
                txid: row.try_get("txid").map_err(|e| ServiceError::DatabaseError(e.to_string()))?,
                block_hash: row.try_get("block_hash").map_err(|e| ServiceError::DatabaseError(e.to_string()))?,
                block_height: row.try_get("block_height").map_err(|e| ServiceError::DatabaseError(e.to_string()))?,
                merkle_root: row.try_get("merkle_root").map_err(|e| ServiceError::DatabaseError(e.to_string()))?,
                siblings,
                tx_index: row.try_get("tx_index").map_err(|e| ServiceError::DatabaseError(e.to_string()))?,
            }))
        } else {
            Ok(None)
        }
    }
}

// ============================================================================
// SPV Verification Logic
// ============================================================================

fn double_sha256(data: &[u8]) -> Vec<u8> {
    let hash1 = Sha256::digest(data);
    let hash2 = Sha256::digest(&hash1);
    hash2.to_vec()
}

fn verify_merkle_proof(tx_hash: &str, siblings: &[String], index: i32, merkle_root: &str) -> bool {
    let mut hash = match hex::decode(tx_hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    
    let mut current_index = index;
    
    for sibling in siblings {
        let sibling_bytes = match hex::decode(sibling) {
            Ok(b) => b,
            Err(_) => return false,
        };
        
        let combined = if current_index % 2 == 0 {
            [hash.as_slice(), sibling_bytes.as_slice()].concat()
        } else {
            [sibling_bytes.as_slice(), hash.as_slice()].concat()
        };
        
        hash = double_sha256(&combined);
        current_index /= 2;
    }
    
    let computed_root = hex::encode(hash);
    computed_root == merkle_root
}

fn verify_block_header_hash(header: &BlockHeader) -> bool {
    let mut serialized = Vec::new();
    serialized.extend_from_slice(&header.version.to_le_bytes());
    
    if let Ok(prev_hash) = hex::decode(&header.prev_block) {
        serialized.extend_from_slice(&prev_hash);
    } else {
        return false;
    }
    
    if let Ok(merkle) = hex::decode(&header.merkle_root) {
        serialized.extend_from_slice(&merkle);
    } else {
        return false;
    }
    
    serialized.extend_from_slice(&(header.timestamp as u32).to_le_bytes());
    serialized.extend_from_slice(&(header.bits as u32).to_le_bytes());
    serialized.extend_from_slice(&(header.nonce as u32).to_le_bytes());
    
    let hash = double_sha256(&serialized);
    let computed_hash = hex::encode(hash);
    
    computed_hash == header.hash
}

fn validate_header_chain(headers: &[BlockHeader]) -> Result<(), ServiceError> {
    if headers.is_empty() {
        return Ok(());
    }
    
    for i in 1..headers.len() {
        let prev_header = &headers[i - 1];
        let curr_header = &headers[i];
        
        if curr_header.height != prev_header.height + 1 {
            return Err(ServiceError::VerificationError(format!(
                "Height discontinuity at {}", curr_header.height
            )));
        }
        
        if curr_header.prev_block != prev_header.hash {
            return Err(ServiceError::VerificationError(format!(
                "Previous block hash mismatch at height {}", curr_header.height
            )));
        }
        
        if !verify_block_header_hash(curr_header) {
            return Err(ServiceError::VerificationError(format!(
                "Invalid block hash at height {}", curr_header.height
            )));
        }
        
        if curr_header.timestamp <= prev_header.timestamp {
            return Err(ServiceError::VerificationError(format!(
                "Timestamp not increasing at height {}", curr_header.height
            )));
        }
    }
    
    Ok(())
}

// ============================================================================
// HTTP Handlers (Phase 6 Enhanced)
// ============================================================================

async fn health_check(data: web::Data<AppState>) -> Result<HttpResponse, ServiceError> {
    let uptime = SystemTime::now()
        .duration_since(data.start_time)
        .unwrap()
        .as_secs();
    
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "service": "spv-verification",
        "status": "healthy",
        "network": data.config.network,
        "version": "0.1.0",
        "uptime_seconds": uptime,
        "features": ["merkle-verification", "chain-validation", "phase6-hardening"]
    })))
}

async fn liveness_check() -> Result<HttpResponse, ServiceError> {
    Ok(HttpResponse::Ok().json(serde_json::json!({"status": "alive"})))
}

async fn readiness_check(data: web::Data<AppState>) -> Result<HttpResponse, ServiceError> {
    let db_ok = sqlx::query("SELECT 1").fetch_optional(&data.db).await.is_ok();
    let woc_ok = data.woc_get_chain_tip().await.is_ok();
    
    if db_ok && woc_ok {
        Ok(HttpResponse::Ok().json(serde_json::json!({
            "status": "ready",
            "checks": {"database": "ok", "whatsonchain_api": "ok"}
        })))
    } else {
        Ok(HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "status": "not_ready",
            "checks": {
                "database": if db_ok { "ok" } else { "error" },
                "whatsonchain_api": if woc_ok { "ok" } else { "error" }
            }
        })))
    }
}

#[derive(Deserialize)]
struct VerifyTxRequest {
    txid: String,
}

async fn verify_transaction(
    data: web::Data<AppState>,
    req: web::Json<VerifyTxRequest>,
) -> Result<HttpResponse, ServiceError> {
    // Phase 6: Validate txid
    validate_txid(&req.txid)
        .map_err(|e| ServiceError::ValidationError(e.to_string()))?;
    
    let woc_proof = data.woc_get_merkle_proof(&req.txid).await?;
    
    let merkle_verified = verify_merkle_proof(
        &req.txid,
        &woc_proof.siblings,
        woc_proof.index,
        &woc_proof.merkle_root,
    );
    
    let confirmations = 0; // Would need block height from proof
    
    if merkle_verified {
        let proof = MerkleProof {
            txid: req.txid.clone(),
            block_hash: String::new(),
            block_height: None,
            merkle_root: woc_proof.merkle_root,
            siblings: woc_proof.siblings,
            tx_index: woc_proof.index,
        };
        let _ = data.save_merkle_proof(&proof).await;
    }
    
    Ok(HttpResponse::Ok().json(VerificationResult {
        txid: req.txid.clone(),
        verified: merkle_verified,
        confirmations,
        block_hash: None,
        block_height: None,
        merkle_verified,
        sufficient_confirmations: confirmations >= data.config.min_confirmations as i32,
    }))
}

#[derive(Deserialize)]
struct GetProofRequest {
    txid: String,
}

async fn get_merkle_proof_handler(
    data: web::Data<AppState>,
    req: web::Json<GetProofRequest>,
) -> Result<HttpResponse, ServiceError> {
    // Phase 6: Validate txid
    validate_txid(&req.txid)
        .map_err(|e| ServiceError::ValidationError(e.to_string()))?;
    
    if let Some(proof) = data.get_merkle_proof(&req.txid).await? {
        return Ok(HttpResponse::Ok().json(proof));
    }
    
    let woc_proof = data.woc_get_merkle_proof(&req.txid).await?;
    
    let proof = MerkleProof {
        txid: req.txid.clone(),
        block_hash: String::new(),
        block_height: None,
        merkle_root: woc_proof.merkle_root,
        siblings: woc_proof.siblings,
        tx_index: woc_proof.index,
    };
    
    let _ = data.save_merkle_proof(&proof).await;
    
    Ok(HttpResponse::Ok().json(proof))
}

async fn get_block_header_handler(
    data: web::Data<AppState>,
    height: web::Path<i32>,
) -> Result<HttpResponse, ServiceError> {
    if let Some(header) = data.get_block_header(*height).await? {
        return Ok(HttpResponse::Ok().json(header));
    }
    
    let woc_header = data.woc_get_block_header(&height.to_string()).await?;
    
    let header = BlockHeader {
        height: woc_header.height,
        hash: woc_header.hash,
        version: woc_header.version,
        prev_block: woc_header.previous_block_hash.unwrap_or_default(),
        merkle_root: woc_header.merkleroot,
        timestamp: woc_header.time,
        bits: woc_header.bits.parse().unwrap_or(0),
        nonce: woc_header.nonce,
        difficulty: woc_header.difficulty,
    };
    
    let _ = data.save_block_header(&header).await;
    
    Ok(HttpResponse::Ok().json(header))
}

#[derive(Deserialize)]
struct ValidateChainQuery {
    from: i32,
    to: i32,
}

async fn validate_chain(
    data: web::Data<AppState>,
    query: web::Query<ValidateChainQuery>,
) -> Result<HttpResponse, ServiceError> {
    let mut headers = Vec::new();
    let mut errors = Vec::new();
    
    for height in query.from..=query.to {
        match data.woc_get_block_header(&height.to_string()).await {
            Ok(woc_header) => {
                let header = BlockHeader {
                    height: woc_header.height,
                    hash: woc_header.hash,
                    version: woc_header.version,
                    prev_block: woc_header.previous_block_hash.unwrap_or_default(),
                    merkle_root: woc_header.merkleroot,
                    timestamp: woc_header.time,
                    bits: woc_header.bits.parse().unwrap_or(0),
                    nonce: woc_header.nonce,
                    difficulty: woc_header.difficulty,
                };
                headers.push(header);
            }
            Err(e) => {
                errors.push(format!("Failed to fetch block {}: {}", height, e));
            }
        }
    }
    
    let valid = match validate_header_chain(&headers) {
        Ok(_) => true,
        Err(e) => {
            errors.push(e.to_string());
            false
        }
    };
    
    Ok(HttpResponse::Ok().json(ChainValidation {
        from_height: query.from,
        to_height: query.to,
        valid,
        blocks_validated: headers.len() as i32,
        errors,
    }))
}

#[derive(Serialize)]
struct ChainHeightResponse {
    height: i32,
}

async fn get_chain_height(data: web::Data<AppState>) -> Result<HttpResponse, ServiceError> {
    let height = data.woc_get_chain_tip().await?;
    Ok(HttpResponse::Ok().json(ChainHeightResponse { height }))
}

#[derive(Deserialize)]
struct CheckReorgsQuery {
    lookback: Option<i32>,
}

async fn check_reorgs(
    data: web::Data<AppState>,
    _query: web::Query<CheckReorgsQuery>,
) -> Result<HttpResponse, ServiceError> {
    let chain_tip = data.woc_get_chain_tip().await?;
    
    let reorg = ReorgDetection {
        detected: false,
        reorg_depth: None,
        old_chain_tip: None,
        new_chain_tip: format!("{}", chain_tip),
        affected_blocks: Vec::new(),
    };
    
    Ok(HttpResponse::Ok().json(reorg))
}

#[derive(Serialize)]
struct DifficultyResponse {
    current_difficulty: f64,
    target: String,
    block_height: i32,
}

async fn get_difficulty(data: web::Data<AppState>) -> Result<HttpResponse, ServiceError> {
    let chain_tip = data.woc_get_chain_tip().await?;
    let header = data.woc_get_block_header(&chain_tip.to_string()).await?;
    
    Ok(HttpResponse::Ok().json(DifficultyResponse {
        current_difficulty: header.difficulty,
        target: header.bits,
        block_height: header.height,
    }))
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
// Main Application (Phase 6 Enhanced)
// ============================================================================

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    
    println!("🚀 BSV Bank - SPV Verification Service Starting (Phase 6)...");
    
    let config = Config::from_env();
    println!("   Network: {}", config.network);
    println!("   Min confirmations: {}", config.min_confirmations);
    
    // Phase 6: Initialize structured logging
    init_logging("spv-service");
    tracing::info!("Starting SPV Verification Service on port 8086");
    
    let state = web::Data::new(
        AppState::new(config.clone())
            .await
            .expect("Failed to initialize application state")
    );
    
    tracing::info!("Database connection established");
    
    // Phase 6: Prometheus metrics
    let registry = Registry::new();
    let _service_metrics = ServiceMetrics::new(&registry, "spv_service")
        .expect("Failed to create service metrics");
    tracing::info!("Metrics initialized");
    
    let registry_data = web::Data::new(registry);
    
    println!("✅ Service ready on http://127.0.0.1:8086");
    println!("📋 Health: http://127.0.0.1:8086/health");
    println!("📊 Metrics: http://127.0.0.1:8086/metrics");
    tracing::info!("Starting HTTP server...");
    
    HttpServer::new(move || {
        // Phase 6: CORS configuration
        let cors = Cors::default()
            .allowed_origin("http://localhost:3000")
            .allowed_origin("http://localhost:5173")
            .allowed_methods(vec!["GET", "POST"])
            .allowed_headers(vec![actix_web::http::header::CONTENT_TYPE])
            .max_age(3600);
        
        App::new()
            .wrap(cors)
            // Phase 6: Request logging
            .wrap(middleware::Logger::default())
            // Phase 6: Security headers
            .wrap(actix_web::middleware::DefaultHeaders::new()
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
            
            // SPV verification endpoints
            .route("/verify/tx", web::post().to(verify_transaction))
            .route("/verify/merkle-proof", web::post().to(get_merkle_proof_handler))
            
            // Chain endpoints
            .route("/chain/headers/{height}", web::get().to(get_block_header_handler))
            .route("/chain/height", web::get().to(get_chain_height))
            .route("/chain/validate", web::get().to(validate_chain))
            .route("/chain/reorgs", web::get().to(check_reorgs))
            .route("/chain/difficulty", web::get().to(get_difficulty))
    })
    .bind("127.0.0.1:8086")?
    .run()
    .await
}