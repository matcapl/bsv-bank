// core/deposit-service/src/main.rs
// Deposit handling microservice with SPV verification
// Integrates with Galaxy node and SPV wallet infrastructure

use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use chrono::{DateTime, Utc};

// Core data structures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deposit {
    pub id: String,
    pub user_paymail: String,
    pub amount_satoshis: u64,
    pub txid: String,
    pub block_height: Option<u64>,
    pub confirmations: u32,
    pub status: DepositStatus,
    pub created_at: DateTime<Utc>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub lock_until: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DepositStatus {
    Pending,
    Confirmed,
    Available,
    Locked,
    Withdrawn,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DepositRequest {
    pub user_paymail: String,
    pub amount_satoshis: u64,
    pub txid: String,
    pub lock_duration_days: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DepositResponse {
    pub deposit_id: String,
    pub status: DepositStatus,
    pub estimated_confirmation_time: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WithdrawalRequest {
    pub deposit_id: String,
    pub destination_address: String,
    pub signature: String,
}

// In-memory state (would use PostgreSQL + Redis in production)
pub struct AppState {
    deposits: Arc<Mutex<HashMap<String, Deposit>>>,
    user_balances: Arc<Mutex<HashMap<String, u64>>>,
}

impl AppState {
    pub fn new() -> Self {
        AppState {
            deposits: Arc::new(Mutex::new(HashMap::new())),
            user_balances: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

// SPV verification simulation (integrates with Galaxy node in production)
async fn verify_transaction_spv(txid: &str) -> Result<(bool, u32), String> {
    // In production: query Galaxy node for merkle proof and block header
    // For now, simulate basic verification
    
    // Check if txid format is valid (64 hex chars)
    if txid.len() != 64 {
        return Err("Invalid transaction ID format".to_string());
    }
    
    // Simulate SPV verification with merkle proof
    // Real implementation would:
    // 1. Request merkle proof from Galaxy node
    // 2. Verify merkle branch against block header
    // 3. Check block header is in longest chain
    // 4. Count confirmations
    
    Ok((true, 6)) // Simulated: verified with 6 confirmations
}

// Create OP_RETURN commitment for deposit
fn create_deposit_commitment(deposit: &Deposit) -> String {
    let commitment_data = format!(
        "DEPOSIT|{}|{}|{}|{}",
        deposit.user_paymail,
        deposit.amount_satoshis,
        deposit.txid,
        deposit.created_at.timestamp()
    );
    
    let mut hasher = Sha256::new();
    hasher.update(commitment_data.as_bytes());
    let hash = hasher.finalize();
    
    format!("6a{}", hex::encode(hash)) // OP_RETURN + hash
}

// Calculate interest rate based on utilization
fn calculate_interest_rate(total_deposits: u64, total_borrowed: u64) -> f64 {
    let utilization = if total_deposits == 0 {
        0.0
    } else {
        total_borrowed as f64 / total_deposits as f64
    };
    
    let base_rate = 0.02; // 2% APY
    let slope1 = 0.10; // 10% up to 80% utilization
    let slope2 = 1.00; // 100% above 80% utilization
    let optimal_utilization = 0.80;
    
    if utilization <= optimal_utilization {
        base_rate + (utilization * slope1)
    } else {
        base_rate + (optimal_utilization * slope1) + 
        ((utilization - optimal_utilization) * slope2)
    }
}

// REST API handlers
async fn create_deposit(
    data: web::Data<AppState>,
    request: web::Json<DepositRequest>,
) -> impl Responder {
    // Verify transaction via SPV
    match verify_transaction_spv(&request.txid).await {
        Ok((verified, confirmations)) => {
            if !verified {
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "Transaction verification failed"
                }));
            }
            
            let deposit_id = format!("DEP_{}", uuid::Uuid::new_v4());
            let now = Utc::now();
            
            let lock_until = request.lock_duration_days.map(|days| {
                now + chrono::Duration::days(days as i64)
            });
            
            let deposit = Deposit {
                id: deposit_id.clone(),
                user_paymail: request.user_paymail.clone(),
                amount_satoshis: request.amount_satoshis,
                txid: request.txid.clone(),
                block_height: None,
                confirmations,
                status: if confirmations >= 6 {
                    DepositStatus::Confirmed
                } else {
                    DepositStatus::Pending
                },
                created_at: now,
                confirmed_at: if confirmations >= 6 { Some(now) } else { None },
                lock_until,
            };
            
            // Create on-chain commitment (would broadcast via Galaxy in production)
            let commitment = create_deposit_commitment(&deposit);
            println!("On-chain commitment: {}", commitment);
            
            // Update state
            let mut deposits = data.deposits.lock().unwrap();
            deposits.insert(deposit_id.clone(), deposit.clone());
            
            if deposit.status == DepositStatus::Confirmed {
                let mut balances = data.user_balances.lock().unwrap();
                *balances.entry(request.user_paymail.clone()).or_insert(0) += 
                    request.amount_satoshis;
            }
            
            HttpResponse::Ok().json(DepositResponse {
                deposit_id,
                status: deposit.status,
                estimated_confirmation_time: "~60 seconds".to_string(),
            })
        }
        Err(e) => {
            HttpResponse::BadRequest().json(serde_json::json!({
                "error": e
            }))
        }
    }
}

async fn get_user_balance(
    data: web::Data<AppState>,
    paymail: web::Path<String>,
) -> impl Responder {
    let balances = data.user_balances.lock().unwrap();
    let balance = balances.get(paymail.as_str()).unwrap_or(&0);
    
    // Calculate accrued interest (simplified)
    let deposits = data.deposits.lock().unwrap();
    let user_deposits: Vec<&Deposit> = deposits
        .values()
        .filter(|d| d.user_paymail == *paymail && d.status == DepositStatus::Confirmed)
        .collect();
    
    let mut total_interest = 0u64;
    for deposit in user_deposits.iter() {
        if let Some(confirmed_at) = deposit.confirmed_at {
            let days_elapsed = (Utc::now() - confirmed_at).num_days() as f64;
            let annual_rate = calculate_interest_rate(1000000, 500000); // Simplified
            let daily_rate = annual_rate / 365.0;
            let interest = (deposit.amount_satoshis as f64 * daily_rate * days_elapsed) as u64;
            total_interest += interest;
        }
    }
    
    HttpResponse::Ok().json(serde_json::json!({
        "paymail": paymail.as_str(),
        "balance_satoshis": balance,
        "accrued_interest_satoshis": total_interest,
        "total_available_satoshis": balance + total_interest,
        "current_apy": calculate_interest_rate(1000000, 500000) * 100.0,
        "active_deposits": user_deposits.len(),
    }))
}

async fn initiate_withdrawal(
    data: web::Data<AppState>,
    request: web::Json<WithdrawalRequest>,
) -> impl Responder {
    let mut deposits = data.deposits.lock().unwrap();
    
    match deposits.get_mut(&request.deposit_id) {
        Some(deposit) => {
            // Verify lock period
            if let Some(lock_until) = deposit.lock_until {
                if Utc::now() < lock_until {
                    return HttpResponse::BadRequest().json(serde_json::json!({
                        "error": "Deposit is still locked",
                        "unlock_date": lock_until
                    }));
                }
            }
            
            // In production: verify signature against user's public key
            // For now, basic check
            if request.signature.len() < 64 {
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "Invalid signature"
                }));
            }
            
            deposit.status = DepositStatus::Withdrawn;
            
            // Create withdrawal transaction via Galaxy node
            let withdrawal_txid = format!("WITHDRAW_{}", uuid::Uuid::new_v4());
            
            // Update balance
            let mut balances = data.user_balances.lock().unwrap();
            if let Some(balance) = balances.get_mut(&deposit.user_paymail) {
                *balance = balance.saturating_sub(deposit.amount_satoshis);
            }
            
            HttpResponse::Ok().json(serde_json::json!({
                "status": "success",
                "withdrawal_txid": withdrawal_txid,
                "amount_satoshis": deposit.amount_satoshis,
                "destination": request.destination_address
            }))
        }
        None => {
            HttpResponse::NotFound().json(serde_json::json!({
                "error": "Deposit not found"
            }))
        }
    }
}

