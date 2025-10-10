// HTTP Interface with L402 Payment Support via ngx_l402
//
// This module provides L402-aware HTTP endpoints that work with ngx_l402 module.
// 
// Flow:
// 1. Client makes request → nginx (with ngx_l402)
// 2. ngx_l402 validates Cashu payment
// 3. ngx_l402 forwards request with headers:
//    - X-Cashu: Cashu <token>  OR  Authorization: L402 <token>
// 4. This module extracts token from headers
// 5. Decodes token to get payment amount in msats
// 6. Calculates pod duration: amount ÷ tier_rate
// 7. Creates pod for calculated duration
//
// Supported header formats from ngx_l402:
// - X-Cashu: Cashu cashuAeyJ0b2tlbiI6...
// - Authorization: L402 cashuAeyJ0b2tlbiI6...

use anyhow::Result;
use std::sync::Arc;
use tracing::{info, error, warn};
use axum::{
    extract::State,
    http::{StatusCode, HeaderMap},
    response::{Json, IntoResponse, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};

use crate::pod_provisioning::PodProvisioningService;

/// L402 payment information extracted from headers
#[derive(Debug, Clone)]
pub struct L402Payment {
    pub token: String,
    pub amount_msats: u64,
}

/// Extract Cashu token and decode amount from request headers
/// 
/// Supports formats from ngx_l402:
/// 1. X-Cashu: Cashu <token>
/// 2. Authorization: L402 <token>
async fn extract_l402_payment(headers: &HeaderMap) -> Option<L402Payment> {
    use crate::sidecar_service::extract_token_value;
    
    let mut cashu_token: Option<String> = None;
    
    // Try X-Cashu header: "Cashu <token>"
    if let Some(cashu_header) = headers.get("x-cashu") {
        if let Ok(cashu_str) = cashu_header.to_str() {
            // Extract token after "Cashu " prefix
            if cashu_str.starts_with("Cashu ") || cashu_str.starts_with("cashu ") {
                cashu_token = Some(cashu_str[6..].to_string());
                info!("Found Cashu token in X-Cashu header");
            } else {
                // Maybe it's just the token without prefix
                cashu_token = Some(cashu_str.to_string());
                info!("Found token in X-Cashu header (no prefix)");
            }
        }
    }
    
    // Try Authorization header: "L402 <token>"
    if cashu_token.is_none() {
        if let Some(auth_header) = headers.get("authorization") {
            if let Ok(auth_str) = auth_header.to_str() {
                if auth_str.starts_with("L402 ") || auth_str.starts_with("l402 ") {
                    cashu_token = Some(auth_str[5..].trim().to_string());
                    info!("Found Cashu token in Authorization header");
                }
            }
        }
    }
    
    // If we found a token, decode it to get the amount
    if let Some(token) = cashu_token {
        match extract_token_value(&token).await {
            Ok(amount_msats) => {
                info!("Decoded Cashu token: {} msats", amount_msats);
                return Some(L402Payment {
                    token,
                    amount_msats,
                });
            }
            Err(e) => {
                warn!("Failed to decode Cashu token: {}", e);
                return None;
            }
        }
    }
    
    None
}

/// Run the HTTP interface with L402 support
pub async fn run_http_l402_interface(service: Arc<PodProvisioningService>) -> Result<()> {
    info!("🌐 Starting HTTP interface with L402 support...");

    let bind_addr = std::env::var("HTTP_BIND_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:8080".to_string());

    info!("✅ HTTP+L402 interface ready - listening on http://{}", bind_addr);

    // Create the router
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/offers", get(get_offers))
        .route("/pods/status", post(get_pod_status))
        .route("/pods/spawn", post(spawn_pod_l402))
        .route("/pods/topup", post(topup_pod_l402))
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
        "service": "paygress-l402",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

/// Get available offers (no payment required)
async fn get_offers(State(service): State<Arc<PodProvisioningService>>) -> Result<Json<serde_json::Value>, StatusCode> {
    let request = crate::pod_provisioning::GetOffersTool {};
    
    match service.get_offers(request).await {
        Ok(response) => {
            let offers_json = serde_json::json!({
                "minimum_duration_seconds": response.minimum_duration_seconds,
                "whitelisted_mints": response.whitelisted_mints,
                "pod_specs": response.pod_specs,
                "payment_info": {
                    "method": "L402",
                    "accepted_tokens": ["cashu"],
                    "header_format": "Authorization: L402 token=<cashu_token> amount=<msats>",
                    "alternative_headers": {
                        "X-Payment-Token": "<cashu_token>",
                        "X-Payment-Amount": "<msats>"
                    }
                }
            });
            Ok(Json(offers_json))
        }
        Err(e) => {
            error!("Failed to get offers: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get pod status (no payment required)
async fn get_pod_status(
    State(service): State<Arc<PodProvisioningService>>,
    Json(request): Json<GetPodStatusHttpRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    info!("📨 Received get pod status request via HTTP");

    let status_tool = crate::pod_provisioning::GetPodStatusTool {
        pod_npub: request.pod_npub,
    };

    match service.get_pod_status(status_tool).await {
        Ok(response) => {
            let response_json = serde_json::json!({
                "success": response.success,
                "message": response.message,
                "pod_npub": response.pod_npub,
                "found": response.found,
                "created_at": response.created_at,
                "expires_at": response.expires_at,
                "time_remaining_seconds": response.time_remaining_seconds,
                "pod_spec_name": response.pod_spec_name,
                "cpu_millicores": response.cpu_millicores,
                "memory_mb": response.memory_mb,
                "status": response.status
            });
            Ok(Json(response_json))
        }
        Err(e) => {
            error!("Failed to get pod status: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Spawn a new pod with L402 payment support
async fn spawn_pod_l402(
    State(service): State<Arc<PodProvisioningService>>,
    headers: HeaderMap,
    Json(mut request): Json<SpawnPodHttpRequest>,
) -> Response {
    info!("📨 Received spawn pod request via HTTP+L402");

    // Check if payment token is provided in request body
    let has_token_in_body = !request.cashu_token.is_empty();

    // Try to extract L402 payment from headers (ngx_l402 provides this)
    if let Some(l402_payment) = extract_l402_payment(&headers).await {
        info!("L402 payment detected: {} msats", l402_payment.amount_msats);
        
        // Override token from header if not in body
        if !has_token_in_body {
            request.cashu_token = l402_payment.token.clone();
        }

        // Validate payment amount matches selected tier (optional)
        if let Some(pod_spec_id) = &request.pod_spec_id {
            let pod_spec = service.get_config().pod_specs.iter()
                .find(|s| &s.id == pod_spec_id);
            
            if let Some(spec) = pod_spec {
                let minimum_for_tier = spec.rate_msats_per_sec 
                    * service.get_config().minimum_pod_duration_seconds;
                
                if l402_payment.amount_msats < minimum_for_tier {
                    warn!("Insufficient payment for tier {}: {} < {} msats", 
                        spec.name, l402_payment.amount_msats, minimum_for_tier);
                    
                    return (
                        StatusCode::PAYMENT_REQUIRED,
                        [(
                            "WWW-Authenticate",
                            format!(
                                "L402 realm=\"paygress\", accept=\"cashu\", \
                                 minimum=\"{}\", tier=\"{}\", rate=\"{} msats/sec\"",
                                minimum_for_tier, spec.name, spec.rate_msats_per_sec
                            )
                        )],
                        Json(serde_json::json!({
                            "error": "Insufficient payment",
                            "tier": spec.name,
                            "minimum_required_msats": minimum_for_tier,
                            "payment_provided_msats": l402_payment.amount_msats,
                            "rate_msats_per_sec": spec.rate_msats_per_sec
                        }))
                    ).into_response();
                }
            }
        }
    } else if !has_token_in_body {
        // No payment provided at all - return 402 Payment Required
        warn!("No payment provided (neither L402 header nor request body)");
        
        let config = service.get_config();
        let basic_spec = config.pod_specs.first();
        
        let challenge = if let Some(spec) = basic_spec {
            format!(
                "L402 realm=\"paygress\", accept=\"cashu\", \
                 minimum=\"{}\", rate=\"{} msats/sec\"",
                spec.rate_msats_per_sec * config.minimum_pod_duration_seconds,
                spec.rate_msats_per_sec
            )
        } else {
            "L402 realm=\"paygress\", accept=\"cashu\"".to_string()
        };

        return (
            StatusCode::PAYMENT_REQUIRED,
            [("WWW-Authenticate", challenge)],
            Json(serde_json::json!({
                "error": "Payment Required",
                "message": "Please provide payment via L402 Authorization header or cashu_token in request body",
                "available_tiers": config.pod_specs.iter().map(|s| {
                    serde_json::json!({
                        "id": s.id,
                        "name": s.name,
                        "rate_msats_per_sec": s.rate_msats_per_sec,
                        "minimum_payment_msats": s.rate_msats_per_sec * config.minimum_pod_duration_seconds
                    })
                }).collect::<Vec<_>>()
            }))
        ).into_response();
    }

    // Process the spawn request (ngx_l402 already verified payment)
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
            (StatusCode::OK, Json(response_json)).into_response()
        }
        Err(e) => {
            error!("Failed to spawn pod: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to spawn pod",
                    "message": e.to_string()
                }))
            ).into_response()
        }
    }
}

/// Top up an existing pod with L402 payment support
async fn topup_pod_l402(
    State(service): State<Arc<PodProvisioningService>>,
    headers: HeaderMap,
    Json(mut request): Json<TopUpPodHttpRequest>,
) -> Response {
    info!("📨 Received topup pod request via HTTP+L402");

    // Check if payment token is provided in request body
    let has_token_in_body = !request.cashu_token.is_empty();

    // Try to extract L402 payment from headers (ngx_l402 provides this)
    if let Some(l402_payment) = extract_l402_payment(&headers).await {
        info!("L402 payment detected for top-up: {} msats", l402_payment.amount_msats);
        
        // Override token from header if not in body
        if !has_token_in_body {
            request.cashu_token = l402_payment.token;
        }
    } else if !has_token_in_body {
        // No payment provided at all - return 402 Payment Required
        warn!("No payment provided for top-up");
        
        return (
            StatusCode::PAYMENT_REQUIRED,
            [("WWW-Authenticate", "L402 realm=\"paygress\", accept=\"cashu\"")],
            Json(serde_json::json!({
                "error": "Payment Required",
                "message": "Please provide payment via L402 Authorization header or cashu_token in request body"
            }))
        ).into_response();
    }

    // Process the topup request (ngx_l402 already verified payment)
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
            (StatusCode::OK, Json(response_json)).into_response()
        }
        Err(e) => {
            error!("Failed to topup pod: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to topup pod",
                    "message": e.to_string()
                }))
            ).into_response()
        }
    }
}

/// HTTP request structures
#[derive(Debug, Deserialize)]
struct SpawnPodHttpRequest {
    #[serde(default)]
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
    #[serde(default)]
    pub cashu_token: String,
}

#[derive(Debug, Deserialize)]
struct GetPodStatusHttpRequest {
    pub pod_npub: String,
}

