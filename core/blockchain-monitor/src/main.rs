// core/blockchain-monitor/src/main.rs
// Blockchain Monitor Service - Port 8084
// Monitors BSV testnet via WhatsOnChain API
// Phase 6 Production Hardening

use actix_web::{web, App, HttpResponse, HttpServer, Result, middleware};
use actix_cors::Cors;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::{HashMap, HashSet};
use chrono::{DateTime, Utc};
use bsv_bank_common::{
    init_logging, ServiceMetrics,
    validate_txid, validate_address,
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
    #[error("Transaction not found: {0}")]
    NotFoundError(String),
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
            ServiceError::NotFoundError(msg) => {
                HttpResponse::NotFound().json(serde_json::json!({
                    "error": "not_found",
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
    polling_interval_secs: u64,
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
            polling_interval_secs: std::env::var("POLLING_INTERVAL")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(10),
        }
    }
}

// ============================================================================
// Data Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Transaction {
    txid: String,
    tx_type: Option<String>,
    from_address: Option<String>,
    to_address: Option<String>,
    amount_satoshis: i64,
    fee_satoshis: Option<i64>,
    confirmations: i32,
    status: String,
    block_hash: Option<String>,
    block_height: Option<i32>,
    block_time: Option<DateTime<Utc>>,
    raw_tx: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
struct WatchedAddress {
    address: String,
    paymail: String,
    purpose: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChainInfo {
    height: i32,
    best_block_hash: String,
    difficulty: f64,
    chain: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Utxo {
    txid: String,
    vout: u32,
    value: i64,
    height: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ConfirmationUpdate {
    txid: String,
    old_confirmations: i32,
    new_confirmations: i32,
    block_height: Option<i32>,
}

// WhatsOnChain API response types
#[derive(Debug, Deserialize)]
struct WocTransaction {
    #[allow(dead_code)]
    txid: String,
    confirmations: Option<i32>,
    blockhash: Option<String>,
    blockheight: Option<i32>,
    blocktime: Option<i64>,
    #[serde(rename = "vin")]
    inputs: Option<Vec<WocInput>>,
    #[serde(rename = "vout")]
    outputs: Option<Vec<WocOutput>>,
    hex: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WocInput {
    #[allow(dead_code)]
    txid: Option<String>,
    #[allow(dead_code)]
    vout: Option<u32>,
    #[serde(rename = "scriptSig")]
    script_sig: Option<WocScript>,
}

#[derive(Debug, Deserialize)]
struct WocOutput {
    value: Option<f64>,
    #[allow(dead_code)]
    n: Option<u32>,
    #[serde(rename = "scriptPubKey")]
    script_pub_key: Option<WocScript>,
}

#[derive(Debug, Deserialize)]
struct WocScript {
    addresses: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct WocChainInfo {
    blocks: i32,
    bestblockhash: String,
    difficulty: f64,
    chain: String,
}

#[derive(Debug, Deserialize)]
struct WocUtxo {
    tx_hash: String,
    tx_pos: u32,
    value: i64,
    height: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct WocBalance {
    confirmed: i64,
    unconfirmed: i64,
}

// ============================================================================
// Application State
// ============================================================================

struct AppState {
    db: PgPool,
    config: Config,
    client: reqwest::Client,
    watched_addresses: Arc<RwLock<HashSet<String>>>,
    tx_cache: Arc<RwLock<HashMap<String, Transaction>>>,
    start_time: SystemTime,
}

impl AppState {
    async fn new(config: Config) -> Result<Self, sqlx::Error> {
        let db = PgPool::connect(&config.database_url).await?;
        let client = reqwest::Client::new();
        
        let state = Self {
            db,
            config,
            client,
            watched_addresses: Arc::new(RwLock::new(HashSet::new())),
            tx_cache: Arc::new(RwLock::new(HashMap::new())),
            start_time: SystemTime::now(),
        };
        
        // Load watched addresses from database
        state.load_watched_addresses().await?;
        
        Ok(state)
    }
    
    async fn load_watched_addresses(&self) -> Result<(), sqlx::Error> {
        let rows = sqlx::query("SELECT address FROM watched_addresses")
            .fetch_all(&self.db)
            .await?;
        
        let mut addresses = self.watched_addresses.write().await;
        for row in rows {
            let address: String = row.try_get("address")?;
            addresses.insert(address);
        }
        
        Ok(())
    }
}

// ============================================================================
// WhatsOnChain API Client
// ============================================================================

impl AppState {
    async fn woc_get_transaction(&self, txid: &str) -> Result<WocTransaction, ServiceError> {
        let url = format!("{}/tx/{}", self.config.woc_api_base, txid);
        
        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| ServiceError::ApiError(format!("Request failed: {}", e)))?;
        
        if !response.status().is_success() {
            return Err(ServiceError::ApiError(format!("Status: {}", response.status())));
        }
        
        response
            .json::<WocTransaction>()
            .await
            .map_err(|e| ServiceError::ApiError(format!("Parse error: {}", e)))
    }
    
    async fn woc_get_chain_info(&self) -> Result<WocChainInfo, ServiceError> {
        let url = format!("{}/chain/info", self.config.woc_api_base);
        
        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| ServiceError::ApiError(e.to_string()))?;
        
        response
            .json::<WocChainInfo>()
            .await
            .map_err(|e| ServiceError::ApiError(e.to_string()))
    }
    
    async fn woc_get_address_utxos(&self, address: &str) -> Result<Vec<WocUtxo>, ServiceError> {
        let url = format!("{}/address/{}/unspent", self.config.woc_api_base, address);
        
        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| ServiceError::ApiError(e.to_string()))?;
        
        if !response.status().is_success() {
            return Ok(vec![]); // Address has no UTXOs
        }
        
        response
            .json::<Vec<WocUtxo>>()
            .await
            .map_err(|e| ServiceError::ApiError(e.to_string()))
    }
    
    async fn woc_get_address_balance(&self, address: &str) -> Result<WocBalance, ServiceError> {
        let url = format!("{}/address/{}/balance", self.config.woc_api_base, address);
        
        tracing::debug!("Fetching balance from: {}", url);
        
        let response = self.client
            .get(&url)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| ServiceError::ApiError(e.to_string()))?;
        
        let status = response.status();
        
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            tracing::error!("WoC API error for {}: status={}, body={}", address, status, error_text);
            return Err(ServiceError::ApiError(format!("Status: {} - {}", status, error_text)));
        }
        
        let response_text = response
            .text()
            .await
            .map_err(|e| ServiceError::ApiError(e.to_string()))?;
        
        if response_text.trim().is_empty() {
            tracing::warn!("Empty response from WoC for address: {}", address);
            return Ok(WocBalance {
                confirmed: 0,
                unconfirmed: 0,
            });
        }
        
        serde_json::from_str::<WocBalance>(&response_text)
            .map_err(|e| ServiceError::ApiError(format!("Parse error: {}", e)))
    }
    
    async fn woc_broadcast_transaction(&self, tx_hex: &str) -> Result<String, ServiceError> {
        let url = format!("{}/tx/raw", self.config.woc_api_base);
        
        #[derive(Serialize)]
        struct BroadcastRequest {
            txhex: String,
        }
        
        let response = self.client
            .post(&url)
            .json(&BroadcastRequest {
                txhex: tx_hex.to_string(),
            })
            .send()
            .await
            .map_err(|e| ServiceError::ApiError(e.to_string()))?;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ServiceError::ApiError(format!("Broadcast failed: {}", error_text)));
        }
        
        response
            .text()
            .await
            .map_err(|e| ServiceError::ApiError(e.to_string()))
    }
}

// ============================================================================
// Database Operations
// ============================================================================

impl AppState {
    async fn save_transaction(&self, tx: &Transaction) -> Result<(), ServiceError> {
        sqlx::query(
            r#"
            INSERT INTO blockchain_transactions 
                (txid, tx_type, from_address, to_address, amount_satoshis, 
                 fee_satoshis, confirmations, status, block_hash, block_height, 
                 block_time, raw_tx, first_seen)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, NOW())
            ON CONFLICT (txid) DO UPDATE SET
                confirmations = $7,
                status = $8,
                block_hash = $9,
                block_height = $10,
                block_time = $11,
                confirmed_at = CASE WHEN $7 > 0 AND blockchain_transactions.confirmed_at IS NULL 
                                    THEN NOW() 
                                    ELSE blockchain_transactions.confirmed_at 
                               END
            "#
        )
        .bind(&tx.txid)
        .bind(&tx.tx_type)
        .bind(&tx.from_address)
        .bind(&tx.to_address)
        .bind(tx.amount_satoshis)
        .bind(tx.fee_satoshis)
        .bind(tx.confirmations)
        .bind(&tx.status)
        .bind(&tx.block_hash)
        .bind(tx.block_height)
        .bind(tx.block_time)
        .bind(&tx.raw_tx)
        .execute(&self.db)
        .await
        .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;
        
        Ok(())
    }
    
    async fn get_transaction(&self, txid: &str) -> Result<Option<Transaction>, ServiceError> {
        let row = sqlx::query(
            r#"
            SELECT txid, tx_type, from_address, to_address, amount_satoshis,
                   fee_satoshis, confirmations, status, block_hash, block_height,
                   block_time, raw_tx
            FROM blockchain_transactions
            WHERE txid = $1
            "#
        )
        .bind(txid)
        .fetch_optional(&self.db)
        .await
        .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;
        
        if let Some(row) = row {
            Ok(Some(Transaction {
                txid: row.try_get("txid").map_err(|e| ServiceError::DatabaseError(e.to_string()))?,
                tx_type: row.try_get("tx_type").map_err(|e| ServiceError::DatabaseError(e.to_string()))?,
                from_address: row.try_get("from_address").map_err(|e| ServiceError::DatabaseError(e.to_string()))?,
                to_address: row.try_get("to_address").map_err(|e| ServiceError::DatabaseError(e.to_string()))?,
                amount_satoshis: row.try_get("amount_satoshis").map_err(|e| ServiceError::DatabaseError(e.to_string()))?,
                fee_satoshis: row.try_get("fee_satoshis").map_err(|e| ServiceError::DatabaseError(e.to_string()))?,
                confirmations: row.try_get("confirmations").map_err(|e| ServiceError::DatabaseError(e.to_string()))?,
                status: row.try_get("status").map_err(|e| ServiceError::DatabaseError(e.to_string()))?,
                block_hash: row.try_get("block_hash").map_err(|e| ServiceError::DatabaseError(e.to_string()))?,
                block_height: row.try_get("block_height").map_err(|e| ServiceError::DatabaseError(e.to_string()))?,
                block_time: row.try_get("block_time").map_err(|e| ServiceError::DatabaseError(e.to_string()))?,
                raw_tx: row.try_get("raw_tx").map_err(|e| ServiceError::DatabaseError(e.to_string()))?,
            }))
        } else {
            Ok(None)
        }
    }
    
    async fn save_confirmation_event(&self, update: &ConfirmationUpdate) -> Result<(), ServiceError> {
        sqlx::query(
            r#"
            INSERT INTO confirmation_events 
                (txid, old_confirmations, new_confirmations, block_height, detected_at)
            VALUES ($1, $2, $3, $4, NOW())
            "#
        )
        .bind(&update.txid)
        .bind(update.old_confirmations)
        .bind(update.new_confirmations)
        .bind(update.block_height)
        .execute(&self.db)
        .await
        .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;
        
        Ok(())
    }
    
    async fn add_watched_address(&self, address: &str, paymail: &str, purpose: &str) -> Result<(), ServiceError> {
        sqlx::query(
            r#"
            INSERT INTO watched_addresses (address, paymail, purpose, created_at)
            VALUES ($1, $2, $3, NOW())
            ON CONFLICT (address) DO NOTHING
            "#
        )
        .bind(address)
        .bind(paymail)
        .bind(purpose)
        .execute(&self.db)
        .await
        .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;
        
        // Add to in-memory set
        let mut addresses = self.watched_addresses.write().await;
        addresses.insert(address.to_string());
        
        Ok(())
    }
    
    async fn get_pending_transactions(&self) -> Result<Vec<String>, ServiceError> {
        let rows = sqlx::query(
            r#"
            SELECT txid FROM blockchain_transactions
            WHERE status = 'pending' OR confirmations < 6
            ORDER BY first_seen DESC
            LIMIT 100
            "#
        )
        .fetch_all(&self.db)
        .await
        .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;
        
        Ok(rows.into_iter().map(|row| row.get("txid")).collect())
    }
}

