// core/common/src/auth.rs
// JWT Authentication and Authorization

use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation}; // Algorithm, 
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Invalid token")]
    InvalidToken,
    #[error("Token expired")]
    TokenExpired,
    #[error("Missing authorization header")]
    MissingAuth,
    #[error("Insufficient permissions")]
    InsufficientPermissions,
    #[error("JWT error: {0}")]
    JwtError(#[from] jsonwebtoken::errors::Error),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,                    // Subject (paymail)
    pub exp: usize,                     // Expiration time
    pub iat: usize,                     // Issued at
    pub permissions: Vec<String>,       // User permissions
}

impl Claims {
    pub fn new(paymail: String, permissions: Vec<String>, ttl_hours: u64) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as usize;
        
        Self {
            sub: paymail,
            exp: now + (ttl_hours * 3600) as usize,
            iat: now,
            permissions,
        }
    }
    
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as usize;
        
        self.exp < now
    }
    
    pub fn has_permission(&self, required_permission: &str) -> bool {
        self.permissions.contains(&required_permission.to_string())
            || self.permissions.contains(&"admin".to_string())
    }
}

#[derive(Clone)]
pub struct JwtManager {
    secret: String,
}

impl JwtManager {
    pub fn new(secret: String) -> Self {
        Self { secret }
    }
    
    /// Create a new JWT token
    pub fn create_token(
        &self,
        paymail: &str,
        permissions: Vec<String>,
        ttl_hours: u64,
    ) -> Result<String, AuthError> {
        let claims = Claims::new(paymail.to_string(), permissions, ttl_hours);
        
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.secret.as_bytes()),
        )?;
        
        Ok(token)
    }

    // // Chatgpt attempt - aborted
    // pub fn create_token_with_expiration(
    //     &self,
    //     user: &str,
    //     roles: Vec<String>,
    //     exp_time: chrono::DateTime<chrono::Utc>,
    // ) -> Result<String, AuthError> {
    //     use jsonwebtoken::{encode, EncodingKey, Header};
    //     use chrono::TimeZone;

    //     let claims = Claims {
    //         sub: user.to_string(),
    //         roles,
    //         exp: exp_time.timestamp() as usize,
    //     };

    //     encode(&Header::default(), &claims, &EncodingKey::from_secret(self.secret.as_ref()))
    //         .map_err(|e| AuthError::TokenCreationFailed(e.to_string()))
    // }
    
    /// Verify and decode a JWT token
    // pub fn verify_token(&self, token: &str) -> Result<Claims, AuthError> {
    //     let token_data = decode::<Claims>(
    //         token,
    //         &DecodingKey::from_secret(self.secret.as_bytes()),
    //         &Validation::new(Algorithm::HS256),
    //     )?;
        
    //     let claims = token_data.claims;
        
    //     if claims.is_expired() {
    //         return Err(AuthError::TokenExpired);
    //     }
        
    //     Ok(claims)
    // }

    pub fn verify_token(&self, token: &str) -> Result<Claims, AuthError> {
        let validation = Validation::default();
        
        match decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.secret.as_ref()),
            &validation,
        ) {
            Ok(token_data) => Ok(token_data.claims),
            Err(err) => {
                // Check specific error type
                use jsonwebtoken::errors::ErrorKind;
                match err.kind() {
                    ErrorKind::ExpiredSignature => Err(AuthError::TokenExpired),
                    _ => Err(AuthError::JwtError(err)),  // ← Pass err directly, NOT err.to_string()
                }
            }
        }
    }
    
    /// Refresh a token (issue new token with same permissions)
    pub fn refresh_token(&self, token: &str, ttl_hours: u64) -> Result<String, AuthError> {
        let claims = self.verify_token(token)?;
        self.create_token(&claims.sub, claims.permissions, ttl_hours)
    }
}

