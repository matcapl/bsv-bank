// core/lending-service/src/main.rs
// Lending Service with Phase 6 Production Hardening

use actix_web::{web, App, HttpResponse, HttpServer, Responder, Result, middleware};
use actix_cors::Cors;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc, Duration};
use uuid::Uuid;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use bsv_bank_common::{
    init_logging, ServiceMetrics,
    validate_paymail, validate_amount,
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
    #[error("Business logic error: {0}")]
    BusinessError(String),
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
            ServiceError::BusinessError(msg) => {
                HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "business_error",
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
pub struct LoanRequest {
    pub borrower_paymail: String,
    pub amount_satoshis: i64,
    pub collateral_satoshis: i64,
    pub duration_days: i32,
    pub interest_rate_bps: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoanResponse {
    pub loan_id: Uuid,
    pub status: String,
    pub collateral_ratio: f64,
    pub total_repayment_satoshis: i64,
    pub interest_satoshis: i64,
    pub due_date: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RepaymentRequest {
    pub borrower_paymail: String,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct LoanHistory {
    pub id: String,
    pub borrower_paymail: String,
    pub lender_paymail: Option<String>,
    pub amount_satoshis: i64,
    pub collateral_satoshis: i64,
    pub interest_rate: f64,
    pub duration_days: i32,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub funded_at: Option<DateTime<Utc>>,
    pub repaid_at: Option<DateTime<Utc>>,
    pub liquidated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct LoanStats {
    pub total_borrowed: i64,
    pub total_lent: i64,
    pub active_borrowed_count: i64,
    pub active_lent_count: i64,
    pub pending_count: i64,
    pub repaid_count: i64,
    pub liquidated_count: i64,
    pub total_interest_earned: i64,
    pub total_interest_paid: i64,
}

struct AppState {
    db_pool: PgPool,
    start_time: SystemTime,
}

// ============================================================================
// BUSINESS LOGIC HELPERS
// ============================================================================

fn calculate_collateral_ratio(collateral: i64, principal: i64) -> f64 {
    if principal == 0 {
        return 0.0;
    }
    collateral as f64 / principal as f64
}

fn bps_to_rate(bps: i32) -> f64 {
    bps as f64 / 10000.0
}

fn validate_loan_request(request: &LoanRequest) -> Result<(), ServiceError> {
    // Phase 6: Validate paymail
    validate_paymail(&request.borrower_paymail)
        .map_err(|e| ServiceError::ValidationError(e.to_string()))?;
    
    // Phase 6: Validate amounts
    validate_amount(request.amount_satoshis)
        .map_err(|e| ServiceError::ValidationError(e.to_string()))?;
    validate_amount(request.collateral_satoshis)
        .map_err(|e| ServiceError::ValidationError(e.to_string()))?;
    
    // Validate duration
    if request.duration_days < 1 || request.duration_days > 365 {
        return Err(ServiceError::ValidationError(
            "Duration must be between 1 and 365 days".to_string()
        ));
    }
    
    // Validate interest rate (0 to 100% APR = 0-10000 bps)
    if request.interest_rate_bps < 0 || request.interest_rate_bps > 10000 {
        return Err(ServiceError::ValidationError(
            "Interest rate must be between 0 and 10000 bps (0-100% APR)".to_string()
        ));
    }
    
    Ok(())
}

// ============================================================================
// HANDLERS
// ============================================================================

async fn create_loan_request(
    pool: web::Data<PgPool>,
    request: web::Json<LoanRequest>,
) -> Result<HttpResponse, ServiceError> {
    // Phase 6: Validate all inputs
    validate_loan_request(&request)?;
    
    let collateral_ratio = calculate_collateral_ratio(
        request.collateral_satoshis,
        request.amount_satoshis
    );
    
    if collateral_ratio < 1.5 {
        return Err(ServiceError::BusinessError(format!(
            "Insufficient collateral. Minimum 150% required. Required: {}, Provided: {}",
            (request.amount_satoshis as f64 * 1.5) as i64,
            request.collateral_satoshis
        )));
    }
    
    let loan_id = Uuid::new_v4();
    let now = Utc::now();
    let due_date = now + Duration::days(request.duration_days as i64);
    let annual_rate = bps_to_rate(request.interest_rate_bps);
    let daily_rate = annual_rate / 365.0;
    let total_interest = (request.amount_satoshis as f64
        * daily_rate
        * request.duration_days as f64) as i64;
    
    let result = sqlx::query!(
        r#"
        INSERT INTO loans (
            id, borrower_paymail, lender_paymail, principal_satoshis,
            collateral_satoshis, interest_rate_bps, interest_accrued,
            status, created_at, due_date
        )
        VALUES ($1, $2, NULL, $3, $4, $5, $6, $7, $8, $9)
        RETURNING id
        "#,
        loan_id,
        request.borrower_paymail,
        request.amount_satoshis,
        request.collateral_satoshis,
        request.interest_rate_bps,
        total_interest,
        "Pending",
        now,
        due_date
    )
    .fetch_one(pool.as_ref())
    .await
    .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;
    
    tracing::info!("Loan created: {} for {}", loan_id, request.borrower_paymail);
    
    Ok(HttpResponse::Ok().json(LoanResponse {
        loan_id,
        status: "Pending".to_string(),
        collateral_ratio,
        total_repayment_satoshis: request.amount_satoshis + total_interest,
        interest_satoshis: total_interest,
        due_date,
    }))
}

async fn get_available_loans(pool: web::Data<PgPool>) -> Result<HttpResponse, ServiceError> {
    let result = sqlx::query!(
        r#"
        SELECT
            id, borrower_paymail, principal_satoshis,
            collateral_satoshis, interest_rate_bps, due_date
        FROM loans
        WHERE status = 'Pending'
        ORDER BY created_at DESC
        LIMIT 50
        "#,
    )
    .fetch_all(pool.as_ref())
    .await
    .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;
    
    let loan_list: Vec<_> = result.iter().map(|loan| {
        serde_json::json!({
            "loan_id": loan.id,
            "borrower": loan.borrower_paymail,
            "amount": loan.principal_satoshis,
            "collateral": loan.collateral_satoshis,
            "collateral_ratio": calculate_collateral_ratio(
                loan.collateral_satoshis,
                loan.principal_satoshis
            ),
            "interest_rate_percent": bps_to_rate(loan.interest_rate_bps) * 100.0,
            "due_date": loan.due_date
        })
    }).collect();
    
    Ok(HttpResponse::Ok().json(loan_list))
}

async fn get_user_loans(
    pool: web::Data<PgPool>,
    paymail: web::Path<String>,
) -> Result<HttpResponse, ServiceError> {
    // Phase 6: Validate paymail
    validate_paymail(&paymail)
        .map_err(|e| ServiceError::ValidationError(e.to_string()))?;
    
    let result = sqlx::query!(
        r#"
        SELECT
            id, borrower_paymail, lender_paymail, principal_satoshis,
            collateral_satoshis, interest_rate_bps, interest_accrued,
            status, created_at, due_date, repaid_at
        FROM loans
        WHERE borrower_paymail = $1 OR lender_paymail = $1
        ORDER BY created_at DESC
        "#,
        paymail.as_str()
    )
    .fetch_all(pool.as_ref())
    .await
    .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;
    
    let loan_list: Vec<_> = result.iter().map(|loan| {
        let total_due = loan.principal_satoshis + loan.interest_accrued;
        serde_json::json!({
            "loan_id": loan.id,
            "borrower": loan.borrower_paymail,
            "lender": loan.lender_paymail,
            "principal": loan.principal_satoshis,
            "interest": loan.interest_accrued,
            "total_due": total_due,
            "collateral": loan.collateral_satoshis,
            "status": loan.status,
            "created_at": loan.created_at,
            "due_date": loan.due_date,
            "repaid_at": loan.repaid_at
        })
    }).collect();
    
    Ok(HttpResponse::Ok().json(loan_list))
}

async fn fund_loan(
    pool: web::Data<PgPool>,
    loan_id: web::Path<Uuid>,
    lender: web::Json<serde_json::Value>,
) -> Result<HttpResponse, ServiceError> {
    let lender_paymail = lender.get("lender_paymail")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ServiceError::ValidationError("lender_paymail required".to_string()))?;
    
    // Phase 6: Validate lender paymail
    validate_paymail(lender_paymail)
        .map_err(|e| ServiceError::ValidationError(e.to_string()))?;
    
    let result = sqlx::query!(
        r#"
        UPDATE loans
        SET lender_paymail = $1, status = 'Active'
        WHERE id = $2 AND status = 'Pending'
        RETURNING id
        "#,
        lender_paymail,
        loan_id.as_ref()
    )
    .fetch_optional(pool.as_ref())
    .await
    .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;
    
    match result {
        Some(_) => {
            tracing::info!("Loan {} funded by {}", loan_id, lender_paymail);
            Ok(HttpResponse::Ok().json(serde_json::json!({
                "status": "success",
                "message": "Loan funded successfully",
                "loan_id": loan_id.as_ref()
            })))
        }
        None => Err(ServiceError::BusinessError("Loan not found or already funded".to_string()))
    }
}

async fn repay_loan(
    pool: web::Data<PgPool>,
    loan_id: web::Path<Uuid>,
    request: web::Json<RepaymentRequest>,
) -> Result<HttpResponse, ServiceError> {
    // Phase 6: Validate borrower paymail
    validate_paymail(&request.borrower_paymail)
        .map_err(|e| ServiceError::ValidationError(e.to_string()))?;
    
    // Get loan details
    let loan = sqlx::query!(
        r#"
        SELECT
            borrower_paymail, lender_paymail, principal_satoshis,
            interest_accrued, collateral_satoshis, status, due_date
        FROM loans
        WHERE id = $1
        "#,
        loan_id.as_ref()
    )
    .fetch_optional(pool.as_ref())
    .await
    .map_err(|e| ServiceError::DatabaseError(e.to_string()))?
    .ok_or_else(|| ServiceError::BusinessError("Loan not found".to_string()))?;
    
    // Verify borrower
    if loan.borrower_paymail != request.borrower_paymail {
        return Err(ServiceError::BusinessError("Only the borrower can repay this loan".to_string()));
    }
    
    // Check if loan is active
    if loan.status != "Active" {
        return Err(ServiceError::BusinessError(format!("Loan is not active (status: {})", loan.status)));
    }
    
    let total_due = loan.principal_satoshis + loan.interest_accrued;
    let now = Utc::now();
    
    // Calculate late fee if overdue
    let late_fee = if now > loan.due_date {
        let days_late = (now - loan.due_date).num_days();
        (loan.principal_satoshis as f64 * 0.01 * days_late as f64) as i64 // 1% per day
    } else {
        0
    };
    
    let total_with_fees = total_due + late_fee;
    
    // Update loan status
    sqlx::query!(
        r#"
        UPDATE loans
        SET status = 'Repaid', repaid_at = $1
        WHERE id = $2
        RETURNING id
        "#,
        now,
        loan_id.as_ref()
    )
    .fetch_one(pool.as_ref())
    .await
    .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;
    
    tracing::info!("Loan {} repaid by {}", loan_id, request.borrower_paymail);
    
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "status": "success",
        "message": "Loan repaid successfully",
        "principal": loan.principal_satoshis,
        "interest": loan.interest_accrued,
        "late_fee": late_fee,
        "total_paid": total_with_fees,
        "collateral_released": loan.collateral_satoshis,
        "repaid_at": now
    })))
}

async fn check_liquidations(pool: web::Data<PgPool>) -> Result<HttpResponse, ServiceError> {
    let now = Utc::now();
    
    // Find overdue loans
    let overdue = sqlx::query!(
        r#"
        SELECT
            id, borrower_paymail, lender_paymail, principal_satoshis,
            collateral_satoshis, due_date
        FROM loans
        WHERE status = 'Active' AND due_date < $1
        "#,
        now
    )
    .fetch_all(pool.as_ref())
    .await
    .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;
    
    let mut liquidated = Vec::new();
    
    for loan in overdue {
        let days_overdue = (now - loan.due_date).num_days();
        
        // Liquidate if more than 7 days overdue
        if days_overdue > 7 {
            let result = sqlx::query!(
                r#"
                UPDATE loans
                SET status = 'Liquidated', liquidated_at = $1
                WHERE id = $2
                RETURNING id
                "#,
                now,
                loan.id
            )
            .fetch_optional(pool.as_ref())
            .await;
            
            if result.is_ok() {
                tracing::warn!("Loan {} liquidated - {} days overdue", loan.id, days_overdue);
                liquidated.push(serde_json::json!({
                    "loan_id": loan.id,
                    "borrower": loan.borrower_paymail,
                    "lender": loan.lender_paymail,
                    "collateral_seized": loan.collateral_satoshis,
                    "days_overdue": days_overdue
                }));
            }
        }
    }
    
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "checked_at": now,
        "liquidated_count": liquidated.len(),
        "liquidations": liquidated
    })))
}