// ============================================================================
// Background Monitoring Task
// ============================================================================

async fn start_monitoring_task(state: web::Data<AppState>) {
    let interval = tokio::time::Duration::from_secs(state.config.polling_interval_secs);
    
    tokio::spawn(async move {
        loop {
            // Update pending transactions
            if let Ok(pending_txids) = state.get_pending_transactions().await {
                for txid in pending_txids {
                    if let Err(e) = update_transaction_confirmations(&state, &txid).await {
                        tracing::error!("Error updating TX {}: {}", txid, e);
                    }
                }
            }
            
            // Check watched addresses
            let addresses = {
                let addr_set = state.watched_addresses.read().await;
                addr_set.iter().cloned().collect::<Vec<_>>()
            };
            
            for address in addresses {
                if let Err(e) = check_address_for_new_transactions(&state, &address).await {
                    tracing::error!("Error checking address {}: {}", address, e);
                }
            }
            
            tokio::time::sleep(interval).await;
        }
    });
}

async fn update_transaction_confirmations(state: &AppState, txid: &str) -> Result<(), ServiceError> {
    // Get current state from database
    let old_tx = state.get_transaction(txid).await?;
    let old_confs = old_tx.as_ref().map(|t| t.confirmations).unwrap_or(0);
    
    // Query WhatsOnChain
    let woc_tx = state.woc_get_transaction(txid).await?;
    let new_confs = woc_tx.confirmations.unwrap_or(0);
    
    // Update if changed
    if new_confs != old_confs {
        let tx = Transaction {
            txid: txid.to_string(),
            tx_type: old_tx.as_ref().and_then(|t| t.tx_type.clone()),
            from_address: extract_from_address(&woc_tx),
            to_address: extract_to_address(&woc_tx),
            amount_satoshis: calculate_output_amount(&woc_tx),
            fee_satoshis: None,
            confirmations: new_confs,
            status: if new_confs > 0 { "confirmed" } else { "pending" }.to_string(),
            block_hash: woc_tx.blockhash.clone(),
            block_height: woc_tx.blockheight,
            block_time: woc_tx.blocktime.map(|t| {
                DateTime::<Utc>::from_timestamp(t, 0).unwrap_or_else(Utc::now)
            }),
            raw_tx: woc_tx.hex,
        };
        
        state.save_transaction(&tx).await?;
        
        // Log confirmation event
        let update = ConfirmationUpdate {
            txid: txid.to_string(),
            old_confirmations: old_confs,
            new_confirmations: new_confs,
            block_height: woc_tx.blockheight,
        };
        
        state.save_confirmation_event(&update).await?;
        
        tracing::info!("TX {} confirmations: {} → {}", txid, old_confs, new_confs);
    }
    
    Ok(())
}

