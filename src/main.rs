// Paygress Sidecar Service - HTTP API for payment verification and pod provisioning
use std::sync::Arc;
use tokio::sync::Mutex;
use warp::Filter;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::pin::Pin;
use std::future::Future;

mod cashu;
mod nostr;

use cashu::{initialize_cashu, verify_cashu_token};
use nostr::{NostrRelaySubscriber, NostrEvent, default_relay_config};

#[derive(Debug, Serialize, Deserialize)]
struct PaymentVerifyRequest {
    token: String,
    amount: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct PaymentVerifyResponse {
    valid: bool,
    message: String,
    pod_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PodProvisionRequest {
    cashu_token: String,
    amount: u64,
    pod_description: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct PodProvisionResponse {
    success: bool,
    pod_id: Option<String>,
    message: String,
    cashu_verified: bool,
}

#[derive(Debug, Clone)]
struct PodInfo {
    pod_id: String,
    payment_amount: u64,
    provisioned_at: u64,
    cashu_token: String,
}

type SharedState = Arc<Mutex<HashMap<String, PodInfo>>>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Starting Paygress Sidecar Service...");

    // Initialize shared state
    let state: SharedState = Arc::new(Mutex::new(HashMap::new()));

    // Initialize Cashu
    let db_path = std::env::var("CASHU_DB_PATH").unwrap_or_else(|_| "./cashu.db".to_string());
    match initialize_cashu(&db_path).await {
        Ok(_) => println!("✅ Cashu initialized"),
        Err(e) => println!("⚠️ Cashu initialization warning: {}", e),
    }

    // Start Nostr listener in background
    let nostr_state = state.clone();
    tokio::spawn(async move {
        if let Err(e) = start_nostr_listener(nostr_state).await {
            eprintln!("❌ Nostr listener error: {}", e);
        }
    });

    // API Routes
    let health = warp::path("health")
        .and(warp::get())
        .map(|| warp::reply::json(&serde_json::json!({"status": "healthy", "service": "paygress-sidecar"})));

    let verify_payment = warp::path("verify")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_state(state.clone()))
        .and_then(handle_payment_verify);

    let provision_pod = warp::path("provision")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_state(state.clone()))
        .and_then(handle_pod_provision);

    let get_pods = warp::path("pods")
        .and(warp::get())
        .and(with_state(state.clone()))
        .and_then(handle_get_pods);

    // NGINX auth_request endpoint
    let auth_request = warp::path("auth")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::header::optional::<String>("x-original-uri"))
        .and(with_state(state.clone()))
        .and_then(handle_auth_request);

    let routes = health
        .or(verify_payment)
        .or(provision_pod)
        .or(get_pods)
        .or(auth_request)
        .with(warp::cors().allow_any_origin());

    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse::<u16>()
        .unwrap_or(3000);

    println!("🌐 Paygress API listening on port {}", port);
    println!("📡 Nostr listener starting in background...");

    warp::serve(routes)
        .run(([0, 0, 0, 0], port))
        .await;

    Ok(())
}

// Helper function to pass state to handlers
fn with_state(state: SharedState) -> impl Filter<Extract = (SharedState,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || state.clone())
}

// Payment verification endpoint
async fn handle_payment_verify(
    request: PaymentVerifyRequest,
    state: SharedState,
) -> Result<impl warp::Reply, warp::Rejection> {
    println!("🔍 Verifying payment: {} sats", request.amount);

    // Verify Cashu token
    let cashu_valid = match verify_cashu_token(&request.token, request.amount).await {
        Ok(valid) => valid,
        Err(e) => {
            println!("❌ Cashu verification error: {}", e);
            false
        }
    };

    let response = if cashu_valid {
        println!("✅ Payment verified: {} sats", request.amount);
        PaymentVerifyResponse {
            valid: true,
            message: "Payment verified".to_string(),
            pod_id: None,
        }
    } else {
        println!("❌ Payment failed: invalid token");
        PaymentVerifyResponse {
            valid: false,
            message: "Invalid Cashu token".to_string(),
            pod_id: None,
        }
    };

    Ok(warp::reply::json(&response))
}

// Pod provisioning endpoint
async fn handle_pod_provision(
    request: PodProvisionRequest,
    state: SharedState,
) -> Result<impl warp::Reply, warp::Rejection> {
    println!("🚀 Provisioning pod for {} sats", request.amount);

    // Verify Cashu token first
    let cashu_valid = match verify_cashu_token(&request.cashu_token, request.amount).await {
        Ok(valid) => valid,
        Err(e) => {
            println!("❌ Cashu verification error: {}", e);
            false
        }
    };

    if !cashu_valid {
        let response = PodProvisionResponse {
            success: false,
            pod_id: None,
            message: "Invalid Cashu token".to_string(),
            cashu_verified: false,
        };
        return Ok(warp::reply::json(&response));
    }

    // Generate pod ID
    let pod_id = format!("pod-{:x}", rand::random::<u64>());

    // Provision pod (simplified for demo)
    match provision_pod_k8s(&pod_id, &request.pod_description).await {
        Ok(_) => {
            // Store pod info
            let pod_info = PodInfo {
                pod_id: pod_id.clone(),
                payment_amount: request.amount,
                provisioned_at: chrono::Utc::now().timestamp() as u64,
                cashu_token: request.cashu_token,
            };

            let mut pods = state.lock().await;
            pods.insert(pod_id.clone(), pod_info);

            println!("✅ Pod provisioned: {}", pod_id);

            let response = PodProvisionResponse {
                success: true,
                pod_id: Some(pod_id),
                message: "Pod provisioned successfully".to_string(),
                cashu_verified: true,
            };
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            println!("❌ Pod provisioning failed: {}", e);
            let response = PodProvisionResponse {
                success: false,
                pod_id: None,
                message: format!("Pod provisioning failed: {}", e),
                cashu_verified: true,
            };
            Ok(warp::reply::json(&response))
        }
    }
}