// Get all loans for a borrower
#[actix_web::get("/loans/borrower/{paymail}")]
async fn get_borrower_loans(
    pool: web::Data<PgPool>,
    paymail: web::Path<String>,
) -> Result<HttpResponse> {
    // Phase 6: Validate paymail
    if let Err(e) = validate_paymail(&paymail) {
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "validation_error",
            "message": e.to_string()
        })));
    }
    
    let loans = sqlx::query_as::<_, LoanHistory>(
        r#"
        SELECT id, borrower_paymail, lender_paymail, amount_satoshis, collateral_satoshis, 
               interest_rate, duration_days, status, created_at, funded_at, repaid_at, liquidated_at
        FROM loans
        WHERE borrower_paymail = $1
        ORDER BY created_at DESC
        "#
    )
    .bind(paymail.as_str())
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| {
        tracing::error!("Database error fetching borrower loans: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to fetch loans")
    })?;
    
    Ok(HttpResponse::Ok().json(loans))
}

// Get all loans funded by a lender
#[actix_web::get("/loans/lender/{paymail}")]
async fn get_lender_loans(
    pool: web::Data<PgPool>,
    paymail: web::Path<String>,
) -> Result<HttpResponse> {
    // Phase 6: Validate paymail
    if let Err(e) = validate_paymail(&paymail) {
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "validation_error",
            "message": e.to_string()
        })));
    }
    
    let loans = sqlx::query_as::<_, LoanHistory>(
        r#"
        SELECT id, borrower_paymail, lender_paymail, amount_satoshis, collateral_satoshis, 
               interest_rate, duration_days, status, created_at, funded_at, repaid_at, liquidated_at
        FROM loans
        WHERE lender_paymail = $1
        ORDER BY created_at DESC
        "#
    )
    .bind(paymail.as_str())
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| {
        tracing::error!("Database error fetching lender loans: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to fetch loans")
    })?;
    
    Ok(HttpResponse::Ok().json(loans))
}

