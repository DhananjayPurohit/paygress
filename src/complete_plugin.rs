use axum::{
    routing::{get, post},
    Router,
    extract::{Query, State},
    Json,
    http::{StatusCode, HeaderMap, HeaderValue},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, warn, error};
use std::collections::{HashMap, BTreeMap};

use crate::{cashu, initialize_cashu, NostrRelaySubscriber, default_relay_config};

// Complete plugin state with all services
#[derive(Clone)]
pub struct CompletePluginState {
    pub cashu_db_path: String,
    pub nostr_client: Option<Arc<NostrRelaySubscriber>>,
    pub k8s_client: Option<Arc<KubernetesService>>,
    pub config: PluginConfig,
}

#[derive(Clone, Debug)]
pub struct PluginConfig {
    pub cashu_db_path: String,
    pub enable_pod_provisioning: bool,
    pub enable_nostr_events: bool,
    pub default_pod_image: String,
    pub pod_namespace: String,
    pub nostr_relays: Vec<String>,
    pub nostr_secret_key: Option<String>,
}

// Query parameters for auth requests
#[derive(Debug, Deserialize)]
pub struct AuthQuery {
    pub token: Option<String>,
    pub amount: Option<i64>,
    pub service: Option<String>,
    pub namespace: Option<String>,
    pub image: Option<String>,
    pub create_pod: Option<bool>,
}

// Response with additional metadata
#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub allowed: bool,
    pub reason: String,
    pub pod_name: Option<String>,
    pub nostr_event_id: Option<String>,
    pub amount_verified: Option<i64>,
}

// Kubernetes service wrapper
pub struct KubernetesService {
    client: kube::Client,
}

