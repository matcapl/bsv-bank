// core/interest-engine/src/main.rs
// Interest Engine with Phase 6 Production Hardening (Minimal Edition)

use actix_web::{web, App, HttpResponse, HttpServer, Responder, middleware};
use actix_cors::Cors;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use sha2::{Sha256, Digest};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use bsv_bank_common::{
    init_logging, ServiceMetrics,
    validate_paymail, // Import validators we actually use
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
    #[error("Internal server error")]
    InternalError,
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
            ServiceError::InternalError => {
                HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": "internal_error"
                }))
            }
        }
    }
}

// ============================================================================
// DATA TYPES
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InterestRate {
    timestamp: DateTime<Utc>,
    utilization_rate: f64,
    borrow_apy: f64,
    supply_apy: f64,
    total_deposits: u64,
    total_borrowed: u64,
}

struct AppState {
    rates: Arc<Mutex<Vec<InterestRate>>>,
    user_accruals: Arc<Mutex<HashMap<String, u64>>>,
    db_pool: PgPool,
    start_time: SystemTime,
}

#[derive(Debug, Deserialize)]
struct DistributeQuery {
    paymail: Option<String>, // Optional: distribute to specific user
}

// ============================================================================
// BUSINESS LOGIC
// ============================================================================

fn calculate_rates(total_deposits: u64, total_borrowed: u64) -> (f64, f64) {
    let utilization = if total_deposits == 0 {
        0.0
    } else {
        total_borrowed as f64 / total_deposits as f64
    };
    
    let base_rate = 0.02;
    let optimal_util = 0.80;
    
    let borrow_apy = if utilization <= optimal_util {
        base_rate + (utilization * 0.10)
    } else {
        base_rate + (optimal_util * 0.10) + ((utilization - optimal_util) * 1.00)
    };
    
    let supply_apy = borrow_apy * utilization * 0.90; // 90% goes to suppliers
    
    (borrow_apy, supply_apy)
}

// ============================================================================
// HANDLERS
// ============================================================================

async fn get_current_rates(data: web::Data<AppState>) -> impl Responder {
    // TODO: Query real totals from database instead of hardcoded values
    let total_deposits = 10000000u64;
    let total_borrowed = 7000000u64;
    
    let (borrow_apy, supply_apy) = calculate_rates(total_deposits, total_borrowed);
    
    let rate = InterestRate {
        timestamp: Utc::now(),
        utilization_rate: total_borrowed as f64 / total_deposits as f64,
        borrow_apy,
        supply_apy,
        total_deposits,
        total_borrowed,
    };
    
    let mut rates = data.rates.lock().unwrap();
    rates.push(rate.clone());
    
    // Create OP_RETURN commitment for BSV blockchain
    let commitment_data = format!(
        "RATE|{}|{}|{}",
        rate.utilization_rate,
        rate.borrow_apy,
        rate.timestamp.timestamp()
    );
    let mut hasher = Sha256::new();
    hasher.update(commitment_data.as_bytes());
    let hash = hex::encode(hasher.finalize());
    
    tracing::info!("Interest rate commitment: 6a{}", hash);
    
    HttpResponse::Ok().json(rate)
}

async fn distribute_interest(
    data: web::Data<AppState>,
    query: web::Query<DistributeQuery>,
) -> impl Responder {
    // Phase 6: Validate paymail if provided (using if-let like deposit-service)
    if let Some(ref paymail) = query.paymail {
        if let Err(e) = validate_paymail(paymail) {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid paymail",
                "error_code": "validation_error",
                "message": e.to_string()
            }));
        }
    }
    
    tracing::info!("Running interest distribution...");
    
    // TODO: Query actual user balances from deposit service database
    // For now, simulate with test data
    let users = vec![
        ("test@handcash.io", 100000u64),
        ("user@handcash.io", 500000u64),
    ];
    
    let daily_rate = 0.07 / 365.0; // 7% APY divided by days
    
    let mut accruals = data.user_accruals.lock().unwrap();
    
    for (paymail, balance) in users {
        // If specific paymail requested, only process that one
        if let Some(ref filter_paymail) = query.paymail {
            if paymail != filter_paymail.as_str() {
                continue;
            }
        }
        
        let interest = (balance as f64 * daily_rate) as u64;
        *accruals.entry(paymail.to_string()).or_insert(0) += interest;
        tracing::info!("  {} earned {} satoshis", paymail, interest);
    }
    
    HttpResponse::Ok().json(serde_json::json!({
        "status": "success",
        "distributed_at": Utc::now(),
        "filter": query.paymail
    }))
}