// Get all provisioned pods
async fn handle_get_pods(state: SharedState) -> Result<impl warp::Reply, warp::Rejection> {
    let pods = state.lock().await;
    let pod_list: Vec<&PodInfo> = pods.values().collect();
    Ok(warp::reply::json(&pod_list))
}

// NGINX auth_request handler
async fn handle_auth_request(
    auth_header: Option<String>,
    original_uri: Option<String>,
    state: SharedState,
) -> Result<impl warp::Reply, warp::Rejection> {
    println!("🔐 Auth request for URI: {:?}", original_uri);

    // Extract token from Authorization header
    let token = match auth_header {
        Some(auth) if auth.starts_with("Bearer ") => {
            auth.strip_prefix("Bearer ").unwrap_or("").to_string()
        }
        _ => {
            println!("❌ No valid Authorization header");
            return Ok(warp::reply::with_status(
                "Unauthorized",
                warp::http::StatusCode::UNAUTHORIZED,
            ));
        }
    };

    // Verify payment (1000 sats for premium content)
    let amount = 1000;
    let cashu_valid = match verify_cashu_token(&token, amount).await {
        Ok(valid) => valid,
        Err(e) => {
            println!("❌ Cashu verification error: {}", e);
            false
        }
    };

    if cashu_valid {
        println!("✅ Auth request approved");
        Ok(warp::reply::with_status(
            "Authorized",
            warp::http::StatusCode::OK,
        ))
    } else {
        println!("❌ Auth request denied: invalid payment");
        Ok(warp::reply::with_status(
            "Payment Required",
            warp::http::StatusCode::PAYMENT_REQUIRED,
        ))
    }
}

// Nostr listener (runs in background)
async fn start_nostr_listener(state: SharedState) -> Result<(), Box<dyn std::error::Error>> {
    println!("📡 Starting Nostr listener...");

    let relay_config = default_relay_config();
    let nostr_client = NostrRelaySubscriber::new(relay_config).await?;

    // Handle incoming Nostr events
    let handler = move |event: NostrEvent| -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>> {
        let state = state.clone();
        Box::pin(async move {
            match handle_nostr_pod_provision(event, state).await {
                Ok(pod_id) => {
                    println!("🎉 Pod provisioned via Nostr: {}", pod_id);
                    Ok(())
                }
                Err(e) => {
                    println!("❌ Nostr pod provisioning failed: {}", e);
                    Err(anyhow::anyhow!(e))
                }
            }
        })
    };

    nostr_client.subscribe_to_pod_events(handler).await?;
    Ok(())
}

// Handle Nostr event for pod provisioning
async fn handle_nostr_pod_provision(
    event: NostrEvent,
    state: SharedState,
) -> Result<String, String> {
    println!("📨 Processing Nostr event: {}", event.id);

    // Parse event content
    let content: serde_json::Value = serde_json::from_str(&event.content)
        .map_err(|e| format!("Failed to parse event: {}", e))?;

    let cashu_token = content.get("cashu_token")
        .and_then(|v| v.as_str())
        .ok_or("Missing cashu_token")?;

    let amount = content.get("amount")
        .and_then(|v| v.as_u64())
        .ok_or("Missing amount")?;

    // Verify Cashu token
    let cashu_valid = verify_cashu_token(cashu_token, amount).await
        .map_err(|e| format!("Cashu verification failed: {}", e))?;

    if !cashu_valid {
        return Err("Invalid Cashu token".to_string());
    }

    // Generate pod ID
    let pod_id = format!("nostr-pod-{:x}", rand::random::<u64>());

    // Get pod description from event
    let pod_description = content.get("pod_description")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({"image": "nginx:alpine"}));

    // Provision pod
    provision_pod_k8s(&pod_id, &pod_description).await
        .map_err(|e| format!("Pod provisioning failed: {}", e))?;

    // Store pod info
    let pod_info = PodInfo {
        pod_id: pod_id.clone(),
        payment_amount: amount,
        provisioned_at: chrono::Utc::now().timestamp() as u64,
        cashu_token: cashu_token.to_string(),
    };

    let mut pods = state.lock().await;
    pods.insert(pod_id.clone(), pod_info);

    println!("✅ Nostr pod provisioned: {}", pod_id);
    Ok(pod_id)
}

// Simplified Kubernetes pod provisioning
async fn provision_pod_k8s(
    pod_id: &str,
    _spec: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Provisioning Kubernetes pod: {}", pod_id);

    // In production, this would use the Kubernetes API
    // For demo, we simulate pod creation
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    println!("✅ Pod {} ready", pod_id);
    Ok(())
}