/// Extract Bearer token from Authorization header
pub fn extract_bearer_token(auth_header: &str) -> Result<String, AuthError> {
    if !auth_header.starts_with("Bearer ") {
        return Err(AuthError::InvalidToken);
    }
    
    Ok(auth_header.trim_start_matches("Bearer ").to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    // use tokio::time::{sleep, Duration};
    // use chrono::Utc;
    
    #[test]
    fn test_create_and_verify_token() {
        let manager = JwtManager::new("test-secret-key".to_string());
        let permissions = vec!["read".to_string(), "write".to_string()];
        
        let token = manager
            .create_token("test@bsvbank.local", permissions.clone(), 24)
            .unwrap();
        
        let claims = manager.verify_token(&token).unwrap();
        
        assert_eq!(claims.sub, "test@bsvbank.local");
        assert_eq!(claims.permissions, permissions);
        assert!(!claims.is_expired());
    }
    
    #[test]
    fn test_token_expiration() {
        use jsonwebtoken::{encode, EncodingKey, Header};
        
        let manager = JwtManager::new("test-secret-key".to_string());
        
        // Create an expired token
        let claims = Claims {
            sub: "test@bsvbank.local".to_string(),
            exp: (chrono::Utc::now() - chrono::Duration::hours(1)).timestamp() as usize,
            iat: (chrono::Utc::now() - chrono::Duration::hours(2)).timestamp() as usize,
            permissions: vec![],
        };
        
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(manager.secret.as_ref()),
        )
        .unwrap();
        
        // Should fail with TokenExpired error
        let result = manager.verify_token(&token);
        
        match result {
            Err(AuthError::TokenExpired) => {
                // ✅ This is what we expect!
            }
            Err(AuthError::JwtError(err)) => {
                // Check if it's an expiration error by checking the kind
                use jsonwebtoken::errors::ErrorKind;
                match err.kind() {
                    ErrorKind::ExpiredSignature => {
                        // ✅ Also acceptable
                    }
                    _ => {
                        panic!("Expected ExpiredSignature error, got: {:?}", err.kind());
                    }
                }
            }
            other => {
                panic!("Expected TokenExpired or JwtError with ExpiredSignature, got: {:?}", other);
            }
        }
    }

    // // ChatGPT optimal (?)
    // #[tokio::test]
    // async fn test_token_expiration() {
    //     let manager = JwtManager::new("test-secret-key".to_string());

    //     // Create a token that expired 1 second ago
    //     let expired_token = manager
    //         .create_token_with_expiration("test@bsvbank.local", vec![], chrono::Utc::now() - chrono::Duration::seconds(1))
    //         .unwrap();

    //     // Verification should fail
    //     let result = manager.verify_token(&expired_token);
    //     assert!(matches!(result, Err(AuthError::TokenExpired)), "Token should be expired but verification succeeded");
    // }

    // // Chatgpt compromise (?)
    // #[tokio::test]
    // async fn test_token_expiration() {
    //     let manager = JwtManager::new("test-secret-key".to_string());

    //     // Create a token with 0-hour validity (expires immediately)
    //     let token = manager
    //         .create_token("test@bsvbank.local", vec![], 0)
    //         .unwrap();

    //     // Attempt to verify immediately — should fail due to expiration
    //     let result = manager.verify_token(&token);

    //     // Adapt to existing AuthError structure
    //     match result {
    //         Err(AuthError::TokenExpired) => {} // expected
    //         other => panic!("Expected TokenExpired, got {:?}", other),
    //     }
    // }
    
    #[test]
    fn test_invalid_token() {
        let manager = JwtManager::new("test-secret-key".to_string());
        
        let result = manager.verify_token("invalid.token.here");
        assert!(result.is_err());
    }
    
    #[test]
    fn test_permissions() {
        let claims = Claims::new(
            "test@bsvbank.local".to_string(),
            vec!["read".to_string(), "write".to_string()],
            24,
        );
        
        assert!(claims.has_permission("read"));
        assert!(claims.has_permission("write"));
        assert!(!claims.has_permission("delete"));
    }
    
    #[test]
    fn test_admin_permission() {
        let claims = Claims::new(
            "admin@bsvbank.local".to_string(),
            vec!["admin".to_string()],
            24,
        );
        
        // Admin should have all permissions
        assert!(claims.has_permission("read"));
        assert!(claims.has_permission("write"));
        assert!(claims.has_permission("delete"));
        assert!(claims.has_permission("anything"));
    }
    
    #[test]
    fn test_extract_bearer_token() {
        let auth_header = "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9";
        let token = extract_bearer_token(auth_header).unwrap();
        assert_eq!(token, "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9");
    }
    
    #[test]
    fn test_invalid_bearer_format() {
        let auth_header = "InvalidFormat token";
        let result = extract_bearer_token(auth_header);
        assert!(matches!(result, Err(AuthError::InvalidToken)));
    }
    
    #[test]
    fn test_refresh_token() {
        let manager = JwtManager::new("test-secret-key".to_string());
        
        let original_token = manager
            .create_token("test@bsvbank.local", vec!["read".to_string()], 1)
            .unwrap();
        
        let refreshed_token = manager.refresh_token(&original_token, 24).unwrap();
        
        let claims = manager.verify_token(&refreshed_token).unwrap();
        assert_eq!(claims.sub, "test@bsvbank.local");
        assert_eq!(claims.permissions, vec!["read".to_string()]);
    }
}