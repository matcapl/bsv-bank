// core/common/src/logging.rs
// Structured JSON logging with correlation IDs

use tracing::{info, warn, error, debug};
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter,
};
// use std::sync::Arc;
use uuid::Uuid;

/// Initialize structured logging for a service
pub fn init_logging(service_name: &str) {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));
    
    tracing_subscriber::registry()
        .with(env_filter)
        .with(
            fmt::layer()
                .json()
                .with_target(true)
                .with_thread_ids(true)
                .with_thread_names(true)
                .with_span_events(FmtSpan::CLOSE)
                .with_current_span(true)
        )
        .init();
    
    info!(
        service = service_name,
        "Logging initialized"
    );
}

/// Initialize simple console logging (for development)
pub fn init_console_logging(service_name: &str) {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));
    
    tracing_subscriber::registry()
        .with(env_filter)
        .with(
            fmt::layer()
                .pretty()
                .with_target(true)
        )
        .init();
    
    info!(
        service = service_name,
        "Console logging initialized"
    );
}

/// Generate a correlation ID for request tracing
pub fn generate_request_id() -> String {
    Uuid::new_v4().to_string()
}

/// Context for structured logging
#[derive(Debug, Clone)]
pub struct LogContext {
    pub request_id: String,
    pub user_paymail: Option<String>,
    pub ip_address: Option<String>,
}

impl LogContext {
    pub fn new(request_id: String) -> Self {
        Self {
            request_id,
            user_paymail: None,
            ip_address: None,
        }
    }
    
    pub fn with_user(mut self, paymail: String) -> Self {
        self.user_paymail = Some(paymail);
        self
    }
    
    pub fn with_ip(mut self, ip: String) -> Self {
        self.ip_address = Some(ip);
        self
    }
}

/// Macro for logging with context
#[macro_export]
macro_rules! log_with_context {
    ($level:expr, $ctx:expr, $($arg:tt)*) => {
        match $level {
            tracing::Level::ERROR => tracing::error!(
                request_id = %$ctx.request_id,
                user_paymail = ?$ctx.user_paymail,
                ip_address = ?$ctx.ip_address,
                $($arg)*
            ),
            tracing::Level::WARN => tracing::warn!(
                request_id = %$ctx.request_id,
                user_paymail = ?$ctx.user_paymail,
                ip_address = ?$ctx.ip_address,
                $($arg)*
            ),
            tracing::Level::INFO => tracing::info!(
                request_id = %$ctx.request_id,
                user_paymail = ?$ctx.user_paymail,
                ip_address = ?$ctx.ip_address,
                $($arg)*
            ),
            tracing::Level::DEBUG => tracing::debug!(
                request_id = %$ctx.request_id,
                user_paymail = ?$ctx.user_paymail,
                ip_address = ?$ctx.ip_address,
                $($arg)*
            ),
            _ => tracing::trace!(
                request_id = %$ctx.request_id,
                user_paymail = ?$ctx.user_paymail,
                ip_address = ?$ctx.ip_address,
                $($arg)*
            ),
        }
    };
}

/// Log a successful operation
pub fn log_success(ctx: &LogContext, action: &str, details: Option<&str>) {
    info!(
        request_id = %ctx.request_id,
        user_paymail = ?ctx.user_paymail,
        ip_address = ?ctx.ip_address,
        action = action,
        details = ?details,
        "Operation successful"
    );
}

/// Log a failed operation
pub fn log_failure(ctx: &LogContext, action: &str, error: &str) {
    error!(
        request_id = %ctx.request_id,
        user_paymail = ?ctx.user_paymail,
        ip_address = ?ctx.ip_address,
        action = action,
        error = error,
        "Operation failed"
    );
}

/// Log a validation error
pub fn log_validation_error(ctx: &LogContext, field: &str, value: &str, reason: &str) {
    warn!(
        request_id = %ctx.request_id,
        user_paymail = ?ctx.user_paymail,
        ip_address = ?ctx.ip_address,
        field = field,
        value = value,
        reason = reason,
        "Validation error"
    );
}

