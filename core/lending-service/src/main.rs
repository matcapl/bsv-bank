use actix_web::{web, App, HttpResponse, HttpServer, Responder, Result};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc, Duration};
use uuid::Uuid;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

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

async fn create_pool() -> Result<PgPool, sqlx::Error> {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://a:@localhost:5432/bsv_bank".to_string());
    PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
}

fn calculate_collateral_ratio(collateral: i64, principal: i64) -> f64 {
    if principal == 0 {
        return 0.0;
    }
    collateral as f64 / principal as f64
}

fn bps_to_rate(bps: i32) -> f64 {
    bps as f64 / 10000.0
}

async fn create_loan_request(
    pool: web::Data<PgPool>,
    request: web::Json<LoanRequest>,
) -> impl Responder {
    let collateral_ratio = calculate_collateral_ratio(
        request.collateral_satoshis,
        request.amount_satoshis
    );
    if collateral_ratio < 1.5 {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Insufficient collateral. Minimum 150% required",
            "required": (request.amount_satoshis as f64 * 1.5) as i64,
            "provided": request.collateral_satoshis
        }));
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
    .await;
    match result {
        Ok(_) => {
            HttpResponse::Ok().json(LoanResponse {
                loan_id,
                status: "Pending".to_string(),
                collateral_ratio,
                total_repayment_satoshis: request.amount_satoshis + total_interest,
                interest_satoshis: total_interest,
                due_date,
            })
        }
        Err(e) => {
            eprintln!("Failed to create loan: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to create loan"
            }))
        }
    }
}

async fn get_available_loans(pool: web::Data<PgPool>) -> impl Responder {
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
    .await;
    match result {
        Ok(loans) => {
            let loan_list: Vec<_> = loans.iter().map(|loan| {
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
            HttpResponse::Ok().json(loan_list)
        }
        Err(e) => {
            eprintln!("Database error: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to fetch loans"
            }))
        }
    }
}

async fn get_user_loans(
    pool: web::Data<PgPool>,
    paymail: web::Path<String>,
) -> impl Responder {
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
    .await;
    match result {
        Ok(loans) => {
            let loan_list: Vec<_> = loans.iter().map(|loan| {
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
            HttpResponse::Ok().json(loan_list)
        }
        Err(e) => {
            eprintln!("Database error: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to fetch user loans"
            }))
        }
    }
}

async fn fund_loan(
    pool: web::Data<PgPool>,
    loan_id: web::Path<Uuid>,
    lender: web::Json<serde_json::Value>,
) -> impl Responder {
    let lender_paymail = lender.get("lender_paymail")
        .and_then(|v| v.as_str())
        .unwrap_or("");
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
    .await;
    match result {
        Ok(Some(_)) => {
            HttpResponse::Ok().json(serde_json::json!({
                "status": "success",
                "message": "Loan funded successfully",
                "loan_id": loan_id.as_ref()
            }))
        }
        Ok(None) => {
            HttpResponse::NotFound().json(serde_json::json!({
                "error": "Loan not found or already funded"
            }))
        }
        Err(e) => {
            eprintln!("Database error: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to fund loan"
            }))
        }
    }
}

async fn repay_loan(
    pool: web::Data<PgPool>,
    loan_id: web::Path<Uuid>,
    request: web::Json<RepaymentRequest>,
) -> impl Responder {
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
    .await;
    match loan {
        Ok(Some(loan)) => {
            // Verify borrower
            if loan.borrower_paymail != request.borrower_paymail {
                return HttpResponse::Forbidden().json(serde_json::json!({
                    "error": "Only the borrower can repay this loan"
                }));
            }
            // Check if loan is active
            if loan.status != "Active" {
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "error": format!("Loan is not active (status: {})", loan.status)
                }));
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
            let result = sqlx::query!(
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
            .await;
            match result {
                Ok(_) => {
                    HttpResponse::Ok().json(serde_json::json!({
                        "status": "success",
                        "message": "Loan repaid successfully",
                        "principal": loan.principal_satoshis,
                        "interest": loan.interest_accrued,
                        "late_fee": late_fee,
                        "total_paid": total_with_fees,
                        "collateral_released": loan.collateral_satoshis,
                        "repaid_at": now
                    }))
                }
                Err(e) => {
                    eprintln!("Failed to update loan: {}", e);
                    HttpResponse::InternalServerError().json(serde_json::json!({
                        "error": "Failed to process repayment"
                    }))
                }
            }
        }
        Ok(None) => {
            HttpResponse::NotFound().json(serde_json::json!({
                "error": "Loan not found"
            }))
        }
        Err(e) => {
            eprintln!("Database error: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to fetch loan"
            }))
        }
    }
}

