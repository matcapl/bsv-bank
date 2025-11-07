mod database;
mod auth;
mod node_integration;
mod validation;

use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use chrono::Utc;
use uuid::Uuid;
use sqlx::PgPool;

#[derive(Debug, Serialize, Deserialize)]
pub struct DepositRequest {
    pub user_paymail: String,
    pub amount_satoshis: i64,
    pub txid: String,
    pub lock_duration_days: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DepositResponse {
    pub deposit_id: String,
    pub status: String,
    pub estimated_confirmation_time: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BalanceResponse {
    pub paymail: String,
    pub balance_satoshis: i64,
    pub accrued_interest_satoshis: i64,
    pub total_available_satoshis: i64,
    pub current_apy: f64,
    pub active_deposits: i64,
}

async fn create_deposit(
    pool: web::Data<PgPool>,
    request: web::Json<DepositRequest>,
) -> impl Responder {
    // Validate inputs
    if let Err(e) = validation::validate_paymail(&request.user_paymail) {
        return HttpResponse::BadRequest().json(serde_json::json!({"error": e}));
    }
    if let Err(e) = validation::validate_txid(&request.txid) {
        return HttpResponse::BadRequest().json(serde_json::json!({"error": e}));
    }
    if let Err(e) = validation::validate_amount(request.amount_satoshis) {
        return HttpResponse::BadRequest().json(serde_json::json!({"error": e}));
    }

    match node_integration::verify_transaction_real(&request.txid).await {
        Ok((verified, confirmations)) => {
            if !verified {
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "Transaction verification failed"
                }));
            }
            
            let user_id = match database::get_or_create_user(&pool, &request.user_paymail).await {
                Ok(id) => id,
                Err(e) => {
                    eprintln!("Database error: {}", e);
                    return HttpResponse::InternalServerError().json(serde_json::json!({
                        "error": "Database error"
                    }));
                }
            };
            
            let deposit_id = Uuid::new_v4();
            let now = Utc::now();
            let status = if confirmations >= 6 { "Confirmed" } else { "Pending" };
            
            let lock_until = request.lock_duration_days.map(|days| {
                now + chrono::Duration::days(days as i64)
            });
            
            let result = sqlx::query!(
                r#"
                INSERT INTO deposits (
                    id, user_id, paymail, amount_satoshis, txid, 
                    confirmations, status, lock_until, created_at, confirmed_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                RETURNING id
                "#,
                deposit_id,
                user_id,
                request.user_paymail,
                request.amount_satoshis,
                request.txid,
                confirmations as i32,
                status,
                lock_until,
                now,
                if confirmations >= 6 { Some(now) } else { None }
            )
            .fetch_one(pool.as_ref())
            .await;
            
            match result {
                Ok(_) => {
                    let commitment_data = format!(
                        "DEPOSIT|{}|{}|{}|{}",
                        request.user_paymail,
                        request.amount_satoshis,
                        request.txid,
                        now.timestamp()
                    );
                    let mut hasher = Sha256::new();
                    hasher.update(commitment_data.as_bytes());
                    let hash = hex::encode(hasher.finalize());
                    println!("On-chain commitment: 6a{}", hash);
                    
                    HttpResponse::Ok().json(DepositResponse {
                        deposit_id: deposit_id.to_string(),
                        status: status.to_string(),
                        estimated_confirmation_time: "~60 seconds".to_string(),
                    })
                }
                Err(e) => {
                    eprintln!("Failed to insert deposit: {}", e);
                    HttpResponse::InternalServerError().json(serde_json::json!({
                        "error": "Failed to create deposit"
                    }))
                }
            }
        }
        Err(e) => {
            HttpResponse::BadRequest().json(serde_json::json!({
                "error": e
            }))
        }
    }
}

async fn get_user_balance(
    pool: web::Data<PgPool>,
    paymail: web::Path<String>,
) -> impl Responder {
    // Validate paymail
    if let Err(e) = validation::validate_paymail(&paymail) {
        return HttpResponse::BadRequest().json(serde_json::json!({"error": e}));
    }

    let result = sqlx::query!(
        r#"
        SELECT 
            balance_satoshis,
            accrued_interest_satoshis,
            active_deposits
        FROM user_balances
        WHERE paymail = $1
        "#,
        paymail.as_str()
    )
    .fetch_optional(pool.as_ref())
    .await;
    
    match result {
        Ok(Some(balance)) => {
            let bal = balance.balance_satoshis.unwrap_or(0);
            let interest = balance.accrued_interest_satoshis.unwrap_or(0);
            
            HttpResponse::Ok().json(BalanceResponse {
                paymail: paymail.to_string(),
                balance_satoshis: bal,
                accrued_interest_satoshis: interest,
                total_available_satoshis: bal + interest,
                current_apy: 7.0,
                active_deposits: balance.active_deposits.unwrap_or(0),
            })
        }
        Ok(None) => {
            HttpResponse::Ok().json(BalanceResponse {
                paymail: paymail.to_string(),
                balance_satoshis: 0,
                accrued_interest_satoshis: 0,
                total_available_satoshis: 0,
                current_apy: 7.0,
                active_deposits: 0,
            })
        }
        Err(e) => {
            eprintln!("Database error: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Database error"
            }))
        }
    }
}

async fn health_check(pool: web::Data<PgPool>) -> impl Responder {
    let db_health = sqlx::query("SELECT 1")
        .fetch_one(pool.as_ref())
        .await
        .is_ok();
    
    HttpResponse::Ok().json(serde_json::json!({
        "service": "deposit-service",
        "status": "healthy",
        "version": "0.2.0",
        "database": if db_health { "connected" } else { "disconnected" }
    }))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("🏦 BSV Bank - Deposit Service Starting...");
    println!("📡 Connecting to database...");
    
    let pool = database::create_pool().await
        .expect("Failed to create database pool");
    
    println!("✅ Database connected");
    println!("✅ Service ready on http://0.0.0.0:8080");
    
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(pool.clone()))
            .route("/health", web::get().to(health_check))
            .route("/deposits", web::post().to(create_deposit))
            .route("/balance/{paymail}", web::get().to(get_user_balance))
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}
