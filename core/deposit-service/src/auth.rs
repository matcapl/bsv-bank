use serde::{Deserialize, Serialize};
use jsonwebtoken::{encode, decode, Header, Validation, EncodingKey, DecodingKey, errors::Error};
use std::env;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,  // paymail
    pub exp: usize,   // expiration
    pub iat: usize,   // issued at
}

pub fn create_jwt(paymail: &str) -> Result<String, Error> {
    let secret = env::var("JWT_SECRET").unwrap_or_else(|_| "dev-secret-key".to_string());
    
    let now = chrono::Utc::now().timestamp() as usize;
    let claims = Claims {
        sub: paymail.to_string(),
        exp: now + 3600, // 1 hour
        iat: now,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes())
    )
}

pub fn verify_jwt(token: &str) -> Result<Claims, Error> {
    let secret = env::var("JWT_SECRET").unwrap_or_else(|_| "dev-secret-key".to_string());
    
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default()
    )?;

    Ok(token_data.claims)
}
