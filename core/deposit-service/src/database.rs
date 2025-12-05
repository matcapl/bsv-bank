// core/deposit-service/src/database.rs

use sqlx::{PgPool}; // postgres::PgPoolOptions
// use std::env;

// pub async fn create_pool() -> Result<PgPool, sqlx::Error> {
//     let database_url = env::var("DATABASE_URL")
//         .unwrap_or_else(|_| "postgresql://a:@localhost:5432/bsv_bank".to_string());
    
//     PgPoolOptions::new()
//         .max_connections(5)
//         .connect(&database_url)
//         .await
// }

pub async fn get_or_create_user(pool: &PgPool, paymail: &str) -> Result<i32, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        INSERT INTO users (paymail)
        VALUES ($1)
        ON CONFLICT (paymail) DO UPDATE SET updated_at = NOW()
        RETURNING id
        "#,
        paymail
    )
    .fetch_one(pool)
    .await?;
    
    Ok(result.id)
}