async fn health_check() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "service": "deposit-service",
        "status": "healthy",
        "version": "0.1.0",
        "timestamp": Utc::now()
    }))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("🏦 BSV Bank - Deposit Service Starting...");
    println!("📡 Connecting to Galaxy node...");
    println!("✅ Service ready on http://0.0.0.0:8080");
    
    let app_state = web::Data::new(AppState::new());
    
    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .route("/health", web::get().to(health_check))
            .route("/deposits", web::post().to(create_deposit))
            .route("/balance/{paymail}", web::get().to(get_user_balance))
            .route("/withdrawals", web::post().to(initiate_withdrawal))
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_interest_rate_calculation() {
        // Low utilization
        let rate = calculate_interest_rate(1000000, 200000);
        assert!(rate > 0.02 && rate < 0.05);
        
        // Optimal utilization
        let rate = calculate_interest_rate(1000000, 800000);
        assert!(rate > 0.08 && rate < 0.11);
        
        // High utilization (danger zone)
        let rate = calculate_interest_rate(1000000, 950000);
        assert!(rate > 0.20);
    }
    
    #[test]
    fn test_deposit_commitment() {
        let deposit = Deposit {
            id: "TEST123".to_string(),
            user_paymail: "user@handcash.io".to_string(),
            amount_satoshis: 100000,
            txid: "a".repeat(64),
            block_height: None,
            confirmations: 0,
            status: DepositStatus::Pending,
            created_at: Utc::now(),
            confirmed_at: None,
            lock_until: None,
        };
        
        let commitment = create_deposit_commitment(&deposit);
        assert!(commitment.starts_with("6a")); // OP_RETURN
        assert_eq!(commitment.len(), 66); // OP_RETURN (2) + SHA256 hash (64)
    }
}