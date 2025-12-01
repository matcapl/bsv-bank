// core/common/src/rate_limit.rs
// Rate limiting with sliding window algorithm

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RateLimitError {
    #[error("Rate limit exceeded: {0}")]
    LimitExceeded(String),
    #[error("Internal error: {0}")]
    InternalError(String),
}

#[derive(Debug, Clone)]
pub struct RateLimit {
    pub requests_per_window: u32,
    pub window_seconds: u64,
}

impl RateLimit {
    pub fn new(requests_per_window: u32, window_seconds: u64) -> Self {
        Self {
            requests_per_window,
            window_seconds,
        }
    }
    
    // Common rate limit configurations
    pub fn per_second(requests: u32) -> Self {
        Self::new(requests, 1)
    }
    
    pub fn per_minute(requests: u32) -> Self {
        Self::new(requests, 60)
    }
    
    pub fn per_hour(requests: u32) -> Self {
        Self::new(requests, 3600)
    }
}

#[derive(Debug, Clone)]
struct RateLimitEntry {
    timestamps: Vec<u64>,
}

impl RateLimitEntry {
    fn new() -> Self {
        Self {
            timestamps: Vec::new(),
        }
    }
    
    fn add_request(&mut self, now: u64) {
        self.timestamps.push(now);
    }
    
    fn cleanup_old_requests(&mut self, window_start: u64) {
        self.timestamps.retain(|&ts| ts >= window_start);
    }
    
    fn request_count(&self) -> usize {
        self.timestamps.len()
    }
}

pub struct RateLimiter {
    limits: HashMap<String, RateLimit>,
    entries: Arc<RwLock<HashMap<String, RateLimitEntry>>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            limits: HashMap::new(),
            entries: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Add a rate limit for a specific endpoint
    pub fn add_limit(&mut self, endpoint: String, limit: RateLimit) {
        self.limits.insert(endpoint, limit);
    }
    
    /// Check if request is allowed for given key (IP, user, API key)
    pub async fn check_rate_limit(
        &self,
        endpoint: &str,
        key: &str,
    ) -> Result<RateLimitInfo, RateLimitError> {
        let limit = self.limits.get(endpoint).ok_or_else(|| {
            RateLimitError::InternalError(format!("No rate limit configured for {}", endpoint))
        })?;
        
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| RateLimitError::InternalError(e.to_string()))?
            .as_secs();
        
        let window_start = now - limit.window_seconds;
        let rate_key = format!("{}:{}", endpoint, key);
        
        let mut entries = self.entries.write().await;
        let entry = entries
            .entry(rate_key.clone())
            .or_insert_with(RateLimitEntry::new);
        
        // Clean up old requests outside the window
        entry.cleanup_old_requests(window_start);
        
        let current_count = entry.request_count();
        
        if current_count >= limit.requests_per_window as usize {
            let oldest_request = entry.timestamps.first().copied().unwrap_or(now);
            let retry_after = (oldest_request + limit.window_seconds).saturating_sub(now);
            
            return Err(RateLimitError::LimitExceeded(format!(
                "Rate limit exceeded. Retry after {} seconds",
                retry_after
            )));
        }
        
        // Add current request
        entry.add_request(now);
        
