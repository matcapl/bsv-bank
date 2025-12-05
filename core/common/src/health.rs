// core/common/src/health.rs
// Health check system for services

use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};
use sqlx::PgPool;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

impl HealthStatus {
    pub fn is_healthy(&self) -> bool {
        matches!(self, HealthStatus::Healthy)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: HealthStatus,
    pub service: String,
    pub version: String,
    pub uptime_seconds: u64,
    pub features: Vec<String>,
    pub dependencies: Vec<DependencyHealth>,
}

impl HealthResponse {
    pub fn new(service: String, version: String, start_time: SystemTime) -> Self {
        let uptime = SystemTime::now()
            .duration_since(start_time)
            .unwrap_or(Duration::from_secs(0))
            .as_secs();
        
        Self {
            status: HealthStatus::Healthy,
            service,
            version,
            uptime_seconds: uptime,
            features: Vec::new(),
            dependencies: Vec::new(),
        }
    }
    
    pub fn with_features(mut self, features: Vec<String>) -> Self {
        self.features = features;
        self
    }
    
    pub fn add_dependency(&mut self, dependency: DependencyHealth) {
        self.dependencies.push(dependency);
        self.update_overall_status();
    }
    
    fn update_overall_status(&mut self) {
        if self.dependencies.is_empty() {
            return;
        }
        
        let has_unhealthy = self.dependencies.iter().any(|d| d.status == HealthStatus::Unhealthy);
        let has_degraded = self.dependencies.iter().any(|d| d.status == HealthStatus::Degraded);
        
        if has_unhealthy {
            self.status = HealthStatus::Unhealthy;
        } else if has_degraded {
            self.status = HealthStatus::Degraded;
        } else {
            self.status = HealthStatus::Healthy;
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DependencyHealth {
    pub name: String,
    pub status: HealthStatus,
    pub latency_ms: Option<u64>,
    pub message: Option<String>,
}

impl DependencyHealth {
    pub fn new(name: String, status: HealthStatus) -> Self {
        Self {
            name,
            status,
            latency_ms: None,
            message: None,
        }
    }
    
    pub fn with_latency(mut self, latency_ms: u64) -> Self {
        self.latency_ms = Some(latency_ms);
        self
    }
    
    pub fn with_message(mut self, message: String) -> Self {
        self.message = Some(message);
        self
    }
}

/// Check database health
pub async fn check_database_health(pool: &PgPool) -> DependencyHealth {
    let start = SystemTime::now();
    
    match sqlx::query("SELECT 1")
        .fetch_one(pool)
        .await
    {
        Ok(_) => {
            let latency = SystemTime::now()
                .duration_since(start)
                .unwrap_or(Duration::from_secs(0))
                .as_millis() as u64;
            
            let status = if latency > 1000 {
                HealthStatus::Degraded
            } else {
                HealthStatus::Healthy
            };
            
            DependencyHealth::new("database".to_string(), status)
                .with_latency(latency)
        }
        Err(e) => {
            DependencyHealth::new("database".to_string(), HealthStatus::Unhealthy)
                .with_message(format!("Database error: {}", e))
        }
    }
}

/// Check Redis health (if Redis feature is enabled)
#[cfg(feature = "redis-cache")]
pub async fn check_redis_health(client: &redis::Client) -> DependencyHealth {
    use redis::AsyncCommands;
    
    let start = SystemTime::now();
    
    match client.get_async_connection().await {
        Ok(mut conn) => {
            match redis::cmd("PING").query_async::<_, String>(&mut conn).await {
                Ok(_) => {
                    let latency = SystemTime::now()
                        .duration_since(start)
                        .unwrap_or(Duration::from_secs(0))
                        .as_millis() as u64;
                    
                    let status = if latency > 500 {
                        HealthStatus::Degraded
                    } else {
                        HealthStatus::Healthy
                    };
                    
                    DependencyHealth::new("redis".to_string(), status)
                        .with_latency(latency)
                }
                Err(e) => {
                    DependencyHealth::new("redis".to_string(), HealthStatus::Unhealthy)
                        .with_message(format!("Redis error: {}", e))
                }
            }
        }
        Err(e) => {
            DependencyHealth::new("redis".to_string(), HealthStatus::Unhealthy)
                .with_message(format!("Redis connection error: {}", e))
        }
    }
}

/// Check external API health
pub async fn check_external_api_health(
    name: &str,
    url: &str,
) -> DependencyHealth {
    let start = SystemTime::now();
    let client = reqwest::Client::new();
    
    match client
        .get(url)
        .timeout(Duration::from_secs(5))
        .send()
        .await
    {
        Ok(response) => {
            let latency = SystemTime::now()
                .duration_since(start)
                .unwrap_or(Duration::from_secs(0))
                .as_millis() as u64;
            
            let status = if response.status().is_success() {
                if latency > 2000 {
                    HealthStatus::Degraded
                } else {
                    HealthStatus::Healthy
                }
            } else {
                HealthStatus::Unhealthy
            };
            
            DependencyHealth::new(name.to_string(), status)
                .with_latency(latency)
                .with_message(format!("HTTP {}", response.status()))
        }
        Err(e) => {
            DependencyHealth::new(name.to_string(), HealthStatus::Unhealthy)
                .with_message(format!("API error: {}", e))
        }
    }
}

/// Liveness probe - is the service running?
#[derive(Debug, Clone, Serialize)]
pub struct LivenessProbe {
    pub alive: bool,
}

impl LivenessProbe {
    pub fn healthy() -> Self {
        Self { alive: true }
    }
}

/// Readiness probe - is the service ready to accept traffic?
#[derive(Debug, Clone, Serialize)]
pub struct ReadinessProbe {
    pub ready: bool,
    pub dependencies_ready: bool,
}

impl ReadinessProbe {
    pub fn new(dependencies_ready: bool) -> Self {
        Self {
            ready: dependencies_ready,
            dependencies_ready,
        }
    }
    
    pub fn ready() -> Self {
        Self::new(true)
    }
    
    pub fn not_ready() -> Self {
        Self::new(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_health_status_is_healthy() {
        assert!(HealthStatus::Healthy.is_healthy());
        assert!(!HealthStatus::Degraded.is_healthy());
        assert!(!HealthStatus::Unhealthy.is_healthy());
    }
    
    #[test]
    fn test_health_response_creation() {
        let start_time = SystemTime::now();
        let health = HealthResponse::new("common".to_string(), "1.0.0".to_string(), start_time);
        
        assert_eq!(health.status, HealthStatus::Healthy);
        assert_eq!(health.version, "1.0.0");
        assert_eq!(health.dependencies.len(), 0);
    }
    
    #[test]
    fn test_health_response_with_healthy_dependency() {
        let start_time = SystemTime::now();
        let mut health = HealthResponse::new("common".to_string(), "1.0.0".to_string(), start_time);
        
        let dep = DependencyHealth::new("database".to_string(), HealthStatus::Healthy)
            .with_latency(10);
        
        health.add_dependency(dep);
        
        assert_eq!(health.status, HealthStatus::Healthy);
        assert_eq!(health.dependencies.len(), 1);
    }
    
    #[test]
    fn test_health_response_with_degraded_dependency() {
        let start_time = SystemTime::now();
        let mut health = HealthResponse::new("common".to_string(), "1.0.0".to_string(), start_time);
        
        let dep = DependencyHealth::new("database".to_string(), HealthStatus::Degraded)
            .with_latency(1500);
        
        health.add_dependency(dep);
        
        assert_eq!(health.status, HealthStatus::Degraded);
    }
    
    #[test]
    fn test_health_response_with_unhealthy_dependency() {
        let start_time = SystemTime::now();
        let mut health = HealthResponse::new("common".to_string(), "1.0.0".to_string(), start_time);
        
        let dep = DependencyHealth::new("database".to_string(), HealthStatus::Unhealthy)
            .with_message("Connection refused".to_string());
        
        health.add_dependency(dep);
        
        assert_eq!(health.status, HealthStatus::Unhealthy);
    }
    
    #[test]
    fn test_health_response_multiple_dependencies() {
        let start_time = SystemTime::now();
        let mut health = HealthResponse::new("common".to_string(), "1.0.0".to_string(), start_time);
        
        health.add_dependency(
            DependencyHealth::new("database".to_string(), HealthStatus::Healthy)
        );
        health.add_dependency(
            DependencyHealth::new("redis".to_string(), HealthStatus::Degraded)
        );
        
        // Should be degraded because one dependency is degraded
        assert_eq!(health.status, HealthStatus::Degraded);
        assert_eq!(health.dependencies.len(), 2);
    }
    
    #[test]
    fn test_health_response_worst_status_wins() {
        let start_time = SystemTime::now();
        let mut health = HealthResponse::new("common".to_string(), "1.0.0".to_string(), start_time);
        
        health.add_dependency(
            DependencyHealth::new("database".to_string(), HealthStatus::Healthy)
        );
        health.add_dependency(
            DependencyHealth::new("redis".to_string(), HealthStatus::Degraded)
        );
        health.add_dependency(
            DependencyHealth::new("api".to_string(), HealthStatus::Unhealthy)
        );
        
        // Should be unhealthy because one dependency is unhealthy
        assert_eq!(health.status, HealthStatus::Unhealthy);
    }
    
    #[test]
    fn test_dependency_health_with_latency() {
        let dep = DependencyHealth::new("test".to_string(), HealthStatus::Healthy)
            .with_latency(25);
        
        assert_eq!(dep.latency_ms, Some(25));
    }
    
    #[test]
    fn test_dependency_health_with_message() {
        let dep = DependencyHealth::new("test".to_string(), HealthStatus::Unhealthy)
            .with_message("Connection timeout".to_string());
        
        assert_eq!(dep.message, Some("Connection timeout".to_string()));
    }
    
    #[test]
    fn test_liveness_probe() {
        let probe = LivenessProbe::healthy();
        assert!(probe.alive);
    }
    
    #[test]
    fn test_readiness_probe_ready() {
        let probe = ReadinessProbe::ready();
        assert!(probe.ready);
        assert!(probe.dependencies_ready);
    }
    
    #[test]
    fn test_readiness_probe_not_ready() {
        let probe = ReadinessProbe::not_ready();
        assert!(!probe.ready);
        assert!(!probe.dependencies_ready);
    }
    
    #[test]
    fn test_health_response_serialization() {
        let start_time = SystemTime::now();
        let mut health = HealthResponse::new("common".to_string(), "1.0.0".to_string(), start_time);
        
        health.add_dependency(
            DependencyHealth::new("database".to_string(), HealthStatus::Healthy)
                .with_latency(10)
        );
        
        let json = serde_json::to_string(&health).unwrap();
        assert!(json.contains("\"status\":\"healthy\""));
        assert!(json.contains("\"version\":\"1.0.0\""));
        assert!(json.contains("\"database\""));
    }
}