async fn get_accrued_interest(
    data: web::Data<AppState>,
    paymail: web::Path<String>,
) -> impl Responder {
    // Phase 6: Validate paymail (using if-let like deposit-service)
    if let Err(e) = validate_paymail(&paymail) {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Invalid paymail",
            "error_code": "validation_error",
            "message": e.to_string()
        }));
    }
    
    let accruals = data.user_accruals.lock().unwrap();
    let interest = accruals.get(paymail.as_str()).copied().unwrap_or(0);
    
    HttpResponse::Ok().json(serde_json::json!({
        "paymail": paymail.as_str(),
        "accrued_interest_satoshis": interest,
        "timestamp": Utc::now()
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
        "service": "interest-engine",
        "status": "healthy",
        "version": "0.1.0",
        "timestamp": Utc::now(),
        "uptime_seconds": uptime
    }))
}

async fn liveness_check() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({"status": "alive"}))
}

async fn readiness_check(data: web::Data<AppState>) -> impl Responder {
    // Check if we can acquire locks (basic readiness)
    let rates_accessible = data.rates.try_lock().is_ok();
    let accruals_accessible = data.user_accruals.try_lock().is_ok();
    
    // Check database connection
    let db_ok = sqlx::query("SELECT 1")
        .fetch_optional(&data.db_pool)
        .await
        .is_ok();
    
    if rates_accessible && accruals_accessible && db_ok {
        HttpResponse::Ok().json(serde_json::json!({
            "status": "ready",
            "checks": {
                "rates_store": "ok",
                "accruals_store": "ok",
                "database": "ok"
            }
        }))
    } else {
        HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "status": "not_ready",
            "checks": {
                "rates_store": if rates_accessible { "ok" } else { "locked" },
                "accruals_store": if accruals_accessible { "ok" } else { "locked" },
                "database": if db_ok { "ok" } else { "error" }
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
    
    println!("📊 BSV Bank - Interest Engine Starting (Phase 6)...");
    
    // Get configuration
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| {
            println!("⚠️  DATABASE_URL not set, using default");
            "postgres://postgres:postgres@localhost:5432/bsv_bank".to_string()
        });
    
    let port: u16 = 8081; // Fixed port for interest-engine
    
    // Phase 6: Initialize structured logging
    init_logging("interest-engine");
    tracing::info!("Starting Interest Engine on port {}", port);
    
    // Database connection pool
    println!("📡 Connecting to database...");
    let db_pool = PgPoolOptions::new()
        .max_connections(5) // Lower than deposit service, less traffic
        .connect(&database_url)
        .await
        .expect("Failed to connect to database");
    
    println!("✅ Database connected");
    tracing::info!("Database connection established");
    
    // Phase 6: Prometheus metrics
    let registry = Registry::new();
    let _service_metrics = ServiceMetrics::new(&registry, "interest_engine")
        .expect("Failed to create service metrics");
    tracing::info!("Metrics initialized");
    
    // Application state
    let app_state = web::Data::new(AppState {
        rates: Arc::new(Mutex::new(Vec::new())),
        user_accruals: Arc::new(Mutex::new(HashMap::new())),
        db_pool: db_pool.clone(),
        start_time: SystemTime::now(),
    });
    
    let registry_data = web::Data::new(registry);
    
    println!("✅ Service ready on http://0.0.0.0:{}", port);
    println!("📋 Health: http://0.0.0.0:{}/health", port);
    println!("📊 Metrics: http://0.0.0.0:{}/metrics", port);
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
            .app_data(app_state.clone())
            .app_data(registry_data.clone())
            // Health endpoints (no auth)
            .route("/health", web::get().to(health_check))
            .route("/liveness", web::get().to(liveness_check))
            .route("/readiness", web::get().to(readiness_check))
            // Metrics endpoint (no auth)
            .route("/metrics", web::get().to(metrics_handler))
            // Business endpoints
            .route("/rates/current", web::get().to(get_current_rates))
            .route("/interest/distribute", web::post().to(distribute_interest))
            .route("/interest/{paymail}", web::get().to(get_accrued_interest))
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}