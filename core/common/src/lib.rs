// core/common/src/lib.rs
// BSV Bank Common Library - Shared functionality across all services

pub mod auth;
pub mod validation;
pub mod rate_limit;
pub mod health;
pub mod logging;
pub mod metrics;
pub mod error;
pub mod middleware;

// Re-export commonly used items
pub use auth::{AuthError, Claims, JwtManager};
pub use validation::{
    validate_address, validate_amount, validate_paymail, validate_txid, 
    validate_no_xss, validate_no_sql_injection, validate_max_length, ValidationError,
};
pub use rate_limit::{RateLimit, RateLimiter, RateLimitError, RateLimitInfo, start_cleanup_task};
pub use health::{
    check_database_health, check_external_api_health, HealthResponse, HealthStatus,
    DependencyHealth, LivenessProbe, ReadinessProbe,
};
pub use logging::{
    generate_request_id, init_logging, init_console_logging, LogContext,
    log_success, log_failure, log_validation_error, log_auth_attempt,
};
pub use metrics::{
    ServiceMetrics, MetricsTimer, DepositMetrics, LendingMetrics, ChannelMetrics,
    // http_request_counter, http_request_duration ( should now be part of ServiceMetrcs(?) ) 
};
pub use error::{ErrorResponse, ServiceError};
pub use middleware::{RateLimitMiddleware, configure_rate_limits};

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_library_imports() {
        // Test that all modules are accessible
        let _ = JwtManager::new("test".to_string());
        let _ = RateLimiter::new();
        let _ = generate_request_id();
    }
}