// Get detailed loan statistics for a user
#[actix_web::get("/loans/stats/{paymail}")]
async fn get_loan_stats(
    pool: web::Data<PgPool>,
    paymail: web::Path<String>,
) -> Result<HttpResponse> {
    // Phase 6: Validate paymail
    if let Err(e) = validate_paymail(&paymail) {
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "validation_error",
            "message": e.to_string()
        })));
    }
    
    let stats = sqlx::query_as::<_, (i64, i64, i64, i64, i64, i64, i64, i64, i64)>(
        r#"
        SELECT
            COALESCE(SUM(CASE WHEN borrower_paymail = $1 THEN amount_satoshis ELSE 0 END), 0) as total_borrowed,
            COALESCE(SUM(CASE WHEN lender_paymail = $1 THEN amount_satoshis ELSE 0 END), 0) as total_lent,
            COALESCE(COUNT(CASE WHEN borrower_paymail = $1 AND status = 'Active' THEN 1 END), 0) as active_borrowed_count,
            COALESCE(COUNT(CASE WHEN lender_paymail = $1 AND status = 'Active' THEN 1 END), 0) as active_lent_count,
            COALESCE(COUNT(CASE WHEN status = 'Pending' THEN 1 END), 0) as pending_count,
            COALESCE(COUNT(CASE WHEN status = 'Repaid' THEN 1 END), 0) as repaid_count,
            COALESCE(COUNT(CASE WHEN status = 'Liquidated' THEN 1 END), 0) as liquidated_count,
            COALESCE(SUM(CASE WHEN lender_paymail = $1 AND status IN ('Repaid', 'Active') 
                THEN CAST(amount_satoshis * interest_rate AS BIGINT) ELSE 0 END), 0) as total_interest_earned,
            COALESCE(SUM(CASE WHEN borrower_paymail = $1 AND status = 'Repaid' 
                THEN CAST(amount_satoshis * interest_rate AS BIGINT) ELSE 0 END), 0) as total_interest_paid
        FROM loans
        WHERE borrower_paymail = $1 OR lender_paymail = $1
        "#
    )
    .bind(paymail.as_str())
    .fetch_one(pool.get_ref())
    .await
    .map_err(|e| {
        tracing::error!("Database error fetching loan stats: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to fetch statistics")
    })?;
    
    let stats = LoanStats {
        total_borrowed: stats.0,
        total_lent: stats.1,
        active_borrowed_count: stats.2,
        active_lent_count: stats.3,
        pending_count: stats.4,
        repaid_count: stats.5,
        liquidated_count: stats.6,
        total_interest_earned: stats.7,
        total_interest_paid: stats.8,
    };
    
    Ok(HttpResponse::Ok().json(stats))
}

pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg
        .service(get_borrower_loans)
        .service(get_lender_loans)
        .service(get_loan_stats);
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
        "service": "lending-service",
        "status": "healthy",
        "version": "0.2.0",
        "features": ["repayment", "liquidation", "phase6-validation"],
        "uptime_seconds": uptime
    }))
}

async fn liveness_check() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({"status": "alive"}))
}

async fn readiness_check(data: web::Data<AppState>) -> impl Responder {
    // Check database connection
    let db_ok = sqlx::query("SELECT 1")
        .fetch_optional(&data.db_pool)
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
    
    println!("🤝 BSV Bank - Lending Service Starting (Phase 6)...");
    
    // Get configuration
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| {
            println!("⚠️  DATABASE_URL not set, using default");
            "postgres://postgres:postgres@localhost:5432/bsv_bank".to_string()
        });
    
    let port: u16 = 8082; // Fixed port for lending-service
    
    // Phase 6: Initialize structured logging
    init_logging("lending-service");
    tracing::info!("Starting Lending Service on port {}", port);
    
    // Database connection pool
    println!("📡 Connecting to database...");
    let db_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to connect to database");
    
    println!("✅ Database connected");
    tracing::info!("Database connection established");
    
    // Phase 6: Prometheus metrics
    let registry = Registry::new();
    let _service_metrics = ServiceMetrics::new(&registry, "lending_service")
        .expect("Failed to create service metrics");
    let _lending_metrics = bsv_bank_common::LendingMetrics::new(&registry)
        .expect("Failed to create lending metrics");
    tracing::info!("Metrics initialized");
    
    // Application state
    let app_state = web::Data::new(AppState {
        db_pool: db_pool.clone(),
        start_time: SystemTime::now(),
    });
    
    let registry_data = web::Data::new(registry);
    
    println!("✅ Service ready on http://0.0.0.0:{}", port);
    println!("📋 Health: http://0.0.0.0:{}/health", port);
    println!("📊 Metrics: http://0.0.0.0:{}/metrics", port);
    println!("📋 Endpoints: /loans/request, /loans/available, /loans/{{id}}/fund, /loans/{{id}}/repay");
    tracing::info!("Starting HTTP server...");
    
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
            // Phase 6: Request logging
            .wrap(middleware::Logger::default())
            // Phase 6: Security headers
            .wrap(middleware::DefaultHeaders::new()
                .add(("X-Frame-Options", "DENY"))
                .add(("X-Content-Type-Options", "nosniff"))
                .add(("Content-Security-Policy", "default-src 'self'"))
                .add(("X-XSS-Protection", "1; mode=block"))
            )
            .app_data(web::Data::new(db_pool.clone()))
            .app_data(app_state.clone())
            .app_data(registry_data.clone())
            // Health endpoints (no auth)
            .route("/health", web::get().to(health_check))
            .route("/liveness", web::get().to(liveness_check))
            .route("/readiness", web::get().to(readiness_check))
            // Metrics endpoint (no auth)
            .route("/metrics", web::get().to(metrics_handler))
            // Business endpoints
            .route("/loans/request", web::post().to(create_loan_request))
            .route("/loans/available", web::get().to(get_available_loans))
            .route("/loans/my-loans/{paymail}", web::get().to(get_user_loans))
            .route("/loans/{id}/fund", web::post().to(fund_loan))
            .route("/loans/{id}/repay", web::post().to(repay_loan))
            .route("/loans/liquidations/check", web::post().to(check_liquidations))
            .configure(configure_routes)
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