/// Log authentication attempt
pub fn log_auth_attempt(ctx: &LogContext, paymail: &str, success: bool) {
    if success {
        info!(
            request_id = %ctx.request_id,
            user_paymail = paymail,
            ip_address = ?ctx.ip_address,
            "Authentication successful"
        );
    } else {
        warn!(
            request_id = %ctx.request_id,
            attempted_paymail = paymail,
            ip_address = ?ctx.ip_address,
            "Authentication failed"
        );
    }
}

/// Log rate limit exceeded
pub fn log_rate_limit_exceeded(ctx: &LogContext, endpoint: &str, key: &str) {
    warn!(
        request_id = %ctx.request_id,
        ip_address = ?ctx.ip_address,
        endpoint = endpoint,
        rate_limit_key = key,
        "Rate limit exceeded"
    );
}

/// Log database operation
pub fn log_database_operation(
    ctx: &LogContext,
    operation: &str,
    table: &str,
    duration_ms: u64,
    success: bool,
) {
    if success {
        debug!(
            request_id = %ctx.request_id,
            operation = operation,
            table = table,
            duration_ms = duration_ms,
            "Database operation successful"
        );
    } else {
        error!(
            request_id = %ctx.request_id,
            operation = operation,
            table = table,
            duration_ms = duration_ms,
            "Database operation failed"
        );
    }
}

/// Log external API call
pub fn log_external_api_call(
    ctx: &LogContext,
    api_name: &str,
    url: &str,
    duration_ms: u64,
    status_code: u16,
) {
    info!(
        request_id = %ctx.request_id,
        api_name = api_name,
        url = url,
        duration_ms = duration_ms,
        status_code = status_code,
        "External API call"
    );
}

/// Sanitize sensitive data for logging (redact private keys, passwords, etc.)
pub fn sanitize_for_logging(input: &str) -> String {
    if input.len() > 100 {
        format!("{}...[REDACTED]", &input[..20])
    } else if input.contains("private") || input.contains("password") || input.contains("secret") {
        "[REDACTED]".to_string()
    } else {
        input.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_generate_request_id() {
        let id1 = generate_request_id();
        let id2 = generate_request_id();
        
        assert_ne!(id1, id2);
        assert_eq!(id1.len(), 36); // UUID v4 length
    }
    
    #[test]
    fn test_log_context_creation() {
        let ctx = LogContext::new("test-123".to_string());
        
        assert_eq!(ctx.request_id, "test-123");
        assert!(ctx.user_paymail.is_none());
        assert!(ctx.ip_address.is_none());
    }
    
    #[test]
    fn test_log_context_with_user() {
        let ctx = LogContext::new("test-123".to_string())
            .with_user("user@example.com".to_string());
        
        assert_eq!(ctx.user_paymail, Some("user@example.com".to_string()));
    }
    
    #[test]
    fn test_log_context_with_ip() {
        let ctx = LogContext::new("test-123".to_string())
            .with_ip("192.168.1.1".to_string());
        
        assert_eq!(ctx.ip_address, Some("192.168.1.1".to_string()));
    }
    
    #[test]
    fn test_log_context_chaining() {
        let ctx = LogContext::new("test-123".to_string())
            .with_user("user@example.com".to_string())
            .with_ip("192.168.1.1".to_string());
        
        assert_eq!(ctx.request_id, "test-123");
        assert_eq!(ctx.user_paymail, Some("user@example.com".to_string()));
        assert_eq!(ctx.ip_address, Some("192.168.1.1".to_string()));
    }
    
    #[test]
    fn test_sanitize_for_logging_short() {
        let input = "short value";
        let sanitized = sanitize_for_logging(input);
        assert_eq!(sanitized, "short value");
    }
    
    #[test]
    fn test_sanitize_for_logging_long() {
        let input = "a".repeat(200);
        let sanitized = sanitize_for_logging(&input);
        assert!(sanitized.contains("[REDACTED]"));
        assert!(sanitized.len() < input.len());
    }
    
    #[test]
    fn test_sanitize_for_logging_sensitive() {
        let inputs = vec![
            "my_private_key_here",
            "password123",
            "secret_token",
        ];
        
        for input in inputs {
            let sanitized = sanitize_for_logging(input);
            assert_eq!(sanitized, "[REDACTED]");
        }
    }
    
    #[test]
    fn test_sanitize_for_logging_normal() {
        let input = "user@example.com";
        let sanitized = sanitize_for_logging(input);
        assert_eq!(sanitized, "user@example.com");
    }
}