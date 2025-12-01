// core/common/src/validation.rs
// Comprehensive input validation

use regex::Regex;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Invalid paymail: {0}")]
    InvalidPaymail(String),
    #[error("Invalid transaction ID: {0}")]
    InvalidTxid(String),
    #[error("Invalid amount: {0}")]
    InvalidAmount(String),
    #[error("Invalid address: {0}")]
    InvalidAddress(String),
    #[error("Input too long: {field} exceeds {max} characters")]
    InputTooLong { field: String, max: usize },
    #[error("Suspicious input detected: {0}")]
    SuspiciousInput(String),
}

// Bitcoin SV constants
const MAX_SATOSHIS: i64 = 21_000_000_00_000_000; // 21 million BSV
const MIN_SATOSHIS: i64 = 0;

// Input length limits
const MAX_PAYMAIL_LENGTH: usize = 255;
const TXID_LENGTH: usize = 64;
const MAX_ADDRESS_LENGTH: usize = 100;

/// Validate paymail address
pub fn validate_paymail(paymail: &str) -> Result<(), ValidationError> {
    // Check length
    if paymail.len() > MAX_PAYMAIL_LENGTH {
        return Err(ValidationError::InputTooLong {
            field: "paymail".to_string(),
            max: MAX_PAYMAIL_LENGTH,
        });
    }
    
    // Must contain exactly one @
    if paymail.matches('@').count() != 1 {
        return Err(ValidationError::InvalidPaymail(
            "must contain exactly one @ symbol".to_string(),
        ));
    }
    
    // Basic format validation
    let paymail_regex = Regex::new(r"^[a-zA-Z0-9._-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$")
        .map_err(|_| ValidationError::InvalidPaymail("regex error".to_string()))?;
    
    if !paymail_regex.is_match(paymail) {
        return Err(ValidationError::InvalidPaymail(
            "invalid format".to_string(),
        ));
    }
    
    // Check for suspicious patterns
    if contains_suspicious_chars(paymail) {
        return Err(ValidationError::SuspiciousInput(
            "paymail contains suspicious characters".to_string(),
        ));
    }
    
    // Check for SQL injection patterns
    if contains_sql_injection(paymail) {
        return Err(ValidationError::SuspiciousInput(
            "paymail contains SQL injection pattern".to_string(),
        ));
    }
    
    // Check for XSS patterns
    if contains_xss(paymail) {
        return Err(ValidationError::SuspiciousInput(
            "paymail contains XSS pattern".to_string(),
        ));
    }
    
    Ok(())
}

/// Validate transaction ID (64 hex characters)
pub fn validate_txid(txid: &str) -> Result<(), ValidationError> {
    if txid.len() != TXID_LENGTH {
        return Err(ValidationError::InvalidTxid(format!(
            "must be exactly {} characters",
            TXID_LENGTH
        )));
    }
    
    // Must be valid hexadecimal
    let hex_regex = Regex::new(r"^[0-9a-fA-F]{64}$")
        .map_err(|_| ValidationError::InvalidTxid("regex error".to_string()))?;
    
    if !hex_regex.is_match(txid) {
        return Err(ValidationError::InvalidTxid(
            "must contain only hexadecimal characters".to_string(),
        ));
    }
    
    Ok(())
}

/// Validate satoshi amount
pub fn validate_amount(satoshis: i64) -> Result<(), ValidationError> {
    if satoshis < MIN_SATOSHIS {
        return Err(ValidationError::InvalidAmount(
            "amount cannot be negative".to_string(),
        ));
    }
    
    if satoshis == 0 {
        return Err(ValidationError::InvalidAmount(
            "amount must be greater than zero".to_string(),
        ));
    }
    
    if satoshis > MAX_SATOSHIS {
        return Err(ValidationError::InvalidAmount(format!(
            "amount exceeds maximum supply ({})",
            MAX_SATOSHIS
        )));
    }
    
    Ok(())
}

/// Validate Bitcoin address (testnet or mainnet)
pub fn validate_address(address: &str) -> Result<(), ValidationError> {
    if address.is_empty() {
        return Err(ValidationError::InvalidAddress("address is empty".to_string()));
    }
    
    if address.len() > MAX_ADDRESS_LENGTH {
        return Err(ValidationError::InputTooLong {
            field: "address".to_string(),
            max: MAX_ADDRESS_LENGTH,
        });
    }
    
    // Check for testnet or mainnet prefix
    if !address.starts_with('1')
        && !address.starts_with('3')
        && !address.starts_with("bc1")
        && !address.starts_with('m')
        && !address.starts_with('n')
        && !address.starts_with('2')
        && !address.starts_with("tb1")
    {
        return Err(ValidationError::InvalidAddress(
            "invalid address prefix".to_string(),
        ));
    }
    
    // Basic character validation (base58 or bech32)
    let valid_chars_regex = Regex::new(r"^[123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz]+$|^(bc1|tb1)[a-z0-9]{39,87}$")
        .map_err(|_| ValidationError::InvalidAddress("regex error".to_string()))?;
    
    if !valid_chars_regex.is_match(address) {
        return Err(ValidationError::InvalidAddress(
            "contains invalid characters".to_string(),
        ));
    }
    
    Ok(())
}

