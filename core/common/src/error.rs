// core/common/src/error.rs
// Standardized error responses and handling

use actix_web::{error::ResponseError, http::StatusCode, HttpResponse};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub error_code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

impl ErrorResponse {
    pub fn new(
        error: String,
        error_code: String,
        message: String,
    ) -> Self {
        Self {
            error,
            error_code,
            message,
            details: None,
            request_id: None,
        }
    }
    
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
    
    pub fn with_request_id(mut self, request_id: String) -> Self {
        self.request_id = Some(request_id);
        self
    }
}

#[derive(Debug)]
pub enum ServiceError {
    // Client errors (4xx)
    ValidationError(String),
    NotFound(String),
    Unauthorized,
    Forbidden,
    Conflict(String),
    RateLimitExceeded(String),
    BadRequest(String),
    
    // Server errors (5xx)
    DatabaseError(String),
    ExternalServiceError(String),
    InternalError(String),
    
    // Custom errors
    Custom {
        status_code: StatusCode,
        error_code: String,
        message: String,
    },
}

impl ServiceError {
    pub fn error_code(&self) -> String {
        match self {
            ServiceError::ValidationError(_) => "validation_error".to_string(),
            ServiceError::NotFound(_) => "not_found".to_string(),
            ServiceError::Unauthorized => "unauthorized".to_string(),
            ServiceError::Forbidden => "forbidden".to_string(),
            ServiceError::Conflict(_) => "conflict".to_string(),
            ServiceError::RateLimitExceeded(_) => "rate_limit_exceeded".to_string(),
            ServiceError::BadRequest(_) => "bad_request".to_string(),
            ServiceError::DatabaseError(_) => "database_error".to_string(),
            ServiceError::ExternalServiceError(_) => "external_service_error".to_string(),
            ServiceError::InternalError(_) => "internal_error".to_string(),
            ServiceError::Custom { error_code, .. } => error_code.clone(),
        }
    }
    
    pub fn message(&self) -> String {
        match self {
            ServiceError::ValidationError(msg) => msg.clone(),
            ServiceError::NotFound(msg) => msg.clone(),
            ServiceError::Unauthorized => "Unauthorized access".to_string(),
            ServiceError::Forbidden => "Access forbidden".to_string(),
            ServiceError::Conflict(msg) => msg.clone(),
            ServiceError::RateLimitExceeded(msg) => msg.clone(),
            ServiceError::BadRequest(msg) => msg.clone(),
            ServiceError::DatabaseError(msg) => format!("Database error: {}", msg),
            ServiceError::ExternalServiceError(msg) => format!("External service error: {}", msg),
            ServiceError::InternalError(msg) => format!("Internal error: {}", msg),
            ServiceError::Custom { message, .. } => message.clone(),
        }
    }
    
    pub fn status_code(&self) -> StatusCode {
        match self {
            ServiceError::ValidationError(_) => StatusCode::BAD_REQUEST,
            ServiceError::NotFound(_) => StatusCode::NOT_FOUND,
            ServiceError::Unauthorized => StatusCode::UNAUTHORIZED,
            ServiceError::Forbidden => StatusCode::FORBIDDEN,
            ServiceError::Conflict(_) => StatusCode::CONFLICT,
            ServiceError::RateLimitExceeded(_) => StatusCode::TOO_MANY_REQUESTS,
            ServiceError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ServiceError::DatabaseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ServiceError::ExternalServiceError(_) => StatusCode::BAD_GATEWAY,
            ServiceError::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ServiceError::Custom { status_code, .. } => *status_code,
        }
    }
    
    pub fn to_error_response(&self, request_id: Option<String>) -> ErrorResponse {
        let mut response = ErrorResponse::new(
            self.to_string(),
            self.error_code(),
            self.message(),
        );
        
        if let Some(id) = request_id {
            response = response.with_request_id(id);
        }
        
        response
    }
}

impl fmt::Display for ServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message())
    }
}

impl ResponseError for ServiceError {
    fn error_response(&self) -> HttpResponse {
        let error_response = self.to_error_response(None);
        HttpResponse::build(self.status_code()).json(error_response)
    }
    
    fn status_code(&self) -> StatusCode {
        self.status_code()
    }
}

// Conversion from common error types
impl From<sqlx::Error> for ServiceError {
    fn from(err: sqlx::Error) -> Self {
        ServiceError::DatabaseError(err.to_string())
    }
}

