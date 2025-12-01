// core/deposit-service/src/middleware/metrics.rs
// Metrics collection middleware

use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error,
};
use actix_web::{HttpResponse, Responder};
use bsv_bank_common::{MetricsTimer, ServiceMetrics};
use std::future::{ready, Ready};
use std::rc::Rc;
use futures_util::future::LocalBoxFuture;

pub struct MetricsMiddleware {
    metrics: Rc<ServiceMetrics>,
}

pub async fn metrics_handler() -> impl Responder {
    HttpResponse::Ok()
        .content_type("text/plain")
        .body("# Metrics\n")
}

impl MetricsMiddleware {
    pub fn new(metrics: ServiceMetrics) -> Self {
        Self {
            metrics: Rc::new(metrics),
        }
    }
}

impl<S, B> Transform<S, ServiceRequest> for MetricsMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = MetricsMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(MetricsMiddlewareService {
            service: Rc::new(service),
            metrics: self.metrics.clone(),
        }))
    }
}

pub struct MetricsMiddlewareService<S> {
    service: Rc<S>,
    metrics: Rc<ServiceMetrics>,
}

impl<S, B> Service<ServiceRequest> for MetricsMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let metrics = self.metrics.clone();
        let service = self.service.clone();
        
        let method = req.method().to_string();
        let path = req.path().to_string();

        // Increment in-progress counter
        metrics
            .http_requests_in_progress
            .with_label_values(&[&method, &path])
            .inc();

        let timer = MetricsTimer::new();

        Box::pin(async move {
            let result = service.call(req).await;

            // Decrement in-progress counter
            metrics
                .http_requests_in_progress
                .with_label_values(&[&method, &path])
                .dec();

            let duration = timer.elapsed_seconds();

            match &result {
                Ok(response) => {
                    let status = response.status().as_u16();
                    metrics.record_http_request(&method, &path, status, duration);
                }
                Err(_) => {
                    metrics.record_http_request(&method, &path, 500, duration);
                }
            }

            result
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, web, App, HttpResponse};
    use bsv_bank_common::ServiceMetrics;
    use prometheus::Registry;

    async fn test_handler() -> HttpResponse {
        HttpResponse::Ok().body("success")
    }

    #[actix_web::test]
    async fn test_metrics_middleware_records_request() {
        let registry = Registry::new();
        let metrics = ServiceMetrics::new(&registry, "test_service").unwrap();
        
        let app = test::init_service(
            App::new()
                .wrap(MetricsMiddleware::new(metrics))
                .route("/test", web::get().to(test_handler)),
        )
        .await;

        let req = test::TestRequest::get().uri("/test").to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());

        // Verify metrics were recorded
        let metric_families = registry.gather();
        assert!(!metric_families.is_empty());
    }
}