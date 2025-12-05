// core/common/src/middleware.rs

use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform}, 
    Error, // HttpResponse,
};
use futures_util::future::LocalBoxFuture;
use std::future::{ready, Ready};
use std::sync::Arc;
use crate::rate_limit::{RateLimiter, RateLimitError};
// use crate::error::ServiceError;

pub struct RateLimitMiddleware {
    limiter: Arc<RateLimiter>,
}

impl RateLimitMiddleware {
    pub fn new(limiter: Arc<RateLimiter>) -> Self {
        Self { limiter }
    }
}

impl<S, B> Transform<S, ServiceRequest> for RateLimitMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = RateLimitMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RateLimitMiddlewareService {
            service,
            limiter: self.limiter.clone(),
        }))
    }
}

pub struct RateLimitMiddlewareService<S> {
    service: S,
    limiter: Arc<RateLimiter>,
}

impl<S, B> Service<ServiceRequest> for RateLimitMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let limiter = self.limiter.clone();
        let endpoint = req.path().to_string();
        
        // Extract rate limit key (IP address or API key)
        let key = req
            .connection_info()
            .realip_remote_addr()
            .unwrap_or("unknown")
            .to_string();

        let fut = self.service.call(req);

        Box::pin(async move {
            // Check rate limit BEFORE processing request
            match limiter.check_rate_limit(&endpoint, &key).await {
                Ok(info) => {
                    // Allow request, process it
                    let mut res = fut.await?;
                    
                    // Add rate limit headers to response
                    let headers = res.headers_mut();
                    for (header_name, header_value) in info.to_headers() {
                        if let Ok(name) = actix_web::http::header::HeaderName::from_bytes(header_name.as_bytes()) {
                            if let Ok(value) = actix_web::http::header::HeaderValue::from_str(&header_value) {
                                headers.insert(name, value);
                            }
                        }
                    }
                    
                    Ok(res)
                }
                Err(RateLimitError::LimitExceeded(msg)) => {
                    // Block request with 429 Too Many Requests
                    Err(actix_web::error::ErrorTooManyRequests(msg))
                }
                Err(RateLimitError::InternalError(_)) => {
                    // On rate limiter error, allow request through (fail open)
                    fut.await
                }
            }
        })
    }
}

// Helper function to set up rate limiting in your service
pub fn configure_rate_limits(limiter: &mut RateLimiter) {
    use crate::rate_limit::RateLimit;
    
    // Default limits for common endpoints
    limiter.add_limit("/api/deposits".to_string(), RateLimit::per_minute(10));
    limiter.add_limit("/api/balance".to_string(), RateLimit::per_minute(60));
    limiter.add_limit("/api/history".to_string(), RateLimit::per_minute(30));
    limiter.add_limit("/api/webhook".to_string(), RateLimit::per_minute(100));
    
    // Stricter limits for sensitive operations
    limiter.add_limit("/api/withdraw".to_string(), RateLimit::per_minute(5));
    limiter.add_limit("/api/admin".to_string(), RateLimit::per_minute(20));
}