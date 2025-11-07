use regex::Regex;

pub fn validate_paymail(paymail: &str) -> Result<(), String> {
    // Paymail format: handle@domain.tld
    let re = Regex::new(r"^[a-zA-Z0-9._-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap();
    
    if !re.is_match(paymail) {
        return Err("Invalid paymail format".to_string());
    }
    
    if paymail.len() > 255 {
        return Err("Paymail too long".to_string());
    }
    
    // Block suspicious characters
    let suspicious_chars = ['<', '>', '"', '\'', ';', '(', ')', '{', '}', '[', ']'];
    if paymail.chars().any(|c| suspicious_chars.contains(&c)) {
        return Err("Paymail contains invalid characters".to_string());
    }
    
    Ok(())
}

pub fn validate_txid(txid: &str) -> Result<(), String> {
    if txid.len() != 64 {
        return Err("Transaction ID must be 64 characters".to_string());
    }
    
    // Must be hexadecimal
    if !txid.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("Transaction ID must be hexadecimal".to_string());
    }
    
    Ok(())
}

pub fn validate_amount(satoshis: i64) -> Result<(), String> {
    if satoshis <= 0 {
        return Err("Amount must be positive".to_string());
    }
    
    if satoshis > 21_000_000 * 100_000_000 {
        return Err("Amount exceeds maximum supply".to_string());
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_valid_paymail() {
        assert!(validate_paymail("user@handcash.io").is_ok());
        assert!(validate_paymail("test.user@example.com").is_ok());
    }
    
    #[test]
    fn test_invalid_paymail() {
        assert!(validate_paymail("<script>@xss.com").is_err());
        assert!(validate_paymail("user'; DROP TABLE--@hack.com").is_err());
        assert!(validate_paymail("invalid").is_err());
    }
    
    #[test]
    fn test_valid_txid() {
        assert!(validate_txid(&"a".repeat(64)).is_ok());
        assert!(validate_txid(&"1234567890abcdef".repeat(4)).is_ok());
    }
    
    #[test]
    fn test_invalid_txid() {
        assert!(validate_txid("short").is_err());
        assert!(validate_txid(&"z".repeat(64)).is_err());
    }
}