async fn check_address_for_new_transactions(state: &AppState, address: &str) -> Result<(), ServiceError> {
    // Get UTXOs for address
    let utxos = state.woc_get_address_utxos(address).await?;
    
    // Check each UTXO's transaction
    for utxo in utxos {
        // Check if we've seen this transaction
        if state.get_transaction(&utxo.tx_hash).await?.is_none() {
            // New transaction found!
            tracing::info!("New TX detected for {}: {}", address, utxo.tx_hash);
            
            // Fetch and store transaction
            if let Err(e) = update_transaction_confirmations(state, &utxo.tx_hash).await {
                tracing::error!("Failed to fetch new TX: {}", e);
            }
        }
    }
    
    Ok(())
}

fn extract_from_address(woc_tx: &WocTransaction) -> Option<String> {
    woc_tx.inputs.as_ref()
        .and_then(|inputs| inputs.first())
        .and_then(|input| input.script_sig.as_ref())
        .and_then(|script| script.addresses.as_ref())
        .and_then(|addrs| addrs.first())
        .cloned()
}

fn extract_to_address(woc_tx: &WocTransaction) -> Option<String> {
    woc_tx.outputs.as_ref()
        .and_then(|outputs| outputs.first())
        .and_then(|output| output.script_pub_key.as_ref())
        .and_then(|script| script.addresses.as_ref())
        .and_then(|addrs| addrs.first())
        .cloned()
}

