// HTTP Interface for Paygress
//
// Provides REST API endpoints for pod provisioning
// using a shared PodProvisioningService instance.

use anyhow::Result;
use std::sync::Arc;
use tracing::{info, error, warn};
use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};

use crate::pod_provisioning::PodProvisioningService;

/// Run the HTTP interface
pub async fn run_http_interface(service: Arc<PodProvisioningService>) -> Result<()> {
    info!("🌐 Starting HTTP interface...");

    let bind_addr = std::env::var("HTTP_BIND_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:8080".to_string());

    info!("✅ HTTP interface ready - listening on http://{}", bind_addr);

    // Create the router
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/offers", get(get_offers))
        .route("/pods/spawn", post(spawn_pod))
        .route("/pods/topup", post(topup_pod))
        .with_state(service);

    // Start the HTTP server
    let listener = tokio::net::TcpListener::bind(&bind_addr).await
        .map_err(|e| anyhow::anyhow!("Failed to bind to {}: {}", bind_addr, e))?;

    axum::serve(listener, app).await
        .map_err(|e| anyhow::anyhow!("HTTP server error: {}", e))?;

    Ok(())
}

/// Health check endpoint
async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "paygress",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

/// Get available offers
async fn get_offers(State(service): State<Arc<PodProvisioningService>>) -> Result<Json<serde_json::Value>, StatusCode> {
    let request = crate::pod_provisioning::GetOffersTool {};
    
    match service.get_offers(request).await {
        Ok(response) => {
            let offers_json = serde_json::json!({
                "minimum_duration_seconds": response.minimum_duration_seconds,
                "whitelisted_mints": response.whitelisted_mints,
                "pod_specs": response.pod_specs
            });
            Ok(Json(offers_json))
        }
        Err(e) => {
            error!("Failed to get offers: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}


/// Spawn a new pod
async fn spawn_pod(
    State(service): State<Arc<PodProvisioningService>>,
    Json(request): Json<SpawnPodHttpRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    info!("📨 Received spawn pod request via HTTP");

    let spawn_tool = crate::pod_provisioning::SpawnPodTool {
        cashu_token: request.cashu_token,
        pod_spec_id: request.pod_spec_id,
        pod_image: request.pod_image,
        ssh_username: request.ssh_username,
        ssh_password: request.ssh_password,
        user_pubkey: request.user_pubkey,
    };

    match service.spawn_pod(spawn_tool).await {
        Ok(response) => {
            let response_json = serde_json::json!({
                "success": response.success,
                "message": response.message,
                "pod_npub": response.pod_npub,
                "ssh_host": response.ssh_host,
                "ssh_port": response.ssh_port,
                "ssh_username": response.ssh_username,
                "ssh_password": response.ssh_password,
                "expires_at": response.expires_at,
                "pod_spec_name": response.pod_spec_name,
                "instructions": response.instructions
            });
            Ok(Json(response_json))
        }
        Err(e) => {
            error!("Failed to spawn pod: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Top up an existing pod
async fn topup_pod(
    State(service): State<Arc<PodProvisioningService>>,
    Json(request): Json<TopUpPodHttpRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    info!("📨 Received topup pod request via HTTP");

    let topup_tool = crate::pod_provisioning::TopUpPodTool {
        pod_npub: request.pod_npub,
        cashu_token: request.cashu_token,
    };

    match service.topup_pod(topup_tool).await {
        Ok(response) => {
            let response_json = serde_json::json!({
                "success": response.success,
                "message": response.message,
                "pod_npub": response.pod_npub,
                "extended_duration_seconds": response.extended_duration_seconds,
                "new_expires_at": response.new_expires_at
            });
            Ok(Json(response_json))
        }
        Err(e) => {
            error!("Failed to topup pod: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// HTTP request structures
#[derive(Debug, Deserialize)]
struct SpawnPodHttpRequest {
    pub cashu_token: String,
    pub pod_spec_id: Option<String>,
    pub pod_image: String,
    pub ssh_username: String,
    pub ssh_password: String,
    pub user_pubkey: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TopUpPodHttpRequest {
    pub pod_npub: String,
    pub cashu_token: String,
}
