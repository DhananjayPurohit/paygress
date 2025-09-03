use axum::{
    routing::get,
    Router,
    extract::Query,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use tracing::{info, warn, error};

use crate::{cashu, initialize_cashu};

// Query parameters for NGINX auth_request
#[derive(Debug, Deserialize)]
pub struct AuthQuery {
    pub token: Option<String>,
    pub amount: Option<i64>,
}

// Create simple NGINX auth router
pub fn create_nginx_auth_router() -> Router {
    Router::new()
        .route("/healthz", get(health_check))
        .route("/auth", get(nginx_auth))
}

// Health check endpoint
async fn health_check() -> impl IntoResponse {
    "OK"
}

// NGINX auth_request endpoint
async fn nginx_auth(Query(params): Query<AuthQuery>) -> Response {
    info!("NGINX auth request: {:?}", params);

    // Check if token is provided
    let Some(token) = params.token else {
        warn!("No Cashu token provided");
        return StatusCode::UNAUTHORIZED.into_response();
    };

    // Default amount to 1000 msat if not specified
    let amount = params.amount.unwrap_or(1000);

    // Verify Cashu token
    match cashu::verify_cashu_token(&token, amount).await {
        Ok(true) => {
            info!("✅ Payment verified: {} msat", amount);
            StatusCode::OK.into_response()
        },
        Ok(false) => {
            warn!("❌ Payment verification failed");
            StatusCode::PAYMENT_REQUIRED.into_response()
        },
        Err(e) => {
            error!("💥 Payment verification error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

// Initialize the auth service
pub async fn start_nginx_auth_service(bind_addr: &str, cashu_db_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Initialize Cashu database
    initialize_cashu(cashu_db_path).await?;
    
    // Create router
    let app = create_nginx_auth_router();
    
    // Start server
    println!("🚀 Starting NGINX Auth Service");
    println!("📍 Listening on: {}", bind_addr);
    println!("🔗 NGINX config: auth_url http://{}/auth", bind_addr);
    
    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    axum::serve(listener, app).await?;
    
    Ok(())
}
