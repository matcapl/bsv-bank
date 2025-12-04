// core/common/src/cache.rs

use redis::AsyncCommands;

pub struct CacheManager {
    redis: RedisPool,
}

impl CacheManager {
    pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>>
    pub async fn set<T: Serialize>(&self, key: &str, value: &T, ttl_seconds: u64) -> Result<()>
    pub async fn invalidate(&self, key: &str) -> Result<()>
}