        Ok(RateLimitInfo {
            limit: limit.requests_per_window,
            remaining: limit.requests_per_window - (current_count as u32 + 1),
            reset: now + limit.window_seconds,
        })
    }
    
    /// Background cleanup task to remove old entries
    pub async fn cleanup_old_entries(&self) {
        let mut entries = self.entries.write().await;
        entries.retain(|_, entry| !entry.timestamps.is_empty());
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct RateLimitInfo {
    pub limit: u32,
    pub remaining: u32,
    pub reset: u64,
}

impl RateLimitInfo {
    /// Generate HTTP headers for rate limiting
    pub fn to_headers(&self) -> Vec<(String, String)> {
        vec![
            ("X-RateLimit-Limit".to_string(), self.limit.to_string()),
            ("X-RateLimit-Remaining".to_string(), self.remaining.to_string()),
            ("X-RateLimit-Reset".to_string(), self.reset.to_string()),
        ]
    }
}

/// Start background cleanup task
pub fn start_cleanup_task(limiter: Arc<RateLimiter>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            limiter.cleanup_old_entries().await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};
    
    #[tokio::test]
    async fn test_rate_limit_allows_requests_within_limit() {
        let mut limiter = RateLimiter::new();
        limiter.add_limit("test".to_string(), RateLimit::per_minute(5));
        
        // First 5 requests should succeed
        for i in 0..5 {
            let result = limiter.check_rate_limit("test", "user1").await;
            assert!(result.is_ok(), "Request {} should succeed", i);
        }
    }
    
    #[tokio::test]
    async fn test_rate_limit_blocks_excess_requests() {
        let mut limiter = RateLimiter::new();
        limiter.add_limit("test".to_string(), RateLimit::per_minute(3));
        
        // First 3 requests should succeed
        for _ in 0..3 {
            limiter.check_rate_limit("test", "user1").await.unwrap();
        }
        
        // 4th request should fail
        let result = limiter.check_rate_limit("test", "user1").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RateLimitError::LimitExceeded(_)));
    }
    
    #[tokio::test]
    async fn test_rate_limit_per_key() {
        let mut limiter = RateLimiter::new();
        limiter.add_limit("test".to_string(), RateLimit::per_minute(2));
        
        // User1 makes 2 requests
        limiter.check_rate_limit("test", "user1").await.unwrap();
        limiter.check_rate_limit("test", "user1").await.unwrap();
        
        // User1 is blocked
        let result = limiter.check_rate_limit("test", "user1").await;
        assert!(result.is_err());
        
        // User2 can still make requests
        let result = limiter.check_rate_limit("test", "user2").await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_rate_limit_info() {
        let mut limiter = RateLimiter::new();
        limiter.add_limit("test".to_string(), RateLimit::per_minute(10));
        
        let info = limiter.check_rate_limit("test", "user1").await.unwrap();
        
        assert_eq!(info.limit, 10);
        assert_eq!(info.remaining, 9); // Used 1
    }
    
    #[tokio::test]
    async fn test_rate_limit_headers() {
        let info = RateLimitInfo {
            limit: 100,
            remaining: 95,
            reset: 1234567890,
        };
        
        let headers = info.to_headers();
        assert_eq!(headers.len(), 3);
        assert!(headers.contains(&("X-RateLimit-Limit".to_string(), "100".to_string())));
        assert!(headers.contains(&("X-RateLimit-Remaining".to_string(), "95".to_string())));
    }
    
    #[tokio::test]
    async fn test_sliding_window_cleanup() {
        let mut limiter = RateLimiter::new();
        // Very short window for testing
        limiter.add_limit("test".to_string(), RateLimit::new(2, 1));
        
        // Make 2 requests
        limiter.check_rate_limit("test", "user1").await.unwrap();
        limiter.check_rate_limit("test", "user1").await.unwrap();
        
        // Should be blocked
        let result = limiter.check_rate_limit("test", "user1").await;
        assert!(result.is_err());
        
        // Wait for window to expire
        sleep(Duration::from_secs(2)).await;
        
        // Should be allowed again
        let result = limiter.check_rate_limit("test", "user1").await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_cleanup_old_entries() {
        let mut limiter = RateLimiter::new();
        limiter.add_limit("test".to_string(), RateLimit::new(5, 1));
        
        // Make some requests
        limiter.check_rate_limit("test", "user1").await.unwrap();
        limiter.check_rate_limit("test", "user2").await.unwrap();
        
        // Wait for window to expire
        sleep(Duration::from_secs(2)).await;
        
        // Run cleanup
        limiter.cleanup_old_entries().await;
        
        // Entries should be removed after cleanup
        let entries = limiter.entries.read().await;
        assert_eq!(entries.len(), 0);
    }
    
    #[tokio::test]
    async fn test_different_endpoints() {
        let mut limiter = RateLimiter::new();
        limiter.add_limit("endpoint1".to_string(), RateLimit::per_minute(2));
        limiter.add_limit("endpoint2".to_string(), RateLimit::per_minute(5));
        
        // Use up endpoint1 limit
        limiter.check_rate_limit("endpoint1", "user1").await.unwrap();
        limiter.check_rate_limit("endpoint1", "user1").await.unwrap();
        
        // endpoint1 should be blocked
        let result = limiter.check_rate_limit("endpoint1", "user1").await;
        assert!(result.is_err());
        
        // endpoint2 should still work
        let result = limiter.check_rate_limit("endpoint2", "user1").await;
        assert!(result.is_ok());
    }
}