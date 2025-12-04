// core/payment-channel-service/src/main.rs
// Payment Channel Service with Phase 6 Production Hardening

use actix_web::{web, App, HttpResponse, HttpServer, Responder, Result, middleware};
use actix_cors::Cors;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::time::{Instant, SystemTime};
use bsv_bank_common::{
    init_logging, ServiceMetrics,
    validate_paymail, validate_amount,
};
use dotenv::dotenv;
use prometheus::Registry;
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
// DATA STRUCTURES
// ============================================================================

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct PaymentChannel {
    pub id: Uuid,
    pub channel_id: String,
    pub party_a_paymail: String,
    pub party_b_paymail: String,
    pub initial_balance_a: i64,
    pub initial_balance_b: i64,
    pub current_balance_a: i64,
    pub current_balance_b: i64,
    pub status: String,
    pub sequence_number: i64,
    pub opened_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub last_payment_at: Option<DateTime<Utc>>,
    pub settlement_txid: Option<String>,
    pub timeout_blocks: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    // Phase 5 additions (optional for backward compatibility)
    pub blockchain_enabled: Option<bool>,
    pub funding_txid: Option<String>,
    pub funding_address: Option<String>,
    pub funding_vout: Option<i32>,
    pub funding_confirmations: Option<i32>,
    pub settlement_confirmations: Option<i32>,
    pub spv_verified: Option<bool>,
    pub multisig_script: Option<String>,
    pub redeem_script: Option<String>,
    pub current_commitment_txid: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenChannelRequest {
    pub party_a_paymail: String,
    pub party_b_paymail: String,
    pub initial_balance_a: i64,
    pub initial_balance_b: i64,
    #[serde(default = "default_timeout")]
    pub timeout_blocks: i32,
}

fn default_timeout() -> i32 {
    144
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SendPaymentRequest {
    pub from_paymail: String,
    pub to_paymail: String,
    pub amount_satoshis: i64,
    pub memo: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ChannelPayment {
    pub id: Uuid,
    pub channel_id: String,
    pub from_paymail: String,
    pub to_paymail: String,
    pub amount_satoshis: i64,
    pub sequence_number: i64,
    pub memo: Option<String>,
    pub balance_a_after: i64,
    pub balance_b_after: i64,
    pub created_at: DateTime<Utc>,
    pub processing_time_ms: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct PaymentResponse {
    pub payment_id: Uuid,
    pub channel_id: String,
    pub from_paymail: String,
    pub to_paymail: String,
    pub amount_satoshis: i64,
    pub sequence_number: i64,
    pub balance_a: i64,
    pub balance_b: i64,
    pub created_at: DateTime<Utc>,
    pub processing_time_ms: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CloseChannelRequest {
    pub party_paymail: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ForceCloseRequest {
    pub party_paymail: String,
    pub reason: Option<String>,
}

struct AppState {
    db_pool: PgPool,
    start_time: SystemTime,
}

// ============================================================================
// UTILITY FUNCTIONS
// ============================================================================

fn generate_channel_id(party_a: &str, party_b: &str) -> String {
    use sha2::{Sha256, Digest};
    let now = Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let input = format!("{}{}{}", party_a, party_b, now);
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("0x{:x}", hasher.finalize())
}

fn create_error_response(error: &str, message: &str) -> HttpResponse {
    HttpResponse::BadRequest().json(ErrorResponse {
        error: error.to_string(),
        message: message.to_string(),
        timestamp: Utc::now(),
    })
}

fn validate_channel_request(request: &OpenChannelRequest) -> Result<(), ServiceError> {
    // Phase 6: Validate both paymails
    validate_paymail(&request.party_a_paymail)
        .map_err(|e| ServiceError::ValidationError(e.to_string()))?;
    validate_paymail(&request.party_b_paymail)
        .map_err(|e| ServiceError::ValidationError(e.to_string()))?;
    
    // Phase 6: Validate amounts
    if request.initial_balance_a != 0 {
        validate_amount(request.initial_balance_a)
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;
    }
    if request.initial_balance_b != 0 {
        validate_amount(request.initial_balance_b)
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;
    }
    
    // Service-specific validation
    if request.party_a_paymail == request.party_b_paymail {
        return Err(ServiceError::BusinessError("Cannot create channel with yourself".to_string()));
    }
    
    if request.initial_balance_a < 0 || request.initial_balance_b < 0 {
        return Err(ServiceError::BusinessError("Balances must be non-negative".to_string()));
    }
    
    if request.timeout_blocks <= 0 {
        return Err(ServiceError::BusinessError("Timeout must be positive".to_string()));
    }
    
    Ok(())
}

// ============================================================================
// API ENDPOINTS
// ============================================================================

async fn open_channel(
    pool: web::Data<PgPool>,
    request: web::Json<OpenChannelRequest>,
) -> Result<HttpResponse, ServiceError> {
    // Phase 6: Validate all inputs
    validate_channel_request(&request)?;
    
    // Generate unique channel ID
    let channel_id = generate_channel_id(&request.party_a_paymail, &request.party_b_paymail);
    
    // Create channel in database
    let result = sqlx::query_as::<_, PaymentChannel>(
        r#"
        INSERT INTO payment_channels (
            channel_id,
            party_a_paymail,
            party_b_paymail,
            initial_balance_a,
            initial_balance_b,
            current_balance_a,
            current_balance_b,
            status,
            timeout_blocks
        ) VALUES ($1, $2, $3, $4, $5, $4, $5, 'Open', $6)
        RETURNING *
        "#
    )
    .bind(&channel_id)
    .bind(&request.party_a_paymail)
    .bind(&request.party_b_paymail)
    .bind(request.initial_balance_a)
    .bind(request.initial_balance_b)
    .bind(request.timeout_blocks)
    .fetch_one(pool.get_ref())
    .await
    .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;
    
    // Create initial state snapshot
    let _ = sqlx::query!(
        r#"
        INSERT INTO channel_states (
            channel_id, sequence_number, balance_a, balance_b
        ) VALUES ($1, 0, $2, $3)
        "#,
        &channel_id,
        request.initial_balance_a,
        request.initial_balance_b
    )
    .execute(pool.get_ref())
    .await;
    
    tracing::info!("Channel opened: {} between {} and {}", 
        channel_id, request.party_a_paymail, request.party_b_paymail);
    
    Ok(HttpResponse::Ok().json(result))
}

async fn send_payment(
    pool: web::Data<PgPool>,
    channel_id: web::Path<String>,
    request: web::Json<SendPaymentRequest>,
) -> Result<HttpResponse> {
    let start_time = Instant::now();
    
    // Phase 6: Validate paymails
    if let Err(e) = validate_paymail(&request.from_paymail) {
        return Ok(create_error_response("ValidationError", &e.to_string()));
    }
    if let Err(e) = validate_paymail(&request.to_paymail) {
        return Ok(create_error_response("ValidationError", &e.to_string()));
    }
    
    // Phase 6: Validate amount
    if let Err(e) = validate_amount(request.amount_satoshis) {
        return Ok(create_error_response("ValidationError", &e.to_string()));
    }
    
    // Service-specific validation
    if request.from_paymail == request.to_paymail {
        return Ok(create_error_response(
            "InvalidRequest",
            "Cannot pay yourself"
        ));
    }
    
    // Use database function for atomic payment processing
    let result = sqlx::query!(
        r#"
        SELECT process_channel_payment($1, $2, $3, $4, $5)::text as "result!"
        "#,
        channel_id.as_str(),
        &request.from_paymail,
        &request.to_paymail,
        request.amount_satoshis,
        request.memo.as_deref()
    )
    .fetch_one(pool.get_ref())
    .await;
    
    match result {
        Ok(record) => {
            let processing_time = start_time.elapsed().as_millis() as i32;
            
            let payment_data: serde_json::Value = match serde_json::from_str(&record.result) {
                Ok(data) => data,
                Err(e) => {
                    tracing::error!("JSON parse error: {} - Raw: {}", e, record.result);
                    return Ok(create_error_response(
                        "ProcessingError",
                        "Invalid response from database"
                    ));
                }
            };
            
            if !payment_data.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
                return Ok(create_error_response(
                    "ProcessingError",
                    "Payment processing failed"
                ));
            }
            
            let payment_id_str = payment_data
                .get("payment_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            
            let payment_id = Uuid::parse_str(payment_id_str)
                .unwrap_or_else(|_| Uuid::new_v4());
            
            // Update processing time
            let _ = sqlx::query!(
                "UPDATE channel_payments SET processing_time_ms = $1 WHERE id = $2",
                processing_time,
                payment_id
            )
            .execute(pool.get_ref())
            .await;
            
            tracing::info!("Payment processed: {} in {}ms", payment_id, processing_time);
            
            Ok(HttpResponse::Ok().json(PaymentResponse {
                payment_id,
                channel_id: channel_id.to_string(),
                from_paymail: request.from_paymail.clone(),
                to_paymail: request.to_paymail.clone(),
                amount_satoshis: request.amount_satoshis,
                sequence_number: payment_data
                    .get("sequence_number")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0),
                balance_a: payment_data
                    .get("balance_a")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0),
                balance_b: payment_data
                    .get("balance_b")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0),
                created_at: Utc::now(),
                processing_time_ms: processing_time,
            }))
        }
        Err(e) => {
            tracing::error!("Payment error: {}", e);
            let error_msg = e.to_string();
            
            if error_msg.contains("not found") {
                Ok(HttpResponse::NotFound().json(ErrorResponse {
                    error: "ChannelNotFound".to_string(),
                    message: "Channel does not exist".to_string(),
                    timestamp: Utc::now(),
                }))
            } else if error_msg.contains("not active") {
                Ok(create_error_response(
                    "ChannelInactive",
                    "Channel is not active"
                ))
            } else if error_msg.contains("Insufficient balance") {
                Ok(create_error_response(
                    "InsufficientBalance",
                    &error_msg
                ))
            } else if error_msg.contains("not a party") {
                Ok(HttpResponse::Forbidden().json(ErrorResponse {
                    error: "Unauthorized".to_string(),
                    message: error_msg,
                    timestamp: Utc::now(),
                }))
            } else {
                Ok(create_error_response(
                    "PaymentError",
                    &format!("Failed to process payment: {}", error_msg)
                ))
            }
        }
    }
}

async fn get_channel(
    pool: web::Data<PgPool>,
    channel_id: web::Path<String>,
) -> Result<HttpResponse> {
    let result = sqlx::query_as::<_, PaymentChannel>(
        "SELECT * FROM payment_channels WHERE channel_id = $1"
    )
    .bind(channel_id.as_str())
    .fetch_optional(pool.get_ref())
    .await;
    
    match result {
        Ok(Some(channel)) => Ok(HttpResponse::Ok().json(channel)),
        Ok(None) => Ok(HttpResponse::NotFound().json(ErrorResponse {
            error: "NotFound".to_string(),
            message: "Channel not found".to_string(),
            timestamp: Utc::now(),
        })),
        Err(e) => {
            tracing::error!("Error fetching channel: {}", e);
            Ok(create_error_response(
                "DatabaseError",
                "Failed to fetch channel"
            ))
        }
    }
}

async fn get_channel_history(
    pool: web::Data<PgPool>,
    channel_id: web::Path<String>,
) -> Result<HttpResponse> {
    let payments = sqlx::query_as::<_, ChannelPayment>(
        r#"
        SELECT * FROM channel_payments 
        WHERE channel_id = $1 
        ORDER BY created_at DESC
        LIMIT 100
        "#
    )
    .bind(channel_id.as_str())
    .fetch_all(pool.get_ref())
    .await;
    
    match payments {
        Ok(payments) => Ok(HttpResponse::Ok().json(serde_json::json!({
            "channel_id": channel_id.as_str(),
            "total_payments": payments.len(),
            "payments": payments
        }))),
        Err(e) => {
            tracing::error!("Error fetching history: {}", e);
            Ok(create_error_response(
                "DatabaseError",
                "Failed to fetch payment history"
            ))
        }
    }
}

async fn get_user_channels(
    pool: web::Data<PgPool>,
    paymail: web::Path<String>,
) -> Result<HttpResponse> {
    // Phase 6: Validate paymail
    if let Err(e) = validate_paymail(&paymail) {
        return Ok(create_error_response("ValidationError", &e.to_string()));
    }
    
    let channels = sqlx::query_as::<_, PaymentChannel>(
        r#"
        SELECT * FROM payment_channels 
        WHERE party_a_paymail = $1 OR party_b_paymail = $1
        ORDER BY opened_at DESC
        "#
    )
    .bind(paymail.as_str())
    .fetch_all(pool.get_ref())
    .await;
    
    match channels {
        Ok(channels) => Ok(HttpResponse::Ok().json(serde_json::json!({
            "paymail": paymail.as_str(),
            "total_channels": channels.len(),
            "channels": channels
        }))),
        Err(e) => {
            tracing::error!("Error fetching user channels: {}", e);
            Ok(create_error_response(
                "DatabaseError",
                "Failed to fetch channels"
            ))
        }
    }
}

async fn get_all_channels(
    pool: web::Data<PgPool>,
) -> Result<HttpResponse> {
    let channels = sqlx::query_as::<_, PaymentChannel>(
        r#"
        SELECT * FROM payment_channels 
        ORDER BY opened_at DESC
        LIMIT 1000
        "#
    )
    .fetch_all(pool.get_ref())
    .await;
    
    match channels {
        Ok(channels) => Ok(HttpResponse::Ok().json(serde_json::json!({
            "total_channels": channels.len(),
            "channels": channels
        }))),
        Err(e) => {
            tracing::error!("Error fetching all channels: {}", e);
            Ok(create_error_response(
                "DatabaseError",
                "Failed to fetch channels"
            ))
        }
    }
}

async fn get_channel_balance(
    pool: web::Data<PgPool>,
    channel_id: web::Path<String>,
) -> Result<HttpResponse> {
    let channel = sqlx::query_as::<_, PaymentChannel>(
        "SELECT * FROM payment_channels WHERE channel_id = $1"
    )
    .bind(channel_id.as_str())
    .fetch_optional(pool.get_ref())
    .await;
    
    match channel {
        Ok(Some(channel)) => Ok(HttpResponse::Ok().json(serde_json::json!({
            "channel_id": channel.channel_id,
            "party_a_paymail": channel.party_a_paymail,
            "party_b_paymail": channel.party_b_paymail,
            "balance_a": channel.current_balance_a,
            "balance_b": channel.current_balance_b,
            "sequence_number": channel.sequence_number
        }))),
        Ok(None) => Ok(HttpResponse::NotFound().json(ErrorResponse {
            error: "NotFound".to_string(),
            message: "Channel not found".to_string(),
            timestamp: Utc::now(),
        })),
        Err(e) => {
            tracing::error!("Error fetching balance: {}", e);
            Ok(create_error_response(
                "DatabaseError",
                "Failed to fetch balance"
            ))
        }
    }
}

async fn close_channel(
    pool: web::Data<PgPool>,
    channel_id: web::Path<String>,
    request: web::Json<CloseChannelRequest>,
) -> Result<HttpResponse> {
    // Phase 6: Validate paymail
    if let Err(e) = validate_paymail(&request.party_paymail) {
        return Ok(create_error_response("ValidationError", &e.to_string()));
    }
    
    let settlement_txid = format!("mock-settlement-{}", Uuid::new_v4());
    
    let result = sqlx::query_as::<_, PaymentChannel>(
        r#"
        UPDATE payment_channels 
        SET status = 'Closed',
            closed_at = NOW(),
            settlement_txid = $1,
            updated_at = NOW()
        WHERE channel_id = $2
            AND (party_a_paymail = $3 OR party_b_paymail = $3)
            AND status IN ('Open', 'Active')
        RETURNING *
        "#
    )
    .bind(&settlement_txid)
    .bind(channel_id.as_str())
    .bind(&request.party_paymail)
    .fetch_optional(pool.get_ref())
    .await;
    
    match result {
        Ok(Some(channel)) => {
            tracing::info!("Channel closed: {}", channel_id);
            Ok(HttpResponse::Ok().json(serde_json::json!({
                "channel_id": channel.channel_id,
                "status": "Closed",
                "final_balance_a": channel.current_balance_a,
                "final_balance_b": channel.current_balance_b,
                "settlement_txid": settlement_txid,
                "closed_at": channel.closed_at,
                "success": true
            })))
        }
        Ok(None) => Ok(create_error_response(
            "ClosureError",
            "Channel not found or already closed"
        )),
        Err(e) => {
            tracing::error!("Error closing channel: {}", e);
            Ok(create_error_response(
                "DatabaseError",
                "Failed to close channel"
            ))
        }
    }
}

async fn get_channel_stats(
    pool: web::Data<PgPool>,
    channel_id: web::Path<String>,
) -> Result<HttpResponse> {
    let stats = sqlx::query!(
        r#"
        SELECT 
            COUNT(*) as total_payments,
            COALESCE(SUM(amount_satoshis)::BIGINT, 0) as "total_volume!",
            COALESCE(AVG(amount_satoshis)::BIGINT, 0) as "avg_payment!",
            COALESCE(MAX(amount_satoshis), 0) as max_payment,
            COALESCE(MIN(amount_satoshis), 0) as min_payment
        FROM channel_payments
        WHERE channel_id = $1
        "#,
        channel_id.as_str()
    )
    .fetch_one(pool.get_ref())
    .await;
    
    match stats {
        Ok(stats) => {
            let total_payments = stats.total_payments.unwrap_or(0);
            
            Ok(HttpResponse::Ok().json(serde_json::json!({
                "channel_id": channel_id.as_str(),
                "total_payments": total_payments,
                "total_volume": stats.total_volume,
                "average_payment": stats.avg_payment,
                "largest_payment": stats.max_payment.unwrap_or(0),
                "smallest_payment": if total_payments > 0 {
                    stats.min_payment.unwrap_or(0)
                } else {
                    0
                }
            })))
        }
        Err(e) => {
            tracing::error!("Error fetching channel stats: {}", e);
            Ok(create_error_response(
                "DatabaseError",
                "Failed to fetch statistics"
            ))
        }
    }
}

async fn get_network_stats(
    pool: web::Data<PgPool>,
) -> Result<HttpResponse> {
    let channel_stats = sqlx::query!(
        r#"
        SELECT 
            COUNT(*) as total_channels,
            COUNT(CASE WHEN status = 'Active' THEN 1 END) as active_channels,
            COUNT(CASE WHEN status = 'Open' THEN 1 END) as open_channels,
            COUNT(CASE WHEN status = 'Closed' THEN 1 END) as closed_channels,
            COALESCE(SUM(current_balance_a + current_balance_b)::BIGINT, 0) as "total_value_locked!"
        FROM payment_channels
        "#
    )
    .fetch_one(pool.get_ref())
    .await;
    
    let payment_stats = sqlx::query!(
        r#"
        SELECT 
            COUNT(*) as payments_24h,
            COALESCE(SUM(amount_satoshis)::BIGINT, 0) as "volume_24h!"
        FROM channel_payments
        WHERE created_at > NOW() - INTERVAL '24 hours'
        "#
    )
    .fetch_one(pool.get_ref())
    .await;
    
    match (channel_stats, payment_stats) {
        (Ok(channels), Ok(payments)) => {
            let total_channels = channels.total_channels.unwrap_or(0);
            let avg_balance = if total_channels > 0 {
                channels.total_value_locked / total_channels
            } else {
                0
            };
            
            Ok(HttpResponse::Ok().json(serde_json::json!({
                "total_channels": total_channels,
                "active_channels": channels.active_channels.unwrap_or(0),
                "open_channels": channels.open_channels.unwrap_or(0),
                "closed_channels": channels.closed_channels.unwrap_or(0),
                "total_value_locked": channels.total_value_locked,
                "average_channel_balance": avg_balance,
                "total_payments_24h": payments.payments_24h.unwrap_or(0),
                "total_volume_24h": payments.volume_24h,
                "timestamp": Utc::now()
            })))
        }
        _ => {
            Ok(create_error_response(
                "DatabaseError",
                "Failed to fetch network statistics"
            ))
        }
    }
}

async fn force_close_channel(
    pool: web::Data<PgPool>,
    channel_id: web::Path<String>,
    request: web::Json<ForceCloseRequest>,
) -> Result<HttpResponse> {
    // Phase 6: Validate paymail
    if let Err(e) = validate_paymail(&request.party_paymail) {
        return Ok(create_error_response("ValidationError", &e.to_string()));
    }
    
    let channel = sqlx::query_as::<_, PaymentChannel>(
        "SELECT * FROM payment_channels WHERE channel_id = $1"
    )
    .bind(channel_id.as_str())
    .fetch_optional(pool.get_ref())
    .await;
    
    match channel {
        Ok(Some(channel)) => {
            if channel.party_a_paymail != request.party_paymail 
                && channel.party_b_paymail != request.party_paymail {
                return Ok(HttpResponse::Forbidden().json(ErrorResponse {
                    error: "Unauthorized".to_string(),
                    message: "Only channel parties can force close".to_string(),
                    timestamp: Utc::now(),
                }));
            }
            
            if channel.status == "Closed" {
                return Ok(create_error_response(
                    "ChannelClosed",
                    "Channel is already closed"
                ));
            }
            
            let result = sqlx::query_as::<_, PaymentChannel>(
                r#"
                UPDATE payment_channels 
                SET status = 'Disputed',
                    updated_at = NOW()
                WHERE channel_id = $1
                RETURNING *
                "#
            )
            .bind(channel_id.as_str())
            .fetch_one(pool.get_ref())
            .await;
            
            match result {
                Ok(updated_channel) => {
                    tracing::warn!("Force close initiated: {} by {}", channel_id, request.party_paymail);
                    Ok(HttpResponse::Ok().json(serde_json::json!({
                        "channel_id": updated_channel.channel_id,
                        "status": "Disputed",
                        "dispute_initiated_by": request.party_paymail,
                        "reason": request.reason.as_ref().unwrap_or(&"No reason provided".to_string()),
                        "current_balance_a": updated_channel.current_balance_a,
                        "current_balance_b": updated_channel.current_balance_b,
                        "dispute_started_at": Utc::now(),
                        "timeout_blocks": updated_channel.timeout_blocks,
                        "message": "Force closure initiated. Counterparty has timeout period to respond."
                    })))
                }
                Err(e) => {
                    tracing::error!("Error force closing channel: {}", e);
                    Ok(create_error_response(
                        "DatabaseError",
                        "Failed to force close channel"
                    ))
                }
            }
        }
        Ok(None) => {
            Ok(HttpResponse::NotFound().json(ErrorResponse {
                error: "NotFound".to_string(),
                message: "Channel not found".to_string(),
                timestamp: Utc::now(),
            }))
        }
        Err(e) => {
            tracing::error!("Error fetching channel: {}", e);
            Ok(create_error_response(
                "DatabaseError",
                "Failed to fetch channel"
            ))
        }
    }
}

async fn check_timeouts(
    pool: web::Data<PgPool>,
) -> Result<HttpResponse> {
    // Find disputed channels that have exceeded timeout
    // In a real implementation, this would check blockchain height
    // For now, we'll use a simple time-based check

    let expired = sqlx::query_as::<_, PaymentChannel>(
        r#"
        SELECT * FROM payment_channels
        WHERE status = 'Disputed'
        AND updated_at < NOW() - INTERVAL '1 hour'
        "#
    )
    .fetch_all(pool.get_ref())
    .await;

    match expired {
        Ok(channels) => {
            let mut closed_channels = Vec::new();
            
            for channel in channels {
                let settlement_txid = format!("force-settlement-{}", Uuid::new_v4());
                
                let result = sqlx::query!(
                    r#"
                    UPDATE payment_channels
                    SET status = 'Closed',
                        closed_at = NOW(),
                        settlement_txid = $1
                    WHERE channel_id = $2
                    "#,
                    &settlement_txid,
                    &channel.channel_id
                )
                .execute(pool.get_ref())
                .await;
                
                if result.is_ok() {
                    closed_channels.push(serde_json::json!({
                        "channel_id": channel.channel_id,
                        "final_balance_a": channel.current_balance_a,
                        "final_balance_b": channel.current_balance_b,
                        "settlement_txid": settlement_txid
                    }));
                }
            }
            
            Ok(HttpResponse::Ok().json(serde_json::json!({
                "checked_at": Utc::now(),
                "expired_channels": closed_channels.len(),
                "channels": closed_channels,
                "message": format!("Processed {} expired dispute(s)", closed_channels.len())
            })))
        }
        Err(e) => {
            tracing::error!("Error checking timeouts: {}", e);
            Ok(create_error_response(
                "DatabaseError",
                "Failed to check timeouts"
            ))
        }
    }
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
        "service": "payment-channel-service",
        "status": "healthy",
        "version": "1.0.0",
        "timestamp": Utc::now(),
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
    println!("⚡ BSV Bank - Payment Channel Service Starting (Phase 6)...");

    // Get configuration
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| {
            println!("⚠️  DATABASE_URL not set, using default");
            "postgres://postgres:postgres@localhost:5432/bsv_bank".to_string()
        });

    let port: u16 = 8083; // Fixed port for payment-channel-service

    // Phase 6: Initialize structured logging
    init_logging("payment-channel-service");
    tracing::info!("Starting Payment Channel Service on port {}", port);

    // Database connection pool
    println!("📡 Connecting to database...");
    let db_pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
        .expect("Failed to connect to database");

    println!("✅ Database connected");
    tracing::info!("Database connection established");

    // Phase 6: Prometheus metrics
    let registry = Registry::new();
    let _service_metrics = ServiceMetrics::new(&registry, "payment_channel_service")
        .expect("Failed to create service metrics");
    let _channel_metrics = bsv_bank_common::ChannelMetrics::new(&registry)
        .expect("Failed to create channel metrics");
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
    println!("📋 Endpoints:");
    println!("   POST /channels/open");
    println!("   POST /channels/{{id}}/payment");
    println!("   GET  /channels/{{id}}");
    println!("   POST /channels/{{id}}/close");
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
            .route("/channels/open", web::post().to(open_channel))
            .route("/channels/{channel_id}/payment", web::post().to(send_payment))
            .route("/channels/{channel_id}", web::get().to(get_channel))
            .route("/channels/{channel_id}/history", web::get().to(get_channel_history))
            .route("/channels/{channel_id}/balance", web::get().to(get_channel_balance))
            .route("/channels/user/{paymail}", web::get().to(get_user_channels))
            .route("/channels", web::get().to(get_all_channels))
            .route("/channels/{channel_id}/stats", web::get().to(get_channel_stats))
            .route("/stats/network", web::get().to(get_network_stats))
            .route("/channels/{channel_id}/force-close", web::post().to(force_close_channel))
            .route("/channels/check-timeouts", web::post().to(check_timeouts))            
            .route("/channels/{channel_id}/close", web::post().to(close_channel))
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}