/// Check for suspicious characters that might indicate an attack
fn contains_suspicious_chars(input: &str) -> bool {
    let suspicious_patterns = [
        "<", ">",           // HTML tags
        "script",           // Script tags
        "javascript:",      // JavaScript protocol
        "onerror",          // Event handlers
        "onload",
        "onclick",
        "../",              // Path traversal
        "..\\",
    ];
    
    let input_lower = input.to_lowercase();
    suspicious_patterns.iter().any(|&pattern| input_lower.contains(pattern))
}

/// Check for SQL injection patterns
fn contains_sql_injection(input: &str) -> bool {
    let sql_patterns = [
        "' or '1'='1",
        "' or 1=1",
        "\" or \"1\"=\"1",
        "' or true--",
        "'; drop table",
        "'; delete from",
        "union select",
        "insert into",
        "update set",
    ];
    
    let input_lower = input.to_lowercase();
    sql_patterns.iter().any(|&pattern| input_lower.contains(pattern))
}

/// Check for XSS patterns
fn contains_xss(input: &str) -> bool {
    let xss_patterns = [
        "<script",
        "</script>",
        "javascript:",
        "onerror=",
        "onload=",
        "onclick=",
        "<iframe",
        "<embed",
        "<object",
    ];
    
    let input_lower = input.to_lowercase();
    xss_patterns.iter().any(|&pattern| input_lower.contains(pattern))
}

/// Sanitize string input (remove control characters, limit length)
pub fn sanitize_string(input: &str, max_length: usize) -> String {
    input
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .take(max_length)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_valid_paymail() {
        assert!(validate_paymail("user@domain.com").is_ok());
        assert!(validate_paymail("test.user@example.com").is_ok());
        assert!(validate_paymail("user123@test-domain.com").is_ok());
    }
    
    #[test]
    fn test_invalid_paymail_no_at() {
        assert!(validate_paymail("invaliddomain.com").is_err());
    }
    
    #[test]
    fn test_invalid_paymail_multiple_at() {
        assert!(validate_paymail("user@@domain.com").is_err());
    }
    
    #[test]
    fn test_invalid_paymail_xss() {
        assert!(validate_paymail("user<script>@domain.com").is_err());
        assert!(validate_paymail("user@domain.com<script>").is_err());
    }
    
    #[test]
    fn test_invalid_paymail_sql_injection() {
        assert!(validate_paymail("user' or '1'='1@domain.com").is_err());
    }
    
    #[test]
    fn test_paymail_too_long() {
        let long_paymail = format!("{}@domain.com", "a".repeat(300));
        assert!(validate_paymail(&long_paymail).is_err());
    }
    
    #[test]
    fn test_valid_txid() {
        let txid = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        assert!(validate_txid(txid).is_ok());
    }
    
    #[test]
    fn test_invalid_txid_length() {
        assert!(validate_txid("short").is_err());
    }
    
    #[test]
    fn test_invalid_txid_non_hex() {
        let txid = "xyz123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        assert!(validate_txid(txid).is_err());
    }
    
    #[test]
    fn test_valid_amount() {
        assert!(validate_amount(1000).is_ok());
        assert!(validate_amount(100_000_000).is_ok());
        assert!(validate_amount(1_000_000_000_000).is_ok());
    }
    
    #[test]
    fn test_invalid_amount_negative() {
        assert!(validate_amount(-1000).is_err());
    }
    
    #[test]
    fn test_invalid_amount_zero() {
        assert!(validate_amount(0).is_err());
    }
    
    #[test]
    fn test_invalid_amount_exceeds_max() {
        assert!(validate_amount(MAX_SATOSHIS + 1).is_err());
    }
    
    #[test]
    fn test_valid_address() {
        assert!(validate_address("1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa").is_ok()); // mainnet
        assert!(validate_address("mipcBbFg9gMiCh81Kj8tqqdgoZub1ZJRfn").is_ok()); // testnet
    }
    
    #[test]
    fn test_invalid_address_empty() {
        assert!(validate_address("").is_err());
    }
    
    #[test]
    fn test_invalid_address_prefix() {
        assert!(validate_address("X1234567890").is_err());
    }
    
    #[test]
    fn test_sanitize_string() {
        let input = "hello\x00world\x01test";
        let sanitized = sanitize_string(input, 100);
        assert!(!sanitized.contains('\x00'));
        assert!(!sanitized.contains('\x01'));
    }
    
    #[test]
    fn test_sanitize_string_length() {
        let input = "a".repeat(1000);
        let sanitized = sanitize_string(&input, 100);
        assert_eq!(sanitized.len(), 100);
    }
}