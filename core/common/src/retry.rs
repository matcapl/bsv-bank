// core/common/src/retry.rs

pub async fn retry_with_backoff<F, T, E>(
    operation: F,
    max_attempts: u32,
    initial_delay_ms: u64,
) -> Result<T, E>
where
    F: Fn() -> Future<Output = Result<T, E>>,