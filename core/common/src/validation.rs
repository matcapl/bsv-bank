// core/common/src/validation.rs
// Comprehensive input validation

use regex::Regex;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Invalid paymail format: {0}")]
    InvalidPaymail(String),
    #[error("Invalid transaction ID format: {0}")]
    InvalidTxid(String),
    #[error("Invalid amount: {0}")]
    InvalidAmount(String),
    #[error("Invalid address: {0}")]
    InvalidAddress(String),
    #[error("Input exceeds maximum length of {0} characters")]
    ExceedsMaxLength(usize),
    #[error("Input too long: {field} exceeds {max} characters")]
    InputTooLong { field: String, max: usize },
    #[error("Suspicious input detected: {0}")]
    SuspiciousInput(String),
    #[error("XSS attempt detected: potentially dangerous content found")]
    XssAttempt,
    #[error("SQL injection attempt detected: potentially dangerous SQL pattern found")]
    SqlInjection,
    #[error("Invalid URL format: {0}")]
    InvalidUrl(String),
    #[error("Required field missing: {0}")]
    MissingField(String),
}

// Bitcoin SV constants
const MAX_SATOSHIS: i64 = 21_000_000_00_000_000; // 21 million BSV
const MIN_SATOSHIS: i64 = 0;

// Input length limits
const MAX_PAYMAIL_LENGTH: usize = 255;
const TXID_LENGTH: usize = 64;
const MAX_ADDRESS_LENGTH: usize = 100;

// ============================================================================
// SECURITY VALIDATORS (NEW - Phase 6)
// ============================================================================

/// XSS prevention
pub fn validate_no_xss(input: &str) -> Result<(), ValidationError> {
    let dangerous_patterns = [
        "<script", "javascript:", "onerror=", "onclick=",
        "onload=", "<iframe", "document.cookie", "eval(",
    ];
    
    let input_lower = input.to_lowercase();
    for pattern in &dangerous_patterns {
        if input_lower.contains(pattern) {
            return Err(ValidationError::XssAttempt);
        }
    }
    Ok(())
}

/// SQL injection prevention  
pub fn validate_no_sql_injection(input: &str) -> Result<(), ValidationError> {
    let sql_patterns = [
        "';", "--", "/*", "*/", "DROP ", "DELETE ",
        "INSERT ", "UPDATE ", "UNION ", "SELECT ",
        " OR ", " AND ", "1=1", "' OR '",
    ];
    
    let input_upper = input.to_uppercase();
    for pattern in &sql_patterns {
        if input_upper.contains(pattern) {
            return Err(ValidationError::SqlInjection);
        }
    }
    Ok(())
}

/// Length validation
pub fn validate_max_length(input: &str, max_length: usize) -> Result<(), ValidationError> {
    if input.len() > max_length {
        return Err(ValidationError::ExceedsMaxLength(max_length));
    }
    Ok(())
}

// ============================================================================
// BUSINESS VALIDATORS
// ============================================================================

/// Validate paymail address with security checks
pub fn validate_paymail(paymail: &str) -> Result<(), ValidationError> {
    // Length check
    validate_max_length(paymail, MAX_PAYMAIL_LENGTH)?;
    
    // Security checks
    validate_no_xss(paymail)?;
    validate_no_sql_injection(paymail)?;
    
    // Format check: must match user@domain.tld
    let re = Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$")
        .map_err(|_| ValidationError::InvalidPaymail("Regex compilation failed".to_string()))?;
    
    if !re.is_match(paymail) {
        return Err(ValidationError::InvalidPaymail(
            "Must be in format: user@domain.tld".to_string()
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
    
    if !txid.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(ValidationError::InvalidTxid(
            "must contain only hexadecimal characters".to_string()
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

/// Sanitize string input (remove control characters, limit length)
pub fn sanitize_string(input: &str, max_length: usize) -> String {
    input
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .take(max_length)
        .collect()
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Security tests
    #[test]
    fn test_validate_no_xss_safe() {
        assert!(validate_no_xss("Hello World").is_ok());
        assert!(validate_no_xss("user@example.com").is_ok());
    }
    
    #[test]
    fn test_validate_no_xss_dangerous() {
        assert!(validate_no_xss("<script>alert('xss')</script>").is_err());
        assert!(validate_no_xss("javascript:alert(1)").is_err());
        assert!(validate_no_xss("<img onerror='alert(1)'>").is_err());
    }
    
    #[test]
    fn test_validate_no_sql_injection_safe() {
        assert!(validate_no_sql_injection("John Doe").is_ok());
        assert!(validate_no_sql_injection("user@example.com").is_ok());
    }
    
    #[test]
    fn test_validate_no_sql_injection_dangerous() {
        assert!(validate_no_sql_injection("'; DROP TABLE users--").is_err());
        assert!(validate_no_sql_injection("1' OR '1'='1").is_err());
        assert!(validate_no_sql_injection("admin'--").is_err());
    }
    
    // Paymail tests
    #[test]
    fn test_validate_paymail_valid() {
        assert!(validate_paymail("user@example.com").is_ok());
        assert!(validate_paymail("test.user@domain.co.uk").is_ok());
    }
    
    #[test]
    fn test_validate_paymail_invalid() {
        assert!(validate_paymail("notanemail").is_err());
        assert!(validate_paymail("@example.com").is_err());
        assert!(validate_paymail("user@").is_err());
        assert!(validate_paymail("<script>@example.com").is_err());
    }
    
    // TXID tests
    #[test]
    fn test_validate_txid_valid() {
        let valid_txid = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        assert!(validate_txid(valid_txid).is_ok());
    }
    
    #[test]
    fn test_validate_txid_invalid() {
        assert!(validate_txid("tooshort").is_err());
        assert!(validate_txid(&"a".repeat(65)).is_err());
        assert!(validate_txid(&"g".repeat(64)).is_err());
    }
    
    // Amount tests
    #[test]
    fn test_validate_amount_valid() {
        assert!(validate_amount(100).is_ok());
        assert!(validate_amount(1_000_000).is_ok());
    }
    
    #[test]
    fn test_validate_amount_invalid() {
        assert!(validate_amount(0).is_err());
        assert!(validate_amount(-100).is_err());
        assert!(validate_amount(MAX_SATOSHIS + 1).is_err());
    }
    
    // Address tests
    #[test]
    fn test_valid_address() {
        assert!(validate_address("1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa").is_ok());
        assert!(validate_address("mipcBbFg9gMiCh81Kj8tqqdgoZub1ZJRfn").is_ok());
    }
    
    #[test]
    fn test_invalid_address() {
        assert!(validate_address("").is_err());
        assert!(validate_address("X1234567890").is_err());
    }
    
    // Sanitize tests
    #[test]
    fn test_sanitize_string() {
        let input = "hello\x00world\x01test";
        let sanitized = sanitize_string(input, 100);
        assert!(!sanitized.contains('\x00'));
    }
}