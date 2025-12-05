// core/deposit-service/src/main.rs
// Deposit Service with Phase 6 Production Hardening

mod database;
mod node_integration;
mod handlers {
    pub mod health;     // ✅ KEEP - uses common's health but adds service-specific checks
    pub mod auth;       // ✅ KEEP - uses common's auth but with local DB
    pub mod metrics;    // ✅ KEEP - exposes Prometheus endpoint
    // pub mod deposits;   // ✅ KEEP - deposit-specific business logic
}
mod middleware;

use actix_web::{web, App, HttpResponse, HttpServer};
use actix_cors::Cors;
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use chrono::Utc;
use uuid::Uuid;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use bsv_bank_common::{
    init_logging, JwtManager, RateLimit, RateLimiter, 
    RateLimitMiddleware, configure_rate_limits, ServiceMetrics,
    validate_paymail, validate_txid, validate_amount,
};
use dotenv::dotenv;
use prometheus::Registry;
use std::sync::Arc;
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
    #[error("Transaction verification failed: {0}")]
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
            ServiceError::VerificationError(msg) => {
                HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "verification_failed",
                    "message": msg
                }))
            }
        }
    }
}

// ============================================================================
// DATA TYPES
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
// HANDLERS (Business Logic Only - Validation via common)
// ============================================================================

