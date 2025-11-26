// services/blockchain-monitor/src/main.rs
// Blockchain Monitor Service - Port 8084
// Monitors BSV testnet via WhatsOnChain API

use actix_web::{web, App, HttpResponse, HttpServer, Result};
use actix_cors::Cors;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::{HashMap, HashSet};
use chrono::{DateTime, Utc};

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
    async fn woc_get_transaction(&self, txid: &str) -> Result<WocTransaction, String> {
        let url = format!("{}/tx/{}", self.config.woc_api_base, txid);
        
        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("WoC API error: {}", e))?;
        
        if !response.status().is_success() {
            return Err(format!("WoC API returned status: {}", response.status()));
        }
        
        response
            .json::<WocTransaction>()
            .await
            .map_err(|e| format!("Failed to parse WoC response: {}", e))
    }
    
    async fn woc_get_chain_info(&self) -> Result<WocChainInfo, String> {
        let url = format!("{}/chain/info", self.config.woc_api_base);
        
        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("WoC API error: {}", e))?;
        
        response
            .json::<WocChainInfo>()
            .await
            .map_err(|e| format!("Failed to parse WoC response: {}", e))
    }
    
    async fn woc_get_address_utxos(&self, address: &str) -> Result<Vec<WocUtxo>, String> {
        let url = format!("{}/address/{}/unspent", self.config.woc_api_base, address);
        
        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("WoC API error: {}", e))?;
        
        if !response.status().is_success() {
            return Ok(vec![]); // Address has no UTXOs
        }
        
        response
            .json::<Vec<WocUtxo>>()
            .await
            .map_err(|e| format!("Failed to parse WoC response: {}", e))
    }
    
    async fn woc_get_address_balance(&self, address: &str) -> Result<WocBalance, String> {
        let url = format!("{}/address/{}/balance", self.config.woc_api_base, address);
        
        log::debug!("Fetching balance from: {}", url);
        
        let response = self.client
            .get(&url)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| format!("WoC API error: {}", e))?;
        
        let status = response.status();
        log::debug!("WoC API response status: {}", status);
        
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            log::error!("WoC API error for {}: status={}, body={}", address, status, error_text);
            return Err(format!("WoC API returned status: {} - {}", status, error_text));
        }
        
        // Get response as text first for better error messages
        let response_text = response
            .text()
            .await
            .map_err(|e| format!("Failed to read response body: {}", e))?;
        
        log::debug!("WoC API raw response: {}", response_text);
        
        // Check for empty response
        if response_text.trim().is_empty() {
            log::warn!("Empty response from WoC for address: {}", address);
            return Ok(WocBalance {
                confirmed: 0,
                unconfirmed: 0,
            });
        }
        
        // Parse JSON
        serde_json::from_str::<WocBalance>(&response_text)
            .map_err(|e| {
                log::error!("JSON parse error: {} (response was: '{}')", e, response_text);
                format!("Failed to parse WoC response: error decoding response body: {}", e)
            })
    }
    
    async fn woc_broadcast_transaction(&self, tx_hex: &str) -> Result<String, String> {
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
            .map_err(|e| format!("WoC API error: {}", e))?;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(format!("Broadcast failed: {}", error_text));
        }
        
        response
            .text()
            .await
            .map_err(|e| format!("Failed to get txid: {}", e))
    }
}

// ============================================================================
// Database Operations
// ============================================================================

impl AppState {
    async fn save_transaction(&self, tx: &Transaction) -> Result<(), sqlx::Error> {
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
        .await?;
        
        Ok(())
    }
    
    async fn get_transaction(&self, txid: &str) -> Result<Option<Transaction>, sqlx::Error> {
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
        .await?;
        
        if let Some(row) = row {
            Ok(Some(Transaction {
                txid: row.try_get("txid")?,
                tx_type: row.try_get("tx_type")?,
                from_address: row.try_get("from_address")?,
                to_address: row.try_get("to_address")?,
                amount_satoshis: row.try_get("amount_satoshis")?,
                fee_satoshis: row.try_get("fee_satoshis")?,
                confirmations: row.try_get("confirmations")?,
                status: row.try_get("status")?,
                block_hash: row.try_get("block_hash")?,
                block_height: row.try_get("block_height")?,
                block_time: row.try_get("block_time")?,
                raw_tx: row.try_get("raw_tx")?,
            }))
        } else {
            Ok(None)
        }
    }
    
