// core/payment-channel-service/src/main.rs
// Payment Channel Service for BSV Bank
// Enables instant, low-cost micropayments through off-chain channels

use actix_web::{web, App, HttpResponse, HttpServer, Responder, Result};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::time::Instant;
use actix_cors::Cors;

// ============================================================================
// DATA STRUCTURES
// ============================================================================

// #[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
// pub struct PaymentChannel {
//     pub id: Uuid,
//     pub channel_id: String,
//     pub party_a_paymail: String,
//     pub party_b_paymail: String,
//     pub initial_balance_a: i64,
//     pub initial_balance_b: i64,
//     pub current_balance_a: i64,
//     pub current_balance_b: i64,
//     pub status: String,
//     pub sequence_number: i64,
//     pub opened_at: DateTime<Utc>,
//     pub closed_at: Option<DateTime<Utc>>,
//     pub last_payment_at: Option<DateTime<Utc>>,
//     pub settlement_txid: Option<String>,
//     pub timeout_blocks: i32,
//     pub created_at: DateTime<Utc>,
//     pub updated_at: DateTime<Utc>,
// }

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

// ============================================================================
// DATABASE CONNECTION
// ============================================================================

async fn create_pool() -> Result<PgPool, sqlx::Error> {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://a:@localhost:5432/bsv_bank".to_string());
    
    PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
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

// ============================================================================
// API ENDPOINTS
// ============================================================================

// Health check endpoint
async fn health_check() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "service": "payment-channel-service",
        "status": "healthy",
        "version": "1.0.0",
        "timestamp": Utc::now()
    }))
}

// Open a new payment channel
async fn open_channel(
    pool: web::Data<PgPool>,
    request: web::Json<OpenChannelRequest>,
) -> Result<HttpResponse> {
    // Validate request
    if request.party_a_paymail == request.party_b_paymail {
        return Ok(create_error_response(
            "InvalidRequest",
            "Cannot create channel with yourself"
        ));
    }
    
    if request.initial_balance_a < 0 || request.initial_balance_b < 0 {
        return Ok(create_error_response(
            "InvalidBalance",
            "Balances must be non-negative"
        ));
    }
    
    if request.timeout_blocks <= 0 {
        return Ok(create_error_response(
            "InvalidTimeout",
            "Timeout must be positive"
        ));
    }
    
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
    .await;
    
    match result {
        Ok(channel) => {
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
            
            Ok(HttpResponse::Ok().json(channel))
        }
        Err(e) => {
            eprintln!("Error creating channel: {}", e);
            Ok(create_error_response(
                "DatabaseError",
                "Failed to create channel"
            ))
        }
    }
}

// // Send payment through channel
// async fn send_payment(
//     pool: web::Data<PgPool>,
//     channel_id: web::Path<String>,
//     request: web::Json<SendPaymentRequest>,
// ) -> Result<HttpResponse> {
//     let start_time = Instant::now();
    
//     // Validate request
//     if request.amount_satoshis <= 0 {
//         return Ok(create_error_response(
//             "InvalidAmount",
//             "Amount must be positive"
//         ));
//     }
    
//     if request.from_paymail == request.to_paymail {
//         return Ok(create_error_response(
//             "InvalidRequest",
//             "Cannot pay yourself"
//         ));
//     }
    
//     // Use database function for atomic payment processing
//     let result = sqlx::query!(
//         r#"
//         SELECT process_channel_payment($1, $2, $3, $4, $5) as result
//         "#,
//         channel_id.as_str(),
//         &request.from_paymail,
//         &request.to_paymail,
//         request.amount_satoshis,
//         request.memo.as_deref()
//     )
//     .fetch_one(pool.get_ref())
//     .await;
    
//     match result {
//         Ok(record) => {
//             let processing_time = start_time.elapsed().as_millis() as i32;
            
//             if let Some(json) = record.result {
//                 let payment_data: serde_json::Value = json;
                