impl From<crate::auth::AuthError> for ServiceError {
    fn from(err: crate::auth::AuthError) -> Self {
        match err {
            crate::auth::AuthError::InvalidToken => ServiceError::Unauthorized,
            crate::auth::AuthError::TokenExpired => ServiceError::Unauthorized,
            crate::auth::AuthError::MissingAuth => ServiceError::Unauthorized,
            crate::auth::AuthError::InsufficientPermissions => ServiceError::Forbidden,
            crate::auth::AuthError::JwtError(_) => ServiceError::Unauthorized,
        }
    }
}

impl From<crate::validation::ValidationError> for ServiceError {
    fn from(err: crate::validation::ValidationError) -> Self {
        ServiceError::ValidationError(err.to_string())
    }
}

impl From<crate::rate_limit::RateLimitError> for ServiceError {
    fn from(err: crate::rate_limit::RateLimitError) -> Self {
        match err {
            crate::rate_limit::RateLimitError::LimitExceeded(msg) => {
                ServiceError::RateLimitExceeded(msg)
            }
            crate::rate_limit::RateLimitError::InternalError(msg) => {
                ServiceError::InternalError(msg)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_error_response_creation() {
        let error = ErrorResponse::new(
            "Test error".to_string(),
            "test_error".to_string(),
            "This is a test error".to_string(),
        );
        
        assert_eq!(error.error, "Test error");
        assert_eq!(error.error_code, "test_error");
        assert_eq!(error.message, "This is a test error");
        assert!(error.details.is_none());
        assert!(error.request_id.is_none());
    }
    
    #[test]
    fn test_error_response_with_details() {
        let error = ErrorResponse::new(
            "Test error".to_string(),
            "test_error".to_string(),
            "This is a test error".to_string(),
        ).with_details(serde_json::json!({"field": "value"}));
        
        assert!(error.details.is_some());
    }
    
    #[test]
    fn test_error_response_with_request_id() {
        let error = ErrorResponse::new(
            "Test error".to_string(),
            "test_error".to_string(),
            "This is a test error".to_string(),
        ).with_request_id("req-123".to_string());
        
        assert_eq!(error.request_id, Some("req-123".to_string()));
    }
    
    #[test]
    fn test_service_error_validation() {
        let error = ServiceError::ValidationError("Invalid input".to_string());
        
        assert_eq!(error.error_code(), "validation_error");
        assert_eq!(error.status_code(), StatusCode::BAD_REQUEST);
        assert_eq!(error.message(), "Invalid input");
    }
    
    #[test]
    fn test_service_error_not_found() {
        let error = ServiceError::NotFound("Resource not found".to_string());
        
        assert_eq!(error.error_code(), "not_found");
        assert_eq!(error.status_code(), StatusCode::NOT_FOUND);
    }
    
    #[test]
    fn test_service_error_unauthorized() {
        let error = ServiceError::Unauthorized;
        
        assert_eq!(error.error_code(), "unauthorized");
        assert_eq!(error.status_code(), StatusCode::UNAUTHORIZED);
    }
    
    #[test]
    fn test_service_error_rate_limit() {
        let error = ServiceError::RateLimitExceeded("Too many requests".to_string());
        
        assert_eq!(error.error_code(), "rate_limit_exceeded");
        assert_eq!(error.status_code(), StatusCode::TOO_MANY_REQUESTS);
    }
    
    #[test]
    fn test_service_error_database() {
        let error = ServiceError::DatabaseError("Connection failed".to_string());
        
        assert_eq!(error.error_code(), "database_error");
        assert_eq!(error.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
    }
    
    #[test]
    fn test_service_error_custom() {
        let error = ServiceError::Custom {
            status_code: StatusCode::PAYMENT_REQUIRED,
            error_code: "insufficient_funds".to_string(),
            message: "Insufficient funds".to_string(),
        };
        
        assert_eq!(error.error_code(), "insufficient_funds");
        assert_eq!(error.status_code(), StatusCode::PAYMENT_REQUIRED);
        assert_eq!(error.message(), "Insufficient funds");
    }
    
    #[test]
    fn test_to_error_response() {
        let error = ServiceError::ValidationError("Invalid paymail".to_string());
        let response = error.to_error_response(Some("req-456".to_string()));
        
        assert_eq!(response.error_code, "validation_error");
        assert_eq!(response.request_id, Some("req-456".to_string()));
    }
    
    #[test]
    fn test_error_response_serialization() {
        let error = ErrorResponse::new(
            "Test error".to_string(),
            "test_error".to_string(),
            "This is a test".to_string(),
        );
        
        let json = serde_json::to_string(&error).unwrap();
        assert!(json.contains("\"error\":\"Test error\""));
        assert!(json.contains("\"error_code\":\"test_error\""));
    }
    
    #[test]
    fn test_error_display() {
        let error = ServiceError::ValidationError("Test message".to_string());
        assert_eq!(format!("{}", error), "Test message");
    }
}