    async fn save_confirmation_event(&self, update: &ConfirmationUpdate) -> Result<(), sqlx::Error> {
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
        .await?;
        
        Ok(())
    }
    
    async fn add_watched_address(&self, address: &str, paymail: &str, purpose: &str) -> Result<(), sqlx::Error> {
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
        .await?;
        
        // Add to in-memory set
        let mut addresses = self.watched_addresses.write().await;
        addresses.insert(address.to_string());
        
        Ok(())
    }
    
    async fn get_pending_transactions(&self) -> Result<Vec<String>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT txid FROM blockchain_transactions
            WHERE status = 'pending' OR confirmations < 6
            ORDER BY first_seen DESC
            LIMIT 100
            "#
        )
        .fetch_all(&self.db)
        .await?;
        
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
                        eprintln!("Error updating TX {}: {}", txid, e);
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
                    eprintln!("Error checking address {}: {}", address, e);
                }
            }
            
            tokio::time::sleep(interval).await;
        }
    });
}

async fn update_transaction_confirmations(state: &AppState, txid: &str) -> Result<(), String> {
    // Get current state from database
    let old_tx = state.get_transaction(txid).await
        .map_err(|e| format!("DB error: {}", e))?;
    
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
        
        state.save_transaction(&tx).await
            .map_err(|e| format!("Failed to save TX: {}", e))?;
        
        // Log confirmation event
        let update = ConfirmationUpdate {
            txid: txid.to_string(),
            old_confirmations: old_confs,
            new_confirmations: new_confs,
            block_height: woc_tx.blockheight,
        };
        
        state.save_confirmation_event(&update).await
            .map_err(|e| format!("Failed to save confirmation event: {}", e))?;
        
        println!("✓ TX {} confirmations: {} → {}", txid, old_confs, new_confs);
    }
    
    Ok(())
}

async fn check_address_for_new_transactions(state: &AppState, address: &str) -> Result<(), String> {
    // Get UTXOs for address
    let utxos = state.woc_get_address_utxos(address).await?;
    
    // Check each UTXO's transaction
    for utxo in utxos {
        // Check if we've seen this transaction
        if state.get_transaction(&utxo.tx_hash).await
            .map_err(|e| format!("DB error: {}", e))?
            .is_none() 
        {
            // New transaction found!
            println!("🆕 New TX detected for {}: {}", address, utxo.tx_hash);
            
            // Fetch and store transaction
            if let Err(e) = update_transaction_confirmations(state, &utxo.tx_hash).await {
                eprintln!("Failed to fetch new TX: {}", e);
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
        service: "blockchain-monitor".to_string(),
        network: data.config.network.clone(),
        version: "0.1.0".to_string(),
    }))
}

#[derive(Deserialize)]
struct GetTxQuery {
    include_raw: Option<bool>,
}