//                 // Update processing time
//                 if let Some(payment_id) = payment_data.get("payment_id").and_then(|v| v.as_str()) {
//                     let _ = sqlx::query!(
//                         "UPDATE channel_payments SET processing_time_ms = $1 WHERE id = $2",
//                         processing_time,
//                         Uuid::parse_str(payment_id).ok()
//                     )
//                     .execute(pool.get_ref())
//                     .await;
//                 }
                
//                 Ok(HttpResponse::Ok().json(PaymentResponse {
//                     payment_id: Uuid::parse_str(
//                         payment_data.get("payment_id")
//                             .and_then(|v| v.as_str())
//                             .unwrap_or("")
//                     ).unwrap_or_else(|_| Uuid::new_v4()),
//                     channel_id: channel_id.to_string(),
//                     from_paymail: request.from_paymail.clone(),
//                     to_paymail: request.to_paymail.clone(),
//                     amount_satoshis: request.amount_satoshis,
//                     sequence_number: payment_data.get("sequence_number")
//                         .and_then(|v| v.as_i64())
//                         .unwrap_or(0),
//                     balance_a: payment_data.get("balance_a")
//                         .and_then(|v| v.as_i64())
//                         .unwrap_or(0),
//                     balance_b: payment_data.get("balance_b")
//                         .and_then(|v| v.as_i64())
//                         .unwrap_or(0),
//                     created_at: Utc::now(),
//                     processing_time_ms: processing_time,
//                 }))
//             } else {
//                 Ok(create_error_response(
//                     "ProcessingError",
//                     "Payment processing failed"
//                 ))
//             }
//         }
//         Err(e) => {
//             eprintln!("Payment error: {}", e);
//             let error_msg = e.to_string();
            
//             if error_msg.contains("not found") {
//                 Ok(HttpResponse::NotFound().json(ErrorResponse {
//                     error: "ChannelNotFound".to_string(),
//                     message: "Channel does not exist".to_string(),
//                     timestamp: Utc::now(),
//                 }))
//             } else if error_msg.contains("not active") {
//                 Ok(create_error_response(
//                     "ChannelInactive",
//                     "Channel is not active"
//                 ))
//             } else if error_msg.contains("Insufficient balance") {
//                 Ok(create_error_response(
//                     "InsufficientBalance",
//                     &error_msg
//                 ))
//             } else if error_msg.contains("not a party") {
//                 Ok(HttpResponse::Forbidden().json(ErrorResponse {
//                     error: "Unauthorized".to_string(),
//                     message: error_msg,
//                     timestamp: Utc::now(),
//                 }))
//             } else {
//                 Ok(create_error_response(
//                     "PaymentError",
//                     &format!("Failed to process payment: {}", error_msg)
//                 ))
//             }
//         }
//     }
// }

async fn send_payment(
    pool: web::Data<PgPool>,
    channel_id: web::Path<String>,
    request: web::Json<SendPaymentRequest>,
) -> Result<HttpResponse> {
    let start_time = Instant::now();
    
    // Validate request
    if request.amount_satoshis <= 0 {
        return Ok(create_error_response(
            "InvalidAmount",
            "Amount must be positive"
        ));
    }
    
    if request.from_paymail == request.to_paymail {
        return Ok(create_error_response(
            "InvalidRequest",
            "Cannot pay yourself"
        ));
    }
    
    // Use database function for atomic payment processing
    // Cast to text and mark as non-null with "result!"
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
            
            // Parse the JSON string directly (no Option check needed with !)
            let payment_data: serde_json::Value = match serde_json::from_str(&record.result) {
                Ok(data) => data,
                Err(e) => {
                    eprintln!("JSON parse error: {} - Raw: {}", e, record.result);
                    return Ok(create_error_response(
                        "ProcessingError",
                        "Invalid response from database"
                    ));
                }
            };
            
            // Check success flag
            if !payment_data.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
                return Ok(create_error_response(
                    "ProcessingError",
                    "Payment processing failed"
                ));
            }
            
            // Extract payment ID
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
            
            // Return success response
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
            eprintln!("Payment error: {}", e);
            let error_msg = e.to_string();
            
            // Handle specific error cases
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

// Get channel details
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
            eprintln!("Error fetching channel: {}", e);
            Ok(create_error_response(
                "DatabaseError",
                "Failed to fetch channel"
            ))
        }
    }
}

