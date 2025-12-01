// core/deposit-service/src/handlers/health.rs
// Health check endpoints

use actix_web::{web, HttpResponse, Result};
use bsv_bank_common::{
    check_database_health, HealthResponse, LivenessProbe, ReadinessProbe,
};
use sqlx::PgPool;
use std::time::SystemTime;

pub struct AppState {
    pub db_pool: PgPool,
    pub start_time: SystemTime,
}

/// Health check endpoint - comprehensive service health
pub async fn health_check(data: web::Data<AppState>) -> Result<HttpResponse> {
    let mut health = HealthResponse::new(
        env!("CARGO_PKG_VERSION").to_string(),
        data.start_time,
    );

    // Check database health
    let db_health = check_database_health(&data.db_pool).await;
    health.add_dependency(db_health);

    // Add more dependency checks as needed
    // e.g., check_external_api_health("blockchain_monitor", "http://localhost:8083/health").await

    Ok(HttpResponse::Ok().json(health))
}

/// Liveness probe - is the service alive?
pub async fn liveness_probe() -> Result<HttpResponse> {
    Ok(HttpResponse::Ok().json(LivenessProbe::healthy()))
}

/// Readiness probe - is the service ready to accept traffic?
pub async fn readiness_probe(data: web::Data<AppState>) -> Result<HttpResponse> {
    // Check if database is accessible
    let db_health = check_database_health(&data.db_pool).await;
    let ready = db_health.status.is_healthy();

    let probe = if ready {
        ReadinessProbe::ready()
    } else {
        ReadinessProbe::not_ready()
    };

    let status_code = if ready { 200 } else { 503 };

    Ok(HttpResponse::build(actix_web::http::StatusCode::from_u16(status_code).unwrap())
        .json(probe))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::postgres::PgPoolOptions;

    async fn setup_test_pool() -> PgPool {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://localhost/bsv_bank_test".to_string());
        
        PgPoolOptions::new()
            .max_connections(1)
            .connect(&database_url)
            .await
            .expect("Failed to connect to test database")
    }

    #[actix_web::test]
    async fn test_liveness_probe() {
        let response = liveness_probe().await.unwrap();
        assert_eq!(response.status(), 200);
    }

    #[actix_web::test]
    async fn test_health_check_structure() {
        let pool = setup_test_pool().await;
        let state = web::Data::new(AppState {
            db_pool: pool,
            start_time: SystemTime::now(),
        });

        let response = health_check(state).await.unwrap();
        assert_eq!(response.status(), 200);
    }

    #[actix_web::test]
    async fn test_readiness_probe() {
        let pool = setup_test_pool().await;
        let state = web::Data::new(AppState {
            db_pool: pool,
            start_time: SystemTime::now(),
        });

        let response = readiness_probe(state).await.unwrap();
        // Should be 200 if DB is available, 503 if not
        assert!(response.status() == 200 || response.status() == 503);
    }
}