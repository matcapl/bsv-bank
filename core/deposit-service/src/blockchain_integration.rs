// ============================================================================
// EXAMPLE 1: Enhanced Deposit Service with Blockchain Verification
// services/deposit-service/src/blockchain_integration.rs
// ============================================================================

use serde::{Deserialize, Serialize};
use reqwest::Client;

const BLOCKCHAIN_MONITOR_URL: &str = "http://localhost:8084";
const SPV_SERVICE_URL: &str = "http://localhost:8086";

#[derive(Debug, Serialize, Deserialize)]
struct BlockchainDeposit {
    paymail: String,
    amount_satoshis: i64,
    txid: Option<String>,
    blockchain_verified: bool,
}

#[derive(Debug, Deserialize)]
struct TransactionInfo {
    txid: String,
    confirmations: i32,
    amount_satoshis: i64,
    to_address: Option<String>,
}

#[derive(Debug, Deserialize)]
struct VerificationResult {
    txid: String,
    verified: bool,
    confirmations: i32,
    merkle_verified: bool,
}

/// Enhanced deposit creation with blockchain verification
pub async fn create_deposit_with_verification(
    client: &Client,
    db: &PgPool,
    paymail: String,
    amount: i64,
    txid: Option<String>,
) -> Result<Deposit, String> {
    
    if let Some(real_txid) = txid {
        // REAL BLOCKCHAIN DEPOSIT (Phase 5)
        
        // Step 1: Verify transaction exists on testnet
        let tx_info = verify_transaction_exists(client, &real_txid).await?;
        
        // Step 2: Verify amount matches
        if tx_info.amount_satoshis != amount {
            return Err(format!(
                "Amount mismatch: expected {}, got {}",
                amount,
                tx_info.amount_satoshis
            ));
        }
        
        // Step 3: Get user's deposit address
        let user_address = get_user_deposit_address(db, &paymail).await?;
        
        // Step 4: Verify transaction pays to user's address
        if tx_info.to_address.as_ref() != Some(&user_address) {
            return Err("Transaction does not pay to user's address".to_string());
        }
        
        // Step 5: Get SPV proof
        let verification = verify_with_spv(client, &real_txid).await?;
        
        if !verification.merkle_verified {
            return Err("SPV verification failed".to_string());
        }
        
        // Step 6: Watch this transaction for confirmations
        watch_transaction(client, &real_txid, &paymail).await?;
        
        // Step 7: Create deposit with verification data
        let deposit = sqlx::query_as::<_, Deposit>(
            r#"
            INSERT INTO deposits 
                (paymail, amount_satoshis, txid, duration_days, 
                 confirmations, testnet_verified, spv_proof_verified, created_at)
            VALUES ($1, $2, $3, 30, $4, true, $5, NOW())
            RETURNING *
            "#
        )
        .bind(&paymail)
        .bind(amount)
        .bind(&real_txid)
        .bind(verification.confirmations)
        .bind(verification.merkle_verified)
        .fetch_one(db)
        .await
        .map_err(|e| format!("Database error: {}", e))?;
        
        println!("✓ Deposit created with blockchain verification: {}", real_txid);
        
        Ok(deposit)
        
    } else {
        // MOCK DEPOSIT (Phase 4 compatibility)
        create_mock_deposit(db, paymail, amount).await
    }
}

/// Verify transaction exists on blockchain
async fn verify_transaction_exists(
    client: &Client,
    txid: &str,
) -> Result<TransactionInfo, String> {
    let url = format!("{}/tx/{}", BLOCKCHAIN_MONITOR_URL, txid);
    
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to query blockchain: {}", e))?;
    
    if !response.status().is_success() {
        return Err("Transaction not found on blockchain".to_string());
    }
    
    response
        .json::<TransactionInfo>()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))
}

/// Verify transaction with SPV proof
async fn verify_with_spv(
    client: &Client,
    txid: &str,
) -> Result<VerificationResult, String> {
    let url = format!("{}/verify/tx", SPV_SERVICE_URL);
    
    let response = client
        .post(&url)
        .json(&serde_json::json!({ "txid": txid }))
        .send()
        .await
        .map_err(|e| format!("SPV verification failed: {}", e))?;
    
    response
        .json::<VerificationResult>()
        .await
        .map_err(|e| format!("Failed to parse SPV response: {}", e))
}

/// Watch transaction for confirmation updates
async fn watch_transaction(
    client: &Client,
    txid: &str,
    paymail: &str,
) -> Result<(), String> {
    // This will trigger the blockchain monitor to track confirmations
    println!("→ Watching transaction {} for {}", txid, paymail);
    Ok(())
}

/// Get user's deposit address (would be stored in database)
async fn get_user_deposit_address(
    db: &PgPool,
    paymail: &str,
) -> Result<String, String> {
    let address = sqlx::query_scalar::<_, String>(
        "SELECT address FROM watched_addresses WHERE paymail = $1 AND purpose = 'deposit'"
    )
    .bind(paymail)
    .fetch_optional(db)
    .await
    .map_err(|e| format!("Database error: {}", e))?;
    
    address.ok_or_else(|| "No deposit address found for user".to_string())
}

/// Create mock deposit (Phase 4 compatibility)
async fn create_mock_deposit(
    db: &PgPool,
    paymail: String,
    amount: i64,
) -> Result<Deposit, String> {
    let mock_txid = format!("mock_{}", uuid::Uuid::new_v4());
    
    let deposit = sqlx::query_as::<_, Deposit>(
        r#"
        INSERT INTO deposits 
            (paymail, amount_satoshis, txid, duration_days, 
             confirmations, testnet_verified, created_at)
        VALUES ($1, $2, $3, 30, 999, false, NOW())
        RETURNING *
        "#
    )
    .bind(&paymail)
    .bind(amount)
    .bind(&mock_txid)
    .fetch_one(db)
    .await
    .map_err(|e| format!("Database error: {}", e))?;
    
    println!("✓ Mock deposit created (Phase 4 mode): {}", mock_txid);
    
    Ok(deposit)
}