// Get channel payment history
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
            eprintln!("Error fetching history: {}", e);
            Ok(create_error_response(
                "DatabaseError",
                "Failed to fetch payment history"
            ))
        }
    }
}

// Get user's channels
async fn get_user_channels(
    pool: web::Data<PgPool>,
    paymail: web::Path<String>,
) -> Result<HttpResponse> {
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
            eprintln!("Error fetching user channels: {}", e);
            Ok(create_error_response(
                "DatabaseError",
                "Failed to fetch channels"
            ))
        }
    }
}

// Get all channels (for admin/debugging)
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
            eprintln!("Error fetching all channels: {}", e);
            Ok(create_error_response(
                "DatabaseError",
                "Failed to fetch channels"
            ))
        }
    }
}

// Get current balance
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
            eprintln!("Error fetching balance: {}", e);
            Ok(create_error_response(
                "DatabaseError",
                "Failed to fetch balance"
            ))
        }
    }
}

// Close channel cooperatively
async fn close_channel(
    pool: web::Data<PgPool>,
    channel_id: web::Path<String>,
    request: web::Json<CloseChannelRequest>,
) -> Result<HttpResponse> {
    // Generate mock settlement transaction ID
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
        Ok(Some(channel)) => Ok(HttpResponse::Ok().json(serde_json::json!({
            "channel_id": channel.channel_id,
            "status": "Closed",
            "final_balance_a": channel.current_balance_a,
            "final_balance_b": channel.current_balance_b,
            "settlement_txid": settlement_txid,
            "closed_at": channel.closed_at,
            "success": true
        }))),
        Ok(None) => Ok(create_error_response(
            "ClosureError",
            "Channel not found or already closed"
        )),
        Err(e) => {
            eprintln!("Error closing channel: {}", e);
            Ok(create_error_response(
                "DatabaseError",
                "Failed to close channel"
            ))
        }
    }
}

// ============================================================================
// STATISTICS & ANALYTICS ENDPOINTS
// ============================================================================

// Get channel statistics
async fn get_channel_stats(
    pool: web::Data<PgPool>,
    channel_id: web::Path<String>,
) -> Result<HttpResponse> {
    // Get payment count and volume (cast aggregates to BIGINT)
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
            eprintln!("Error fetching channel stats: {}", e);
            Ok(create_error_response(
                "DatabaseError",
                "Failed to fetch statistics"
            ))
        }
    }
}