impl KubernetesService {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let client = kube::Client::try_default().await?;
        Ok(Self { client })
    }

    pub async fn create_pod(
        &self,
        namespace: &str,
        name: &str,
        image: &str,
        labels: HashMap<String, String>,
    ) -> Result<String, Box<dyn std::error::Error>> {
        use k8s_openapi::api::core::v1::{Container, Pod, PodSpec};
        use kube::api::PostParams;
        use kube::Api;

        let pods: Api<Pod> = Api::namespaced(self.client.clone(), namespace);

        let mut pod_labels = BTreeMap::new();
        pod_labels.insert("app".to_string(), "paygress-provisioned".to_string());
        pod_labels.insert("payment-verified".to_string(), "true".to_string());
        for (k, v) in labels {
            pod_labels.insert(k, v);
        }

        let pod = Pod {
            metadata: kube::core::ObjectMeta {
                name: Some(name.to_string()),
                labels: Some(pod_labels),
                annotations: Some(BTreeMap::from([
                    ("paygress.io/provisioned-at".to_string(), chrono::Utc::now().to_rfc3339()),
                    ("paygress.io/payment-method".to_string(), "cashu".to_string()),
                ])),
                ..Default::default()
            },
            spec: Some(PodSpec {
                containers: vec![Container {
                    name: name.to_string(),
                    image: Some(image.to_string()),
                    ..Default::default()
                }],
                restart_policy: Some("Never".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };

        let pp = PostParams::default();
        pods.create(&pp, &pod).await?;
        
        info!(pod_name = %name, namespace = %namespace, image = %image, "Created pod via payment");
        Ok(name.to_string())
    }
}

impl CompletePluginState {
    pub async fn new(config: PluginConfig) -> Result<Self, Box<dyn std::error::Error>> {
        // Initialize Cashu
        initialize_cashu(&config.cashu_db_path).await
            .map_err(|e| format!("Cashu init failed: {}", e))?;

        // Initialize Nostr client if enabled
        let nostr_client = if config.enable_nostr_events {
            if let Some(secret_key) = &config.nostr_secret_key {
                let mut relay_config = default_relay_config();
                relay_config.urls = config.nostr_relays.clone();
                relay_config.secret_key = Some(secret_key.clone());

                match NostrRelaySubscriber::new(relay_config).await {
                    Ok(client) => {
                        info!("Nostr client initialized successfully");
                        Some(Arc::new(client))
                    },
                    Err(e) => {
                        warn!(error = %e, "Failed to initialize Nostr client, continuing without it");
                        None
                    }
                }
            } else {
                warn!("Nostr enabled but no secret key provided");
                None
            }
        } else {
            None
        };

        // Initialize Kubernetes client if enabled
        let k8s_client = if config.enable_pod_provisioning {
            match KubernetesService::new().await {
                Ok(client) => {
                    info!("Kubernetes client initialized successfully");
                    Some(Arc::new(client))
                },
                Err(e) => {
                    warn!(error = %e, "Failed to initialize Kubernetes client, continuing without it");
                    None
                }
            }
        } else {
            None
        };

        Ok(Self {
            cashu_db_path: config.cashu_db_path.clone(),
            nostr_client,
            k8s_client,
            config,
        })
    }
}

// Create the complete plugin router
pub fn create_complete_plugin_router(state: CompletePluginState) -> Router {
    Router::new()
        .route("/healthz", get(health_check))
        .route("/auth", get(complete_auth))
        .route("/provision", post(provision_service))
        .with_state(state)
}

// Health check with feature status
async fn health_check(State(state): State<CompletePluginState>) -> impl IntoResponse {
    let status = serde_json::json!({
        "status": "healthy",
        "service": "paygress-complete-plugin",
        "version": env!("CARGO_PKG_VERSION"),
        "features": {
            "cashu_verification": true,
            "pod_provisioning": state.config.enable_pod_provisioning && state.k8s_client.is_some(),
            "nostr_events": state.config.enable_nostr_events && state.nostr_client.is_some(),
        }
    });
    Json(status)
}

// Complete authentication with all features
async fn complete_auth(
    Query(params): Query<AuthQuery>,
    State(state): State<CompletePluginState>,
) -> Response {
    info!("Complete auth request: {:?}", params);

    // 1. Extract and validate token
    let Some(token) = params.token else {
        warn!("No Cashu token provided");
        return create_auth_response(false, "Missing Cashu token", None, None, None);
    };

    let amount = params.amount.unwrap_or(1000);

    // 2. Verify Cashu payment
    let payment_valid = match cashu::verify_cashu_token(&token, amount).await {
        Ok(true) => {
            info!("✅ Payment verified: {} msat", amount);
            true
        },
        Ok(false) => {
            warn!("❌ Payment verification failed");
            return create_auth_response(false, "Payment verification failed", None, None, Some(amount));
        },
        Err(e) => {
            error!("💥 Payment verification error: {}", e);
            return create_auth_response(false, "Payment verification error", None, None, Some(amount));
        }
    };

    if !payment_valid {
        return create_auth_response(false, "Invalid payment", None, None, Some(amount));
    }

    // 3. Publish Nostr event if enabled
    let nostr_event_id = if state.config.enable_nostr_events {
        if let Some(nostr_client) = &state.nostr_client {
            match publish_payment_event(nostr_client, &token, amount).await {
                Ok(event_id) => Some(event_id),
                Err(e) => {
                    warn!(error = %e, "Failed to publish Nostr event, continuing");
                    None
                }
            }
        } else {
            None
        }
    } else {
        None
    };

    // 4. Provision pod if requested and enabled
    let pod_name = if params.create_pod.unwrap_or(false) && state.config.enable_pod_provisioning {
        if let Some(k8s_client) = &state.k8s_client {
            let service_name = params.service.as_deref().unwrap_or("payment-service");
            let namespace = params.namespace.as_deref().unwrap_or(&state.config.pod_namespace);
            let image = params.image.as_deref().unwrap_or(&state.config.default_pod_image);
            let pod_name = format!("{}-{}", service_name, uuid::Uuid::new_v4().to_string()[..8].to_string());

            let labels = HashMap::from([
                ("service".to_string(), service_name.to_string()),
                ("payment-amount".to_string(), amount.to_string()),
            ]);

            match k8s_client.create_pod(namespace, &pod_name, image, labels).await {
                Ok(name) => Some(name),
                Err(e) => {
                    error!(error = %e, "Failed to create pod, allowing request anyway");
                    None
                }
            }
        } else {
            None
        }
    } else {
        None
    };

    // 5. Return success with metadata
    create_auth_response(true, "Payment verified and processed", pod_name, nostr_event_id, Some(amount))
}

// Dedicated provisioning endpoint
async fn provision_service(
    State(state): State<CompletePluginState>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    info!("Service provisioning request");

    // Extract parameters
    let token = payload.get("token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Missing token"}))))?;

    let amount = payload.get("amount").and_then(|v| v.as_i64()).unwrap_or(1000);

    // Verify payment
    match cashu::verify_cashu_token(token, amount).await {
        Ok(true) => {},
        Ok(false) => {
            return Err((StatusCode::PAYMENT_REQUIRED, Json(serde_json::json!({"error": "Payment verification failed"}))));
        },
        Err(e) => {
            return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": format!("Verification error: {}", e)}))));
        }
    }

    // Provision service
    let service_name = payload.get("service").and_then(|v| v.as_str()).unwrap_or("default-service");
    let namespace = payload.get("namespace").and_then(|v| v.as_str()).unwrap_or(&state.config.pod_namespace);
    let image = payload.get("image").and_then(|v| v.as_str()).unwrap_or(&state.config.default_pod_image);

    let response = if let Some(k8s_client) = &state.k8s_client {
        let pod_name = format!("{}-{}", service_name, uuid::Uuid::new_v4().to_string()[..8].to_string());
        let labels = HashMap::from([("service".to_string(), service_name.to_string())]);

        match k8s_client.create_pod(namespace, &pod_name, image, labels).await {
            Ok(_) => serde_json::json!({
                "status": "success",
                "pod_name": pod_name,
                "service": service_name,
                "namespace": namespace,
                "image": image
            }),
            Err(e) => {
                return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": format!("Pod creation failed: {}", e)}))));
            }
        }
    } else {
        serde_json::json!({
            "status": "success",
            "message": "Payment verified (pod provisioning disabled)"
        })
    };

    Ok(Json(response))
}