async fn check_liquidations(pool: web::Data<PgPool>) -> impl Responder {
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
    .await;
    match overdue {
        Ok(loans) => {
            let mut liquidated = Vec::new();
            for loan in loans {
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
            HttpResponse::Ok().json(serde_json::json!({
                "checked_at": now,
                "liquidated_count": liquidated.len(),
                "liquidations": liquidated
            }))
        }
        Err(e) => {
            eprintln!("Database error: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to check liquidations"
            }))
        }
    }
}

// Get all loans for a borrower
#[actix_web::get("/loans/borrower/{paymail}")]
async fn get_borrower_loans(
    pool: web::Data<PgPool>,
    paymail: web::Path<String>,
) -> Result<HttpResponse> {
    let loans = sqlx::query_as::<_, LoanHistory>(
        r#"
        SELECT id, borrower_paymail, lender_paymail, amount_satoshis, collateral_satoshis, interest_rate, duration_days, status, created_at, funded_at, repaid_at, liquidated_at
        FROM loans
        WHERE borrower_paymail = $1
        ORDER BY created_at DESC
        "#
    )
    .bind(paymail.as_str())
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| {
        eprintln!("Database error fetching borrower loans: {}", e);
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
    let loans = sqlx::query_as::<_, LoanHistory>(
        r#"
        SELECT id, borrower_paymail, lender_paymail, amount_satoshis, collateral_satoshis, interest_rate, duration_days, status, created_at, funded_at, repaid_at, liquidated_at
        FROM loans
        WHERE lender_paymail = $1
        ORDER BY created_at DESC
        "#
    )
    .bind(paymail.as_str())
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| {
        eprintln!("Database error fetching lender loans: {}", e);
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
            COALESCE(SUM(CASE WHEN lender_paymail = $1 AND status IN ('Repaid', 'Active') THEN CAST(amount_satoshis * interest_rate AS BIGINT) ELSE 0 END), 0) as total_interest_earned,
            COALESCE(SUM(CASE WHEN borrower_paymail = $1 AND status = 'Repaid' THEN CAST(amount_satoshis * interest_rate AS BIGINT) ELSE 0 END), 0) as total_interest_paid
        FROM loans
        WHERE borrower_paymail = $1 OR lender_paymail = $1
        "#
    )
    .bind(paymail.as_str())
    .fetch_one(pool.get_ref())
    .await
    .map_err(|e| {
        eprintln!("Database error fetching loan stats: {}", e);
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

async fn health_check() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "service": "lending-service",
        "status": "healthy",
        "version": "0.2.0",
        "features": ["repayment", "liquidation"]
    }))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("🤝 BSV Bank - Lending Service Starting...");
    let pool = create_pool().await
        .expect("Failed to create database pool");
    println!("✅ Service ready on http://0.0.0.0:8082");
    println!("📋 New endpoints: /repay, /liquidations, /my-loans");
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(pool.clone()))
            .route("/health", web::get().to(health_check))
            .route("/loans/request", web::post().to(create_loan_request))
            .route("/loans/available", web::get().to(get_available_loans))
            .route("/loans/my-loans/{paymail}", web::get().to(get_user_loans))
            .route("/loans/{id}/fund", web::post().to(fund_loan))
            .route("/loans/{id}/repay", web::post().to(repay_loan))
            .route("/loans/liquidations/check", web::post().to(check_liquidations))
            .configure(configure_routes)
    })
    .bind(("0.0.0.0", 8082))?
    .run()
    .await
}