// Get network-wide statistics
async fn get_network_stats(
    pool: web::Data<PgPool>,
) -> Result<HttpResponse> {
    // Get channel counts by status (cast SUM to BIGINT)
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
    
    // Get payment stats for last 24 hours (cast SUM to BIGINT)
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
                channels.total_value_locked / total_channels  // No unwrap_or needed - it's marked with !
            } else {
                0
            };
            
            Ok(HttpResponse::Ok().json(serde_json::json!({
                "total_channels": total_channels,
                "active_channels": channels.active_channels.unwrap_or(0),
                "open_channels": channels.open_channels.unwrap_or(0),
                "closed_channels": channels.closed_channels.unwrap_or(0),
                "total_value_locked": channels.total_value_locked,  // No unwrap_or - marked with !
                "average_channel_balance": avg_balance,
                "total_payments_24h": payments.payments_24h.unwrap_or(0),
                "total_volume_24h": payments.volume_24h,  // No unwrap_or - marked with !
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

// Force close a channel (dispute handling)
#[derive(Debug, Serialize, Deserialize)]
pub struct ForceCloseRequest {
    pub party_paymail: String,
    pub reason: Option<String>,
}

async fn force_close_channel(
    pool: web::Data<PgPool>,
    channel_id: web::Path<String>,
    request: web::Json<ForceCloseRequest>,
) -> Result<HttpResponse> {
    // Get current channel state
    let channel = sqlx::query_as::<_, PaymentChannel>(
        "SELECT * FROM payment_channels WHERE channel_id = $1"
    )
    .bind(channel_id.as_str())
    .fetch_optional(pool.get_ref())
    .await;
    
    match channel {
        Ok(Some(channel)) => {
            // Verify party is authorized
            if channel.party_a_paymail != request.party_paymail 
                && channel.party_b_paymail != request.party_paymail {
                return Ok(HttpResponse::Forbidden().json(ErrorResponse {
                    error: "Unauthorized".to_string(),
                    message: "Only channel parties can force close".to_string(),
                    timestamp: Utc::now(),
                }));
            }
            
            // Check if channel can be force closed
            if channel.status == "Closed" {
                return Ok(create_error_response(
                    "ChannelClosed",
                    "Channel is already closed"
                ));
            }
            
            // Mark channel as disputed
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
                    eprintln!("Error force closing channel: {}", e);
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
            eprintln!("Error fetching channel: {}", e);
            Ok(create_error_response(
                "DatabaseError",
                "Failed to fetch channel"
            ))
        }
    }
}

// Check for timeout expirations (admin function)
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
                // Force close the channel
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
            eprintln!("Error checking timeouts: {}", e);
            Ok(create_error_response(
                "DatabaseError",
                "Failed to check timeouts"
            ))
        }
    }
}

// ============================================================================
// PREVIOUS (AND RETURNED TO) MAIN SERVER
// ============================================================================

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("⚡ BSV Bank - Payment Channel Service Starting...");
    
    let pool = create_pool().await
        .expect("Failed to create database pool");
    
    println!("✅ Database connected");
    println!("✅ Service ready on http://0.0.0.0:8083");
    println!("📋 Endpoints:");
    println!("   GET  /health");
    println!("   POST /channels/open");
    println!("   POST /channels/{{id}}/payment");
    println!("   GET  /channels/{{id}}");
    println!("   GET  /channels/{{id}}/history");
    println!("   GET  /channels/{{id}}/balance");
    println!("   GET  /channels/user/{{paymail}}");
    println!("   POST /channels/{{id}}/close");
    println!("");
    
    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header();
            
        App::new()
            .wrap(cors)
            .app_data(web::Data::new(pool.clone()))
            .route("/health", web::get().to(health_check))
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
    .bind(("0.0.0.0", 8083))?
    .run()
    .await
}

// // ============================================================================
// // EXAMPLE 12: Main Application Setup
// // services/channel-service/src/main.rs (Updated - NO! merely proposed and likely to be rejected)
// // ============================================================================

// #[actix_web::main]
// async fn main() -> std::io::Result<()> {
//     env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    
//     println!("🚀 Starting Enhanced Channel Service (Phase 5)");
    
//     let state = web::Data::new(
//         AppState::new()
//             .await
//             .expect("Failed to initialize application state")
//     );
    
//     println!("✓ Database connected");
//     println!("✓ Blockchain services configured:");
//     println!("   - Monitor: {}", state.blockchain_config.blockchain_monitor_url);
//     println!("   - TX Builder: {}", state.blockchain_config.tx_builder_url);
//     println!("   - SPV Service: {}", state.blockchain_config.spv_service_url);
//     println!("   - Network: {}", state.blockchain_config.network);
//     println!("   - Blockchain enabled: {}", state.blockchain_config.enable_blockchain);
    
//     println!("✓ Server starting on http://127.0.0.1:8083");
    
//     HttpServer::new(move || {
//         let cors = Cors::default()
//             .allow_any_origin()
//             .allow_any_method()
//             .allow_any_header()
//             .max_age(3600);
        
//         App::new()
//             .wrap(cors)
//             .app_data(state.clone())
//             // Phase 5 enhanced endpoints
//             .route("/channels/create", web::post().to(create_channel_handler))
//             .route("/channels/{id}/status", web::get().to(get_channel_status_handler))
//             .route("/channels/{id}/close", web::post().to(close_channel_handler))
//             // Existing Phase 4 endpoints...
//             .route("/health", web::get().to(health_check))
//             .route("/channels/{id}", web::get().to(get_channel))
//             .route("/channels/{id}/pay", web::post().to(make_payment))
//     })
//     .bind("127.0.0.1:8083")?
//     .run()
//     .await
// }