// core/deposit-service/src/main.rs
// Deposit Service with Phase 6 Production Hardening (MERGED VERSION)

mod database;
mod node_integration;
mod handlers;
mod middleware;

use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use actix_cors::Cors;
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use chrono::Utc;
use uuid::Uuid;
use sqlx::PgPool;
use bsv_bank_common::{
    init_logging, JwtManager, RateLimit, RateLimiter, ServiceMetrics,
    validate_paymail, validate_txid, validate_amount, // Import validators
};
use dotenv::dotenv;
use prometheus::Registry;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use std::time::SystemTime;

// ============================================================================
// EXISTING TYPES (Keep your working structs)
// ============================================================================

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

// ============================================================================
// EXISTING HANDLERS (Keep your working logic, add Phase 6 validation)
// ============================================================================

async fn create_deposit(
    pool: web::Data<PgPool>,
    request: web::Json<DepositRequest>,
) -> impl Responder {
    // Phase 6: Use common library validation
    if let Err(e) = validate_paymail(&request.user_paymail) {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Invalid paymail",
            "error_code": "validation_error",
            "message": e.to_string()
        }));
    }
    if let Err(e) = validate_txid(&request.txid) {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Invalid transaction ID",
            "error_code": "validation_error",
            "message": e.to_string()
        }));
    }
    if let Err(e) = validate_amount(request.amount_satoshis) {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Invalid amount",
            "error_code": "validation_error",
            "message": e.to_string()
        }));
    }

    match node_integration::verify_transaction_real(&request.txid).await {
        Ok(verified) => {
            let confirmations = 6; 
            if !verified {
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "Transaction verification failed",
                    "error_code": "verification_failed"
                }));
            }
            
            let user_id = match database::get_or_create_user(&pool, &request.user_paymail).await {
                Ok(id) => id,
                Err(e) => {
                    eprintln!("Database error: {}", e);
                    return HttpResponse::InternalServerError().json(serde_json::json!({
                        "error": "Database error",
                        "error_code": "database_error"
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
                        "error": "Failed to create deposit",
                        "error_code": "database_error"
                    }))
                }
            }
        }
        Err(e) => {
            HttpResponse::BadRequest().json(serde_json::json!({
                "error": e,
                "error_code": "transaction_error"
            }))
        }
    }
}

async fn get_user_balance(
    pool: web::Data<PgPool>,
    paymail: web::Path<String>,
) -> impl Responder {
    // Phase 6: Use common library validation
    if let Err(e) = validate_paymail(&paymail) {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Invalid paymail",
            "error_code": "validation_error",
            "message": e.to_string()
        }));
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
                "error": "Database error",
                "error_code": "database_error"
            }))
        }
    }
}

// ============================================================================
// MAIN (Phase 6 Enhanced but keeping your existing endpoints)
// ============================================================================

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Load environment variables
    dotenv().ok();
    
    println!("🏦 BSV Bank - Deposit Service Starting (Phase 6)...");
    
    // Get configuration from environment
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| {
            println!("⚠️  DATABASE_URL not set, using default");
            "postgres://postgres:postgres@localhost:5432/bsv_bank".to_string()
        });
    
    let jwt_secret = std::env::var("JWT_SECRET")
        .unwrap_or_else(|_| {
            println!("⚠️  JWT_SECRET not set, using development default");
            "development-secret-change-in-production".to_string()
        });
    
    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse::<u16>()
        .expect("PORT must be a valid number");
    
    // Phase 6: Initialize structured logging
    init_logging("deposit-service");
    tracing::info!("Starting Deposit Service on port {}", port);
    
    // Database connection pool
    println!("📡 Connecting to database...");
    let db_pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
        .expect("Failed to connect to database");
    
    println!("✅ Database connected");
    tracing::info!("Database connection established");
    
    // Phase 6: JWT manager
    let jwt_manager = JwtManager::new(jwt_secret);
    tracing::info!("JWT manager initialized");
    
    // Phase 6: Prometheus metrics
    let registry = Registry::new();
    let service_metrics = ServiceMetrics::new(&registry, "deposit_service")
        .expect("Failed to create service metrics");
    let _deposit_metrics = bsv_bank_common::DepositMetrics::new(&registry)
        .expect("Failed to create deposit metrics");
    
    tracing::info!("Metrics initialized");
    
    // Phase 6: Rate limiter
    let mut rate_limiter = RateLimiter::new();
    rate_limiter.add_limit(
        "deposits".to_string(),
        RateLimit::per_minute(100),
    );
    rate_limiter.add_limit(
        "withdrawals".to_string(),
        RateLimit::per_minute(50),
    );
    rate_limiter.add_limit(
        "auth".to_string(),
        RateLimit::per_minute(10),
    );
    
    let rate_limiter = Arc::new(rate_limiter);
    bsv_bank_common::rate_limit::start_cleanup_task(rate_limiter.clone());
    tracing::info!("Rate limiter initialized");
    
    // Phase 6: Application state for health checks
    let start_time = SystemTime::now();
    let health_state = web::Data::new(handlers::health::AppState {
        db_pool: db_pool.clone(),
        start_time,
    });
    
    // Phase 6: Application state for auth
    let auth_state = web::Data::new(handlers::auth::AuthState {
        db_pool: db_pool.clone(),
        jwt_manager: jwt_manager.clone(),
    });
    
    let registry_data = web::Data::new(registry);
    
    println!("✅ Service ready on http://0.0.0.0:{}", port);
    tracing::info!("Starting HTTP server...");
    
    // HTTP Server
    HttpServer::new(move || {
        // Phase 6: CORS configuration
        let cors = Cors::default()
            .allowed_origin("http://localhost:3000")
            .allowed_origin("http://localhost:5173")
            .allowed_methods(vec!["GET", "POST", "PUT", "DELETE"])
            .allowed_headers(vec![
                actix_web::http::header::AUTHORIZATION,
                actix_web::http::header::CONTENT_TYPE,
            ])
            .max_age(3600);
        
        App::new()
            .wrap(cors)
            // Phase 6: Metrics middleware (always on)
            .wrap(middleware::metrics::MetricsMiddleware::new(
                service_metrics.clone()
            ))
            // Phase 6: Auth middleware (skips /health, /metrics, /register, /login)
            .wrap(middleware::auth::AuthMiddleware::new(
                jwt_manager.clone()
            ))
            .app_data(web::Data::new(db_pool.clone()))
            .app_data(health_state.clone())
            .app_data(auth_state.clone())
            .app_data(registry_data.clone())
            // Phase 6: Health endpoints (no auth required)
            .route("/health", web::get().to(handlers::health::health_check))
            .route("/liveness", web::get().to(handlers::health::liveness_probe))
            .route("/readiness", web::get().to(handlers::health::readiness_probe))
            // Phase 6: Metrics endpoint (no auth required)
            .route("/metrics", web::get().to(handlers::metrics::metrics_handler))
            // Phase 6: Auth endpoints (no auth required)
            .route("/register", web::post().to(handlers::auth::register))
            .route("/login", web::post().to(handlers::auth::login))
            .route("/refresh", web::post().to(handlers::auth::refresh_token))
            // EXISTING: Your working deposit endpoints (now with auth)
            .route("/deposits", web::post().to(create_deposit))
            .route("/balance/{paymail}", web::get().to(get_user_balance))
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}