// Helper functions
async fn publish_payment_event(
    _nostr_client: &NostrRelaySubscriber,
    _token: &str,
    _amount: i64,
) -> Result<String, Box<dyn std::error::Error>> {
    // For now, return a placeholder - you can implement actual Nostr publishing here
    // This would require adding a publish method to NostrRelaySubscriber
    Ok("placeholder-event-id".to_string())
}

fn create_auth_response(
    allowed: bool,
    reason: &str,
    pod_name: Option<String>,
    nostr_event_id: Option<String>,
    amount: Option<i64>,
) -> Response {
    let status = if allowed { StatusCode::OK } else { StatusCode::UNAUTHORIZED };
    let mut response = status.into_response();

    // Add custom headers
    let headers = response.headers_mut();
    
    if let Ok(reason_value) = HeaderValue::from_str(reason) {
        headers.insert("X-Auth-Reason", reason_value);
    }

    if allowed {
        headers.insert("X-Payment-Verified", HeaderValue::from_static("true"));
        
        if let Some(amount) = amount {
            if let Ok(amount_value) = HeaderValue::from_str(&amount.to_string()) {
                headers.insert("X-Payment-Amount", amount_value);
            }
        }

        if let Some(pod) = &pod_name {
            if let Ok(pod_value) = HeaderValue::from_str(pod) {
                headers.insert("X-Provisioned-Pod", pod_value);
            }
        }

        if let Some(event_id) = &nostr_event_id {
            if let Ok(event_value) = HeaderValue::from_str(event_id) {
                headers.insert("X-Nostr-Event", event_value);
            }
        }
    }

    response
}

// Initialize the complete plugin
pub async fn start_complete_plugin(bind_addr: &str, config: PluginConfig) -> Result<(), Box<dyn std::error::Error>> {
    // Create plugin state
    let state = CompletePluginState::new(config.clone()).await?;
    
    // Create router
    let app = create_complete_plugin_router(state);
    
    // Print startup info
    println!("🚀 Starting Complete Paygress Ingress Plugin");
    println!("📍 Listening on: {}", bind_addr);
    println!("✨ Features enabled:");
    println!("   🔐 Cashu verification: ✅");
    println!("   ☸️  Pod provisioning: {}", if config.enable_pod_provisioning { "✅" } else { "❌" });
    println!("   📡 Nostr events: {}", if config.enable_nostr_events { "✅" } else { "❌" });
    println!();
    println!("🔗 NGINX config: auth_url http://{}/auth", bind_addr);
    println!("📋 Endpoints:");
    println!("   GET  /healthz - Health check with feature status");
    println!("   GET  /auth    - Complete auth with payment + provisioning");
    println!("   POST /provision - Dedicated service provisioning");
    
    // Start server
    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    axum::serve(listener, app).await?;
    
    Ok(())
}