fn calculate_output_amount(woc_tx: &WocTransaction) -> i64 {
    woc_tx.outputs.as_ref()
        .map(|outputs| {
            outputs.iter()
                .filter_map(|o| o.value)
                .map(|v| (v * 100_000_000.0) as i64)
                .sum()
        })
        .unwrap_or(0)
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
        "service": "blockchain-monitor",
        "status": "healthy",
        "network": data.config.network,
        "version": "0.1.0",
        "uptime_seconds": uptime,
        "features": ["woc-integration", "tx-monitoring", "phase6-hardening"]
    })))
}

async fn liveness_check() -> Result<HttpResponse, ServiceError> {
    Ok(HttpResponse::Ok().json(serde_json::json!({"status": "alive"})))
}

async fn readiness_check(data: web::Data<AppState>) -> Result<HttpResponse, ServiceError> {
    // Check database connection
    let db_ok = sqlx::query("SELECT 1")
        .fetch_optional(&data.db)
        .await
        .is_ok();
    
    // Check WoC API
    let woc_ok = data.woc_get_chain_info().await.is_ok();
    
    if db_ok && woc_ok {
        Ok(HttpResponse::Ok().json(serde_json::json!({
            "status": "ready",
            "checks": {
                "database": "ok",
                "whatsonchain_api": "ok"
            }
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
struct GetTxQuery {
    include_raw: Option<bool>,
}

async fn get_transaction(
    data: web::Data<AppState>,
    txid: web::Path<String>,
    query: web::Query<GetTxQuery>,
) -> Result<HttpResponse, ServiceError> {
    // Phase 6: Validate txid
    validate_txid(&txid)
        .map_err(|e| ServiceError::ValidationError(e.to_string()))?;
    
    // Check cache first
    {
        let cache = data.tx_cache.read().await;
        if let Some(tx) = cache.get(txid.as_str()) {
            let mut response = tx.clone();
            if !query.include_raw.unwrap_or(false) {
                response.raw_tx = None;
            }
            return Ok(HttpResponse::Ok().json(response));
        }
    }
    
    // Check database
    match data.get_transaction(&txid).await? {
        Some(tx) => {
            // Cache it
            {
                let mut cache = data.tx_cache.write().await;
                cache.insert(txid.to_string(), tx.clone());
            }
            
            let mut response = tx;
            if !query.include_raw.unwrap_or(false) {
                response.raw_tx = None;
            }
            Ok(HttpResponse::Ok().json(response))
        }
        None => {
            // Not in DB, query WhatsOnChain
            let woc_tx = data.woc_get_transaction(&txid).await?;
            
            let tx = Transaction {
                txid: txid.to_string(),
                tx_type: None,
                from_address: extract_from_address(&woc_tx),
                to_address: extract_to_address(&woc_tx),
                amount_satoshis: calculate_output_amount(&woc_tx),
                fee_satoshis: None,
                confirmations: woc_tx.confirmations.unwrap_or(0),
                status: if woc_tx.confirmations.unwrap_or(0) > 0 { 
                    "confirmed" 
                } else { 
                    "pending" 
                }.to_string(),
                block_hash: woc_tx.blockhash.clone(),
                block_height: woc_tx.blockheight,
                block_time: woc_tx.blocktime.map(|t| {
                    DateTime::<Utc>::from_timestamp(t, 0).unwrap_or_else(Utc::now)
                }),
                raw_tx: if query.include_raw.unwrap_or(false) {
                    woc_tx.hex
                } else {
                    None
                },
            };
            
            // Save to database
            let _ = data.save_transaction(&tx).await;
            
            Ok(HttpResponse::Ok().json(tx))
        }
    }
}

#[derive(Serialize)]
struct ConfirmationsResponse {
    txid: String,
    confirmations: i32,
    status: String,
}

async fn get_confirmations(
    data: web::Data<AppState>,
    txid: web::Path<String>,
) -> Result<HttpResponse, ServiceError> {
    // Phase 6: Validate txid
    validate_txid(&txid)
        .map_err(|e| ServiceError::ValidationError(e.to_string()))?;
    
    let woc_tx = data.woc_get_transaction(&txid).await?;
    let confirmations = woc_tx.confirmations.unwrap_or(0);
    
    Ok(HttpResponse::Ok().json(ConfirmationsResponse {
        txid: txid.to_string(),
        confirmations,
        status: if confirmations > 0 { "confirmed" } else { "pending" }.to_string(),
    }))
}

async fn get_chain_info(data: web::Data<AppState>) -> Result<HttpResponse, ServiceError> {
    let info = data.woc_get_chain_info().await?;
    
    Ok(HttpResponse::Ok().json(ChainInfo {
        height: info.blocks,
        best_block_hash: info.bestblockhash,
        difficulty: info.difficulty,
        chain: info.chain,
    }))
}

#[derive(Serialize)]
struct AddressBalanceResponse {
    address: String,
    confirmed_satoshis: i64,
    unconfirmed_satoshis: i64,
    total_satoshis: i64,
}

async fn get_address_balance(
    data: web::Data<AppState>,
    address: web::Path<String>,
) -> Result<HttpResponse, ServiceError> {
    // Phase 6: Validate address
    validate_address(&address)
        .map_err(|e| ServiceError::ValidationError(e.to_string()))?;
    
    let balance = data.woc_get_address_balance(&address).await?;
    
    Ok(HttpResponse::Ok().json(AddressBalanceResponse {
        address: address.to_string(),
        confirmed_satoshis: balance.confirmed,
        unconfirmed_satoshis: balance.unconfirmed,
        total_satoshis: balance.confirmed + balance.unconfirmed,
    }))
}

#[derive(Serialize)]
struct AddressUtxosResponse {
    address: String,
    utxos: Vec<Utxo>,
    total_value: i64,
}

async fn get_address_utxos(
    data: web::Data<AppState>,
    address: web::Path<String>,
) -> Result<HttpResponse, ServiceError> {
    // Phase 6: Validate address
    validate_address(&address)
        .map_err(|e| ServiceError::ValidationError(e.to_string()))?;
    
    let woc_utxos = data.woc_get_address_utxos(&address).await?;
    
    let utxos: Vec<Utxo> = woc_utxos.into_iter().map(|u| Utxo {
        txid: u.tx_hash,
        vout: u.tx_pos,
        value: u.value,
        height: u.height,
    }).collect();
    
    let total_value = utxos.iter().map(|u| u.value).sum();
    
    Ok(HttpResponse::Ok().json(AddressUtxosResponse {
        address: address.to_string(),
        utxos,
        total_value,
    }))
}

#[derive(Deserialize)]
struct WatchAddressRequest {
    address: String,
    paymail: String,
    purpose: String,
}

#[derive(Serialize)]
struct WatchAddressResponse {
    success: bool,
    address: String,
    message: String,
}

async fn watch_address(
    data: web::Data<AppState>,
    req: web::Json<WatchAddressRequest>,
) -> Result<HttpResponse, ServiceError> {
    // Phase 6: Validate address
    validate_address(&req.address)
        .map_err(|e| ServiceError::ValidationError(e.to_string()))?;
    
    data.add_watched_address(&req.address, &req.paymail, &req.purpose).await?;
    
    Ok(HttpResponse::Ok().json(WatchAddressResponse {
        success: true,
        address: req.address.clone(),
        message: "Address is now being monitored".to_string(),
    }))
}

#[derive(Deserialize)]
struct BroadcastRequest {
    tx_hex: String,
}

#[derive(Serialize)]
struct BroadcastResponse {
    success: bool,
    txid: String,
}

async fn broadcast_transaction(
    data: web::Data<AppState>,
    req: web::Json<BroadcastRequest>,
) -> Result<HttpResponse, ServiceError> {
    // Phase 6: Validate hex format (basic check)
    if req.tx_hex.is_empty() || req.tx_hex.len() % 2 != 0 {
        return Err(ServiceError::ValidationError("Invalid transaction hex".to_string()));
    }
    
    let txid = data.woc_broadcast_transaction(&req.tx_hex).await?;
    
    // Start monitoring this transaction
    let _ = update_transaction_confirmations(&data, &txid).await;
    
    Ok(HttpResponse::Ok().json(BroadcastResponse {
        success: true,
        txid,
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
    // Load environment variables
    dotenv().ok();
    
    println!("🚀 BSV Bank - Blockchain Monitor Service Starting (Phase 6)...");
    
    let config = Config::from_env();
    println!("   Network: {}", config.network);
    println!("   API: {}", config.woc_api_base);
    println!("   Polling interval: {}s", config.polling_interval_secs);
    
    // Phase 6: Initialize structured logging
    init_logging("blockchain-monitor");
    tracing::info!("Starting Blockchain Monitor on port 8084");
    tracing::info!("WhatsOnChain API: {}", config.woc_api_base);
    
    // Initialize application state
    let state = web::Data::new(
        AppState::new(config.clone())
            .await
            .expect("Failed to initialize application state")
    );
    
    tracing::info!("Database connection established");
    
    // Phase 6: Prometheus metrics
    let registry = Registry::new();
    let _service_metrics = ServiceMetrics::new(&registry, "blockchain_monitor")
        .expect("Failed to create service metrics");
    tracing::info!("Metrics initialized");
    
    let registry_data = web::Data::new(registry);
    
    // Start background monitoring task
    start_monitoring_task(state.clone()).await;
    tracing::info!("Background monitoring task started");
    
    println!("✅ Service ready on http://127.0.0.1:8084");
    println!("📋 Health: http://127.0.0.1:8084/health");
    println!("📊 Metrics: http://127.0.0.1:8084/metrics");
    tracing::info!("Starting HTTP server...");
    
    HttpServer::new(move || {
        // Phase 6: CORS configuration
        let cors = Cors::default()
            .allowed_origin("http://localhost:3000")
            .allowed_origin("http://localhost:5173")
            .allowed_methods(vec!["GET", "POST"])
            .allowed_headers(vec![
                actix_web::http::header::CONTENT_TYPE,
            ])
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
            
            // Transaction endpoints
            .route("/tx/{txid}", web::get().to(get_transaction))
            .route("/tx/{txid}/confirmations", web::get().to(get_confirmations))
            
            // Chain info
            .route("/chain/info", web::get().to(get_chain_info))
            
            // Address endpoints
            .route("/address/{address}/balance", web::get().to(get_address_balance))
            .route("/address/{address}/utxos", web::get().to(get_address_utxos))
            
            // Monitoring endpoints
            .route("/watch/address", web::post().to(watch_address))
            
            // Broadcast endpoint
            .route("/broadcast", web::post().to(broadcast_transaction))
    })
    .bind("127.0.0.1:8084")?
    .run()
    .await
}