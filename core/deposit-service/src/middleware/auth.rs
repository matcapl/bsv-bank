// Check: Do src/middleware/auth.rs and src/middleware/metrics.rs duplicate common? 
// If yes, delete them and use:
// use bsv_bank_common::{RateLimitMiddleware}; // From common

// core/deposit-service/src/middleware/auth.rs
// Authentication middleware for deposit service

use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpMessage, // HttpResponse, 
};
// use actix_web::http::StatusCode;
use bsv_bank_common::{auth::extract_bearer_token, JwtManager, ServiceError};
use std::future::{ready, Ready};
use std::rc::Rc;
use futures_util::future::LocalBoxFuture;

pub struct AuthMiddleware {
    jwt_manager: Rc<JwtManager>,
}

impl AuthMiddleware {
    pub fn new(jwt_manager: JwtManager) -> Self {
        Self {
            jwt_manager: Rc::new(jwt_manager),
        }
    }
}

impl<S, B> Transform<S, ServiceRequest> for AuthMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = AuthMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(AuthMiddlewareService {
            service: Rc::new(service),
            jwt_manager: self.jwt_manager.clone(),
        }))
    }
}

pub struct AuthMiddlewareService<S> {
    service: Rc<S>,
    jwt_manager: Rc<JwtManager>,
}

impl<S, B> Service<ServiceRequest> for AuthMiddlewareService<S>
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
        let jwt_manager = self.jwt_manager.clone();
        let service = self.service.clone();

        Box::pin(async move {
            let path = req.path();
            
            // Skip auth for public endpoints
            let public_paths = [
                "/health",
                "/liveness",
                "/readiness",
                "/metrics",
                "/register",
                "/login",
                "/refresh",
            ];
            
            if public_paths.iter().any(|p| path.starts_with(p)) {
                // Skip authentication for public routes
                return service.call(req).await;
            }

            // Extract Authorization header
            let auth_header = req
                .headers()
                .get("Authorization")
                .and_then(|h| h.to_str().ok());

            match auth_header {
                Some(header) => {
                    // Extract and verify token
                    match extract_bearer_token(header) {
                        Ok(token) => match jwt_manager.verify_token(&token) {
                            Ok(claims) => {
                                // Add claims to request extensions
                                req.extensions_mut().insert(claims);
                                service.call(req).await
                            }
                            Err(e) => Err(ServiceError::from(e).into()),
                        },
                        Err(e) => Err(ServiceError::from(e).into()),
                    }
                }
                // None => {
                //     // Check for API key
                //     let api_key = req.headers().get("X-API-Key").and_then(|h| h.to_str().ok());

                //     if api_key.is_some() {
                //         // TODO: Implement API key validation
                //         // For now, reject API key auth
                //         Err(ServiceError::Unauthorized.into())
                //     } else {
                //         Err(ServiceError::Unauthorized.into())
                //     }
                // }

                None => {
                    // Missing token → return 401
                    Err(ServiceError::Unauthorized.into())
                }

                // // Chatgpt attempt (aborted for now)

                // None => {
                //     // Check for API key
                //     let api_key = req.headers().get("X-API-Key").and_then(|h| h.to_str().ok());

                //     if api_key.is_some() {
                //         // TODO: Implement API key validation
                //         // For now, reject API key auth
                //         let resp = HttpResponse::Unauthorized().finish();
                //         Ok(req.into_response(resp.map_into_right_body()))
                //     } else {
                //         // Missing Authorization header → return 401
                //         let resp = HttpResponse::Unauthorized().finish();
                //         Ok(req.into_response(resp.map_into_right_body()))
                //     }
                // }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, web, App, HttpResponse}; // , error::ResponseError
    use actix_web::http::StatusCode;
    use bsv_bank_common::JwtManager;

    async fn test_handler() -> HttpResponse {
        HttpResponse::Ok().body("success")
    }

    #[actix_web::test]
    async fn test_auth_middleware_allows_health_endpoint() {
        let jwt_manager = JwtManager::new("test-secret".to_string());
        let app = test::init_service(
            App::new()
                .wrap(AuthMiddleware::new(jwt_manager))
                .route("/health", web::get().to(test_handler)),
        )
        .await;

        let req = test::TestRequest::get().uri("/health").to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }

    #[actix_web::test]
    async fn test_auth_middleware_allows_register_endpoint() {
        let jwt_manager = JwtManager::new("test-secret".to_string());
        let app = test::init_service(
            App::new()
                .wrap(AuthMiddleware::new(jwt_manager))
                .route("/register", web::post().to(test_handler)),
        )
        .await;

        let req = test::TestRequest::post().uri("/register").to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }

    #[actix_web::test]
    async fn test_auth_middleware_allows_login_endpoint() {
        let jwt_manager = JwtManager::new("test-secret".to_string());
        let app = test::init_service(
            App::new()
                .wrap(AuthMiddleware::new(jwt_manager))
                .route("/login", web::post().to(test_handler)),
        )
        .await;

        let req = test::TestRequest::post().uri("/login").to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }

    // #[actix_web::test]
    // async fn test_auth_middleware_rejects_missing_token() {
    //     let jwt_manager = JwtManager::new("test-secret".to_string());
    //     let app = test::init_service(
    //         App::new()
    //             .wrap(AuthMiddleware::new(jwt_manager))
    //             .route("/protected", web::get().to(test_handler)),
    //     )
    //     .await;

    //     let req = test::TestRequest::get().uri("/protected").to_request();
    //     let resp = test::call_service(&app, req).await;
    //     assert_eq!(resp.status(), 401);
    // }

    #[actix_web::test]
    async fn test_auth_middleware_rejects_missing_token() {
        
        let jwt_manager = JwtManager::new("test-secret".to_string());
        let app = test::init_service(
            App::new()
                .wrap(AuthMiddleware::new(jwt_manager))
                .route("/protected", web::get().to(test_handler)),
        )
        .await;

        let req = test::TestRequest::get().uri("/protected").to_request();
        let err = test::try_call_service(&app, req)
            .await
            .expect_err("Expected service to return error");
        
        let response = err.error_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[actix_web::test]
    async fn test_auth_middleware_accepts_valid_token() {
        let jwt_manager = JwtManager::new("test-secret".to_string());
        let token = jwt_manager
            .create_token("test@example.com", vec!["read".to_string()], 24)
            .unwrap();

        let app = test::init_service(
            App::new()
                .wrap(AuthMiddleware::new(jwt_manager))
                .route("/protected", web::get().to(test_handler)),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/protected")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }
}