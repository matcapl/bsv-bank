use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use sha2::{Sha256, Digest};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InterestRate {
    timestamp: DateTime<Utc>,
    utilization_rate: f64,
    borrow_apy: f64,
    supply_apy: f64,
    total_deposits: u64,
    total_borrowed: u64,
}

struct AppState {
    rates: Arc<Mutex<Vec<InterestRate>>>,
    user_accruals: Arc<Mutex<HashMap<String, u64>>>,
}

fn calculate_rates(total_deposits: u64, total_borrowed: u64) -> (f64, f64) {
    let utilization = if total_deposits == 0 {
        0.0
    } else {
        total_borrowed as f64 / total_deposits as f64
    };
    
    let base_rate = 0.02;
    let optimal_util = 0.80;
    
    let borrow_apy = if utilization <= optimal_util {
        base_rate + (utilization * 0.10)
    } else {
        base_rate + (optimal_util * 0.10) + ((utilization - optimal_util) * 1.00)
    };
    
    let supply_apy = borrow_apy * utilization * 0.90; // 90% goes to suppliers
    
    (borrow_apy, supply_apy)
}

async fn get_current_rates(data: web::Data<AppState>) -> impl Responder {
    let total_deposits = 10000000u64;
    let total_borrowed = 7000000u64;
    
    let (borrow_apy, supply_apy) = calculate_rates(total_deposits, total_borrowed);
    
    let rate = InterestRate {
        timestamp: Utc::now(),
        utilization_rate: total_borrowed as f64 / total_deposits as f64,
        borrow_apy,
        supply_apy,
        total_deposits,
        total_borrowed,
    };
    
    let mut rates = data.rates.lock().unwrap();
    rates.push(rate.clone());
    
    // Create OP_RETURN commitment
    let commitment_data = format!(
        "RATE|{}|{}|{}",
        rate.utilization_rate,
        rate.borrow_apy,
        rate.timestamp.timestamp()
    );
    let mut hasher = Sha256::new();
    hasher.update(commitment_data.as_bytes());
    let hash = hex::encode(hasher.finalize());
    
    println!("Interest rate commitment: 6a{}", hash);
    
    HttpResponse::Ok().json(rate)
}

async fn distribute_interest(data: web::Data<AppState>) -> impl Responder {
    println!("Running interest distribution...");
    
    // This would query actual user balances from deposit service
    // For now, simulate
    let users = vec![
        ("test@handcash.io", 100000u64),
        ("user@handcash.io", 500000u64),
    ];
    
    let daily_rate = 0.07 / 365.0; // 7% APY
    
    let mut accruals = data.user_accruals.lock().unwrap();
    
    for (paymail, balance) in users {
        let interest = (balance as f64 * daily_rate) as u64;
        *accruals.entry(paymail.to_string()).or_insert(0) += interest;
        println!("  {} earned {} satoshis", paymail, interest);
    }
    
    HttpResponse::Ok().json(serde_json::json!({
        "status": "success",
        "distributed_at": Utc::now()
    }))
}

async fn health_check() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "service": "interest-engine",
        "status": "healthy",
        "version": "0.1.0"
    }))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("📊 BSV Bank - Interest Engine Starting...");
    println!("✅ Service ready on http://0.0.0.0:8081");
    
    let app_state = web::Data::new(AppState {
        rates: Arc::new(Mutex::new(Vec::new())),
        user_accruals: Arc::new(Mutex::new(HashMap::new())),
    });
    
    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .route("/health", web::get().to(health_check))
            .route("/rates/current", web::get().to(get_current_rates))
            .route("/interest/distribute", web::post().to(distribute_interest))
    })
    .bind(("0.0.0.0", 8081))?
    .run()
    .await
}