async fn get_transaction(
    data: web::Data<AppState>,
    txid: web::Path<String>,
    query: web::Query<GetTxQuery>,
) -> Result<HttpResponse> {
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
    match data.get_transaction(&txid).await {
        Ok(Some(tx)) => {
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
        Ok(None) => {
            // Not in DB, query WhatsOnChain
            match data.woc_get_transaction(&txid).await {
                Ok(woc_tx) => {
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
                Err(e) => Ok(HttpResponse::NotFound().json(serde_json::json!({
                    "error": "Transaction not found",
                    "details": e
                })))
            }
        }
        Err(e) => Ok(HttpResponse::InternalServerError().json(serde_json::json!({
            "error": "Database error",
            "details": e.to_string()
        })))
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
) -> Result<HttpResponse> {
    match data.woc_get_transaction(&txid).await {
        Ok(woc_tx) => {
            let confirmations = woc_tx.confirmations.unwrap_or(0);
            Ok(HttpResponse::Ok().json(ConfirmationsResponse {
                txid: txid.to_string(),
                confirmations,
                status: if confirmations > 0 { "confirmed" } else { "pending" }.to_string(),
            }))
        }
        Err(e) => Ok(HttpResponse::NotFound().json(serde_json::json!({
            "error": "Transaction not found",
            "details": e
        })))
    }
}

async fn get_chain_info(data: web::Data<AppState>) -> Result<HttpResponse> {
    match data.woc_get_chain_info().await {
        Ok(info) => Ok(HttpResponse::Ok().json(ChainInfo {
            height: info.blocks,
            best_block_hash: info.bestblockhash,
            difficulty: info.difficulty,
            chain: info.chain,
        })),
        Err(e) => Ok(HttpResponse::InternalServerError().json(serde_json::json!({
            "error": "Failed to get chain info",
            "details": e
        })))
    }
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
) -> Result<HttpResponse> {
    match data.woc_get_address_balance(&address).await {
        Ok(balance) => Ok(HttpResponse::Ok().json(AddressBalanceResponse {
            address: address.to_string(),
            confirmed_satoshis: balance.confirmed,
            unconfirmed_satoshis: balance.unconfirmed,
            total_satoshis: balance.confirmed + balance.unconfirmed,
        })),
        Err(e) => Ok(HttpResponse::InternalServerError().json(serde_json::json!({
            "error": "Failed to get balance",
            "details": e
        })))
    }
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
) -> Result<HttpResponse> {
    match data.woc_get_address_utxos(&address).await {
        Ok(woc_utxos) => {
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
        Err(e) => Ok(HttpResponse::InternalServerError().json(serde_json::json!({
            "error": "Failed to get UTXOs",
            "details": e
        })))
    }
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
) -> Result<HttpResponse> {
    match data.add_watched_address(&req.address, &req.paymail, &req.purpose).await {
        Ok(_) => Ok(HttpResponse::Ok().json(WatchAddressResponse {
            success: true,
            address: req.address.clone(),
            message: "Address is now being monitored".to_string(),
        })),
        Err(e) => Ok(HttpResponse::InternalServerError().json(serde_json::json!({
            "error": "Failed to watch address",
            "details": e.to_string()
        })))
    }
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
) -> Result<HttpResponse> {
    match data.woc_broadcast_transaction(&req.tx_hex).await {
        Ok(txid) => {
            // Start monitoring this transaction
            let _ = update_transaction_confirmations(&data, &txid).await;
            
            Ok(HttpResponse::Ok().json(BroadcastResponse {
                success: true,
                txid,
            }))
        }
        Err(e) => Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Broadcast failed",
            "details": e
        })))
    }
}

// ============================================================================
// Main Application
// ============================================================================

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    
    let config = Config::from_env();
    println!("🚀 Starting Blockchain Monitor Service");
    println!("   Network: {}", config.network);
    println!("   API: {}", config.woc_api_base);
    println!("   Polling interval: {}s", config.polling_interval_secs);
    
    let state = web::Data::new(
        AppState::new(config.clone())
            .await
            .expect("Failed to initialize application state")
    );
    
    // Start background monitoring task
    start_monitoring_task(state.clone()).await;
    
    println!("✓ Background monitoring task started");
    println!("✓ Server starting on http://127.0.0.1:8084");
    
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
            .route("/tx/{txid}", web::get().to(get_transaction))
            .route("/tx/{txid}/confirmations", web::get().to(get_confirmations))
            .route("/chain/info", web::get().to(get_chain_info))
            .route("/address/{address}/balance", web::get().to(get_address_balance))
            .route("/address/{address}/utxos", web::get().to(get_address_utxos))
            .route("/watch/address", web::post().to(watch_address))
            .route("/broadcast", web::post().to(broadcast_transaction))
    })
    .bind("127.0.0.1:8084")?
    .run()
    .await
}