async fn create_deposit(
    pool: web::Data<PgPool>,
    request: web::Json<DepositRequest>,
) -> Result<HttpResponse, ServiceError> {
    // Phase 6: Validate all inputs using common library
    validate_paymail(&request.user_paymail)
        .map_err(|e| ServiceError::ValidationError(e.to_string()))?;
    validate_txid(&request.txid)
        .map_err(|e| ServiceError::ValidationError(e.to_string()))?;
    validate_amount(request.amount_satoshis)
        .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

    // Verify transaction on BSV blockchain
    let verified = node_integration::verify_transaction_real(&request.txid)
        .await
        .map_err(|e| ServiceError::VerificationError(e))?;
    
    if !verified {
        return Err(ServiceError::VerificationError("Transaction not found or invalid".to_string()));
    }
    
    let confirmations = 6; // Assume confirmed for now
    
    // Get or create user
    let user_id = database::get_or_create_user(&pool, &request.user_paymail)
        .await
        .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;
    
    let deposit_id = Uuid::new_v4();
    let now = Utc::now();
    let status = if confirmations >= 6 { "Confirmed" } else { "Pending" };
    
    let lock_until = request.lock_duration_days.map(|days| {
        now + chrono::Duration::days(days as i64)
    });
    
    // Insert deposit
    sqlx::query!(
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
    .await
    .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;
    
    // Create on-chain commitment
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
    
    tracing::info!("Deposit created: {} for {} (commitment: 6a{})", deposit_id, request.user_paymail, hash);
    
    Ok(HttpResponse::Ok().json(DepositResponse {
        deposit_id: deposit_id.to_string(),
        status: status.to_string(),
        estimated_confirmation_time: "~60 seconds".to_string(),
    }))
}

async fn get_user_balance(
    pool: web::Data<PgPool>,
    paymail: web::Path<String>,
) -> Result<HttpResponse, ServiceError> {
    // Phase 6: Validate paymail
    validate_paymail(&paymail)
        .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

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
    .await
    .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;
    
    // let balance = result.unwrap_or_else(|| {
    //     // Return mock data if user doesn't exist yet
    //     sqlx::query!(
    //         "SELECT 0::BIGINT as balance_satoshis, 0::BIGINT as accrued_interest_satoshis, 0::BIGINT as active_deposits"
    //     ).fetch_one(pool.as_ref()).await.unwrap()
    // });
    
    // let bal = balance.balance_satoshis.unwrap_or(0);
    // let interest = balance.accrued_interest_satoshis.unwrap_or(0);
    
    // Handle Some/None directly
    let (bal, interest, active) = match result {
        Some(balance) => (
            balance.balance_satoshis.unwrap_or(0),
            balance.accrued_interest_satoshis.unwrap_or(0),
            balance.active_deposits.unwrap_or(0),
        ),
        None => (0, 0, 0),
    };
    
    Ok(HttpResponse::Ok().json(BalanceResponse {
        paymail: paymail.to_string(),
        balance_satoshis: bal,
        accrued_interest_satoshis: interest,
        total_available_satoshis: bal + interest,
        current_apy: 7.0,
        active_deposits: active,
    }))
}

// ============================================================================
// MAIN
// ============================================================================

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    
    println!("🏦 BSV Bank - Deposit Service Starting (Phase 6)...");
    
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
    
    let port: u16 = 8080;
    
    // Phase 6: Initialize structured logging
    init_logging("deposit-service");
    tracing::info!("Starting Deposit Service on port {}", port);
    
    // Database connection
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
    // let service_metrics = ServiceMetrics::new(&registry, "deposit_service")
    let _service_metrics = ServiceMetrics::new(&registry, "deposit_service")
        .expect("Failed to create service metrics");
    let _deposit_metrics = bsv_bank_common::DepositMetrics::new(&registry)
        .expect("Failed to create deposit metrics");
    tracing::info!("Metrics initialized");
    
    // Phase 6: Rate limiter
    let mut rate_limiter = RateLimiter::new();
    rate_limiter.add_limit("deposits".to_string(), RateLimit::per_minute(100));
    rate_limiter.add_limit("withdrawals".to_string(), RateLimit::per_minute(50));
    rate_limiter.add_limit("auth".to_string(), RateLimit::per_minute(10));
    configure_rate_limits(&mut rate_limiter);
    
    let rate_limiter = Arc::new(rate_limiter);
    bsv_bank_common::rate_limit::start_cleanup_task(rate_limiter.clone());
    tracing::info!("Rate limiter initialized");
    
    // Application state for health checks
    let start_time = SystemTime::now();
    let health_state = web::Data::new(handlers::health::AppState {
        db_pool: db_pool.clone(),
        start_time,
    });
    
    // Application state for auth
    let auth_state = web::Data::new(handlers::auth::AuthState {
        db_pool: db_pool.clone(),
        jwt_manager: jwt_manager.clone(),
    });
    
    let registry_data = web::Data::new(registry);
    
    println!("✅ Service ready on http://0.0.0.0:{}", port);
    println!("📋 Health: http://0.0.0.0:{}/health", port);
    println!("📊 Metrics: http://0.0.0.0:{}/metrics", port);
    tracing::info!("Starting HTTP server...");
    
    HttpServer::new(move || {
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
            // Phase 6: Common middleware from library
            .wrap(RateLimitMiddleware::new(rate_limiter.clone()))
            // // Phase 6: Metrics middleware
            // .wrap(middleware::metrics::MetricsMiddleware::new(service_metrics.clone()))
            // Phase 6: Auth middleware
            .wrap(middleware::auth::AuthMiddleware::new(jwt_manager.clone()))
            // Add metrics middleware if you have it
            // .wrap(bsv_bank_common::MetricsMiddleware::new(service_metrics.clone()))
            // Phase 6: Security headers
            .wrap(actix_web::middleware::DefaultHeaders::new()
                .add(("X-Frame-Options", "DENY"))
                .add(("X-Content-Type-Options", "nosniff"))
                .add(("Content-Security-Policy", "default-src 'self'"))
                .add(("X-XSS-Protection", "1; mode=block"))
            )
            // Phase 6: Request logging
            .wrap(actix_web::middleware::Logger::default())
            .app_data(web::Data::new(db_pool.clone()))
            .app_data(health_state.clone())
            .app_data(auth_state.clone())
            .app_data(registry_data.clone())
            // Health endpoints (no auth)
            .route("/health", web::get().to(handlers::health::health_check))
            .route("/liveness", web::get().to(handlers::health::liveness_probe))
            .route("/readiness", web::get().to(handlers::health::readiness_probe))
            // Metrics endpoint (no auth)
            .route("/metrics", web::get().to(handlers::metrics::metrics_handler))
            // Auth endpoints (no auth)
            .route("/register", web::post().to(handlers::auth::register))
            .route("/login", web::post().to(handlers::auth::login))
            .route("/refresh", web::post().to(handlers::auth::refresh_token))
            // Business endpoints (with auth)
            .route("/deposits", web::post().to(create_deposit))
            .route("/balance/{paymail}", web::get().to(get_user_balance))
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}