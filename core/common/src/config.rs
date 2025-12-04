// core/common/src/config.rs

#[derive(Debug, Clone)]
pub struct ServiceConfig {
    pub environment: Environment,
    pub database_url: String,
    pub redis_url: Option<String>,
    pub jwt_secret: String,
    pub blockchain_monitor_url: String,
    pub enable_blockchain: bool,
    pub rate_limit_requests_per_minute: u32,
    pub log_level: String,
}

pub enum Environment {
    Development,
    Staging,
    Production,
}