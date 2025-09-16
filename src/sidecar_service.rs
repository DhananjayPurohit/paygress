use axum::{
    routing::{get, post},
    Router,
    extract::{Query, State, Path},
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, warn, error};
use std::collections::{HashMap, BTreeMap};
use tokio::time::{sleep, Duration};
use uuid::Uuid;
use chrono::{DateTime, Utc};

use crate::{cashu, initialize_cashu};

// Configuration for the sidecar service
#[derive(Clone, Debug)]
pub struct SidecarConfig {
    pub cashu_db_path: String,
    pub pod_namespace: String,
    pub payment_rate_sats_per_hour: u64, // Legacy field - now using 1 sat = 1 minute
    pub default_pod_duration_minutes: u64, // Default duration if not specified
    pub ssh_base_image: String, // Base image with SSH server
    pub ssh_port: u16,
    pub enable_cleanup_task: bool,
    pub whitelisted_mints: Vec<String>, // Allowed Cashu mint URLs
}

impl Default for SidecarConfig {
    fn default() -> Self {
        Self {
            cashu_db_path: "./cashu.db".to_string(),
            pod_namespace: "user-workloads".to_string(),
            payment_rate_sats_per_hour: 100, // 100 sats per hour
            default_pod_duration_minutes: 60, // 1 hour default
            ssh_base_image: "linuxserver/openssh-server:latest".to_string(),
            ssh_port: 2222,
            enable_cleanup_task: true,
            whitelisted_mints: vec![
                "https://mint.cashu.space".to_string(),
                "https://mint.f7z.io".to_string(),
                "https://legend.lnbits.com/cashu/api/v1".to_string(),
            ],
        }
    }
}

// Sidecar service state
#[derive(Clone)]
pub struct SidecarState {
    pub config: SidecarConfig,
    pub k8s_client: Arc<PodManager>,
    pub active_pods: Arc<tokio::sync::RwLock<HashMap<String, PodInfo>>>,
}

// Information about active pods
#[derive(Clone, Debug, Serialize)]
pub struct PodInfo {
    pub pod_name: String,
    pub namespace: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub ssh_port: u16,
    pub ssh_username: String,
    pub ssh_password: String,
    pub payment_amount_sats: u64,
    pub duration_minutes: u64,
    pub node_port: Option<u16>,
}

// Request to spawn a pod
#[derive(Debug, Deserialize)]
pub struct SpawnPodRequest {
    pub cashu_token: String,
    pub pod_image: Option<String>, // Optional custom image
    pub ssh_username: Option<String>, // Optional custom username
}

// Response from pod spawning
#[derive(Debug, Serialize)]
pub struct SpawnPodResponse {
    pub success: bool,
    pub message: String,
    pub pod_info: Option<PodInfo>,
}

// Auth query for ingress
#[derive(Debug, Deserialize)]
pub struct AuthQuery {
    pub token: Option<String>,
}

// Pod management service
pub struct PodManager {
    client: kube::Client,
}

impl PodManager {
    pub async fn new() -> Result<Self, String> {
        let client = kube::Client::try_default().await.map_err(|e| format!("Failed to create Kubernetes client: {}", e))?;
        Ok(Self { client })
    }

    pub async fn create_ssh_pod(
        &self,
        namespace: &str,
        pod_name: &str,
        image: &str,
        ssh_port: u16,
        username: &str,
        password: &str,
        duration_minutes: u64,
    ) -> Result<u16, String> {
        use k8s_openapi::api::core::v1::{
            Container, Pod, PodSpec, EnvVar, ContainerPort, Service, ServiceSpec, ServicePort,
            Volume, VolumeMount, EmptyDirVolumeSource,
        };
        use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
        use kube::api::PostParams;
        use kube::Api;

        let pods: Api<Pod> = Api::namespaced(self.client.clone(), namespace);
        let services: Api<Service> = Api::namespaced(self.client.clone(), namespace);
        let configmaps: Api<k8s_openapi::api::core::v1::ConfigMap> = Api::namespaced(self.client.clone(), namespace);

        // Create SSH environment variables
        let env_vars = vec![
            EnvVar {
                name: "PUID".to_string(),
                value: Some("1000".to_string()),
                value_from: None,
            },
            EnvVar {
                name: "PGID".to_string(),
                value: Some("1000".to_string()),
                value_from: None,
            },
            EnvVar {
                name: "TZ".to_string(),
                value: Some("Etc/UTC".to_string()),
                value_from: None,
            },
            EnvVar {
                name: "PUBLIC_KEY_FILE".to_string(),
                value: Some("/config/.ssh/authorized_keys".to_string()),
                value_from: None,
            },
            EnvVar {
                name: "USER_NAME".to_string(),
                value: Some(username.to_string()),
                value_from: None,
            },
            EnvVar {
                name: "USER_PASSWORD".to_string(),
                value: Some(password.to_string()),
                value_from: None,
            },
            EnvVar {
                name: "PASSWORD_ACCESS".to_string(),
                value: Some("true".to_string()),
                value_from: None,
            },
        ];

        // Create pod labels and annotations
        let mut labels = BTreeMap::new();
        labels.insert("app".to_string(), "paygress-ssh-pod".to_string());
        labels.insert("managed-by".to_string(), "paygress-sidecar".to_string());
        labels.insert("pod-type".to_string(), "ssh-access".to_string());
        labels.insert("pod-name".to_string(), pod_name.to_string());

        let mut annotations = BTreeMap::new();
        annotations.insert("paygress.io/created-at".to_string(), Utc::now().to_rfc3339());
        annotations.insert("paygress.io/expires-at".to_string(), 
            (Utc::now() + chrono::Duration::minutes(duration_minutes as i64)).to_rfc3339());
        annotations.insert("paygress.io/duration-minutes".to_string(), duration_minutes.to_string());
        annotations.insert("paygress.io/ssh-username".to_string(), username.to_string());

        // Create volumes
        let volumes = Vec::new();

        // Create containers
        let containers = vec![Container {
            name: "ssh-server".to_string(),
            image: Some(image.to_string()),
            ports: Some(vec![ContainerPort {
                container_port: ssh_port as i32,
                name: Some("ssh".to_string()),
                protocol: Some("TCP".to_string()),
                ..Default::default()
            }]),
            env: Some(env_vars),
            image_pull_policy: Some("IfNotPresent".to_string()),
            ..Default::default()
        }];


        // Create the pod
        let pod = Pod {
            metadata: kube::core::ObjectMeta {
                name: Some(pod_name.to_string()),
                labels: Some(labels.clone()),
                annotations: Some(annotations),
                ..Default::default()
            },
            spec: Some(PodSpec {
                containers,
                volumes: None,
                restart_policy: Some("Never".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };

        // Create the pod
        let pp = PostParams::default();
        pods.create(&pp, &pod).await.map_err(|e| format!("Failed to create pod: {}", e))?;

        // With host network, we don't need a service - SSH is directly accessible
        // But we'll create a simple service for compatibility
        let service = Service {
            metadata: kube::core::ObjectMeta {
                name: Some(format!("{}-ssh", pod_name)),
                labels: Some(labels),
                ..Default::default()
            },
            spec: Some(ServiceSpec {
                selector: Some(BTreeMap::from([
                    ("app".to_string(), "paygress-ssh-pod".to_string()),
                    ("pod-name".to_string(), pod_name.to_string()),
                ])),
                ports: Some(vec![ServicePort {
                    port: 2222, // Always use port 2222 for the service
                    target_port: Some(IntOrString::Int(2222)), // Always target port 2222 (SSH server port)
                    name: Some("ssh".to_string()),
                    protocol: Some("TCP".to_string()),
                    ..Default::default()
                }]),
                type_: Some("NodePort".to_string()), // Use NodePort for external access
                ..Default::default()
            }),
            ..Default::default()
        };

        let created_service = services.create(&pp, &service).await.map_err(|e| format!("Failed to create service: {}", e))?;

        // Get the actual node port that Kubernetes assigned
        let node_port = created_service
            .spec
            .and_then(|spec| spec.ports)
            .and_then(|ports| ports.first())
            .and_then(|port| port.node_port)
            .unwrap_or(2222) as u16;

        info!(
            pod_name = %pod_name, 
            namespace = %namespace, 
            duration_minutes = %duration_minutes,
            ssh_port = %ssh_port,
            username = %username,
            "Created SSH pod with service"
        );

        Ok(node_port)
    }

    pub async fn delete_pod(&self, namespace: &str, pod_name: &str) -> Result<(), String> {
        use kube::api::DeleteParams;
        use kube::Api;
        use k8s_openapi::api::core::v1::{Pod, Service, ConfigMap};

        let pods: Api<Pod> = Api::namespaced(self.client.clone(), namespace);
        let services: Api<Service> = Api::namespaced(self.client.clone(), namespace);
        let configmaps: Api<ConfigMap> = Api::namespaced(self.client.clone(), namespace);

        // Delete the pod
        let dp = DeleteParams::default();
        let _ = pods.delete(pod_name, &dp).await;

        // Delete the associated service
        let service_name = format!("{}-ssh", pod_name);
        let _ = services.delete(&service_name, &dp).await;


        info!(pod_name = %pod_name, namespace = %namespace, "Deleted pod and service");
        Ok(())
    }
}

impl SidecarState {
    pub async fn new(config: SidecarConfig) -> Result<Self, String> {
        // Initialize Cashu
        initialize_cashu(&config.cashu_db_path).await
            .map_err(|e| format!("Cashu init failed: {}", e))?;

        // Initialize Kubernetes client
        let k8s_client = Arc::new(PodManager::new().await?);

        let active_pods = Arc::new(tokio::sync::RwLock::new(HashMap::new()));

        Ok(Self {
            config,
            k8s_client,
            active_pods,
        })
    }

    // Calculate duration based on payment amount (configurable pricing)
    pub fn calculate_duration_from_payment(&self, payment_sats: u64) -> u64 {
        let sats_per_hour = self.config.payment_rate_sats_per_hour.max(1);
        
        // Fixed point calculation: (payment_sats * 60) / sats_per_hour
        // This preserves precision by multiplying by 60 first
        let duration_minutes = (payment_sats * 60) / sats_per_hour;
        
        duration_minutes
    }

    // Generate secure random password
    pub fn generate_password() -> String {
        use rand::Rng;
        const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        let mut rng = rand::thread_rng();
        (0..16)
            .map(|_| {
                let idx = rng.gen_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect()
    }

    // Generate unique SSH port for each pod
    pub fn generate_ssh_port(&self) -> u16 {
        // Always use the configured SSH port (2222) since that's what the SSH server runs on
        self.config.ssh_port
    }
}

// Extract token value in sats from Cashu token
pub async fn extract_token_value(token: &str) -> Result<u64, String> {
    use std::str::FromStr;
    
    // Decode the token to get its value
    let token_decoded = cdk::nuts::Token::from_str(token)
        .map_err(|e| format!("Failed to decode Cashu token: {}", e))?;
    
    // Check if the token is valid
    if token_decoded.proofs().is_empty() {
        return Err("Token has no proofs".to_string());
    }
    
    // Calculate total token amount
    let total_amount = token_decoded.value()
        .map_err(|e| format!("Failed to get token value: {}", e))?;

    // Check if the token unit is in millisatoshis or satoshis
    let total_amount_sats: u64 = match token_decoded.unit() {
        Some(unit) => match unit {
            cdk::nuts::CurrencyUnit::Sat => u64::from(total_amount),
            cdk::nuts::CurrencyUnit::Msat => u64::from(total_amount) / 1000, // Convert msat to sat
            _ => return Err(format!("Unsupported token unit: {:?}", unit)),
        },
        None => return Err("Token has no unit specified".to_string()),
    };
    
    Ok(total_amount_sats)
}

// Create router for the sidecar service
pub fn create_sidecar_router(state: SidecarState) -> Router {
    Router::new()
        .route("/healthz", get(health_check))
        .route("/auth", get(auth_check))
        .route("/spawn-pod", post(|State(state): State<SidecarState>, Json(request): Json<SpawnPodRequest>| async move {
            spawn_pod_handler(state, request).await
        }))
        .route("/pods", get(list_pods))
        .route("/pods/:pod_name", get(get_pod_info))
        .route("/pods/:pod_name/port-forward", get(get_port_forward_command))
        .with_state(state)
}

// Health check endpoint
async fn health_check(State(state): State<SidecarState>) -> impl IntoResponse {
    let active_count = state.active_pods.read().await.len();
    
    let status = serde_json::json!({
        "status": "healthy",
        "service": "paygress-sidecar",
        "version": env!("CARGO_PKG_VERSION"),
        "config": {
            "payment_model": "1 sat = 1 minute",
            "minimum_payment": "1 sat",
            "namespace": state.config.pod_namespace,
        },
        "active_pods": active_count,
    });
    
    Json(status)
}

// Auth check for ingress (simplified)
async fn auth_check(
    Query(params): Query<AuthQuery>,
    State(state): State<SidecarState>,
) -> Response {
    info!("Auth check request: {:?}", params);

    let Some(token) = params.token else {
        warn!("No Cashu token provided");
        return StatusCode::UNAUTHORIZED.into_response();
    };

    // Extract payment amount from token
    let payment_amount_sats = match extract_token_value(&token).await {
        Ok(sats) => sats,
        Err(e) => {
            warn!("Failed to decode token: {}", e);
            return StatusCode::BAD_REQUEST.into_response();
        }
    };

    // Calculate duration based on payment
    let duration_minutes = state.calculate_duration_from_payment(payment_amount_sats);
    
    if duration_minutes == 0 {
        warn!("❌ Insufficient payment: {} sats", payment_amount_sats);
        return StatusCode::PAYMENT_REQUIRED.into_response();
    }

    // Verify Cashu token validity (not amount, just validity)
    match cashu::verify_cashu_token(&token, 1, &state.config.whitelisted_mints).await {
        Ok(true) => {
            info!("✅ Payment verified: {} sats → {} minutes", payment_amount_sats, duration_minutes);
            StatusCode::OK.into_response()
        },
        Ok(false) => {
            warn!("❌ Token verification failed");
            StatusCode::PAYMENT_REQUIRED.into_response()
        },
        Err(e) => {
            error!("💥 Payment verification error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

// Simple test handler
async fn spawn_pod_simple(
    State(_state): State<SidecarState>,
    Json(_request): Json<SpawnPodRequest>,
) -> Json<SpawnPodResponse> {
    Json(SpawnPodResponse {
        success: false,
        message: "Not implemented yet".to_string(),
        pod_info: None,
    })
}

// Spawn a new pod with SSH access - handler function
async fn spawn_pod_handler(
    state: SidecarState,
    request: SpawnPodRequest,
) -> Response {
    info!("Pod spawn request received");

    // First, decode the token to get the payment amount
    let payment_amount_sats = match extract_token_value(&request.cashu_token).await {
        Ok(sats) => sats,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, Json(SpawnPodResponse {
                success: false,
                message: format!("Failed to decode Cashu token: {}", e),
                pod_info: None,
            })).into_response();
        }
    };

    // Calculate duration based on payment amount
    let duration_minutes = state.calculate_duration_from_payment(payment_amount_sats);
    
    if duration_minutes == 0 {
        return (StatusCode::PAYMENT_REQUIRED, Json(SpawnPodResponse {
            success: false,
            message: "Insufficient payment. Minimum required: 1 sat for 1 minute".to_string(),
            pod_info: None,
        })).into_response();
    }

    info!("💰 Payment: {} sats → ⏱️ Duration: {} minutes", payment_amount_sats, duration_minutes);

    // Verify payment (now we just need to verify the token is valid, not check amount)
    match cashu::verify_cashu_token(&request.cashu_token, 1, &state.config.whitelisted_mints).await { // Just verify with 1 msat to check validity
        Ok(false) => {
            return (StatusCode::PAYMENT_REQUIRED, Json(SpawnPodResponse {
                success: false,
                message: "Cashu token verification failed - invalid token".to_string(),
                pod_info: None,
            })).into_response();
        },
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(SpawnPodResponse {
                success: false,
                message: format!("Payment verification error: {}", e),
                pod_info: None,
            })).into_response();
        },
        Ok(true) => {
            info!("✅ Payment verified: {} sats for {} minutes", payment_amount_sats, duration_minutes);
        }
    }

    // Generate pod details
    let pod_name = format!("ssh-pod-{}", Uuid::new_v4().to_string()[..8].to_string());
    let username = request.ssh_username.unwrap_or_else(|| format!("user-{}", &pod_name[8..16]));
    let password = SidecarState::generate_password();
    let image = request.pod_image.unwrap_or_else(|| state.config.ssh_base_image.clone());
    let ssh_port = state.generate_ssh_port(); // Generate unique port for this pod

    let now = Utc::now();
    let expires_at = now + chrono::Duration::minutes(duration_minutes as i64);

    // Create the pod
    match state.k8s_client.create_ssh_pod(
        &state.config.pod_namespace,
        &pod_name,
        &image,
        ssh_port,
        &username,
        &password,
        duration_minutes,
    ).await {
        Ok(node_port) => {
            let pod_info = PodInfo {
                pod_name: pod_name.clone(),
                namespace: state.config.pod_namespace.clone(),
                created_at: now,
                expires_at,
                ssh_port: ssh_port,
                ssh_username: username.clone(),
                ssh_password: password.clone(),
                payment_amount_sats: payment_amount_sats,
                duration_minutes,
                node_port: Some(node_port),
            };

            // Store pod info
            state.active_pods.write().await.insert(pod_name.clone(), pod_info.clone());

            // Schedule pod deletion
            let state_clone = state.clone();
            let pod_name_clone = pod_name.clone();
            tokio::spawn(async move {
                sleep(Duration::from_secs(duration_minutes * 60)).await;
                
                // Remove from active pods
                state_clone.active_pods.write().await.remove(&pod_name_clone);
                
                // Delete the pod
                if let Err(e) = state_clone.k8s_client.delete_pod(&state_clone.config.pod_namespace, &pod_name_clone).await {
                    error!("Failed to cleanup pod {}: {}", pod_name_clone, e);
                } else {
                    info!("Successfully cleaned up expired pod: {}", pod_name_clone);
                }
            });

            (StatusCode::CREATED, Json(SpawnPodResponse {
                success: true,
                message: format!(
                    "Pod created successfully. SSH access available for {} minutes. SSH port: {}, NodePort: {} on any cluster node.",
                    duration_minutes, ssh_port, node_port
                ),
                pod_info: Some(pod_info),
            })).into_response()
        },
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(SpawnPodResponse {
                success: false,
                message: format!("Failed to create pod: {}", e),
                pod_info: None,
            })).into_response()
        }
    }
}


// List all active pods
async fn list_pods(State(state): State<SidecarState>) -> Json<Vec<PodInfo>> {
    let pods = state.active_pods.read().await;
    Json(pods.values().cloned().collect())
}

// Get specific pod info
async fn get_pod_info(
    Path(pod_name): Path<String>,
    State(state): State<SidecarState>,
) -> Result<Json<PodInfo>, StatusCode> {
    let pods = state.active_pods.read().await;

    match pods.get(&pod_name) {
        Some(pod_info) => Ok(Json(pod_info.clone())),
        None => Err(StatusCode::NOT_FOUND),
    }
}

// Get port-forward command for a specific pod
async fn get_port_forward_command(
    Path(pod_name): Path<String>,
    State(state): State<SidecarState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let pods = state.active_pods.read().await;

    match pods.get(&pod_name) {
        Some(pod_info) => {
            let direct_ssh_command = format!(
                "ssh -o PreferredAuthentications=password -o PubkeyAuthentication=no {}@$(minikube ip) -p {}",
                pod_info.ssh_username, pod_info.node_port.unwrap_or(2222)
            );

            let port_forward_command = format!(
                "kubectl -n {} port-forward svc/{}-ssh 2222:2222",
                state.config.pod_namespace,
                pod_name
            );

            let ssh_command = format!(
                "ssh -o PreferredAuthentications=password -o PubkeyAuthentication=no {}@localhost -p 2222",
                pod_info.ssh_username
            );

            let response = serde_json::json!({
                "pod_name": pod_name,
                "ssh_port": 2222,
                "node_port": pod_info.node_port,
                "direct_ssh_command": direct_ssh_command,
                "port_forward_command": port_forward_command,
                "ssh_command": ssh_command,
                "instructions": [
                    "🚀 Direct SSH access (no kubectl needed):".to_string(),
                    direct_ssh_command,
                    format!("Password: {}", pod_info.ssh_password),
                    "".to_string(),
                    "⚠️  Alternative (requires kubectl):".to_string(),
                    format!("Run: {}", port_forward_command),
                    "In another terminal, run: ".to_string() + &ssh_command
                ]
            });

            Ok(Json(response))
        },
        None => Err(StatusCode::NOT_FOUND),
    }
}

// Start the sidecar service
pub async fn start_sidecar_service(bind_addr: &str, config: SidecarConfig) -> Result<(), Box<dyn std::error::Error>> {
    let state = SidecarState::new(config.clone()).await.map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
    
    // Start cleanup task if enabled
    if config.enable_cleanup_task {
        let state_clone = state.clone();
        tokio::spawn(async move {
            loop {
                sleep(Duration::from_secs(60)).await; // Check every minute
                
                let now = Utc::now();
                let mut to_remove = Vec::new();
                
                {
                    let pods = state_clone.active_pods.read().await;
                    for (pod_name, pod_info) in pods.iter() {
                        if now > pod_info.expires_at {
                            to_remove.push((pod_name.clone(), pod_info.namespace.clone()));
                        }
                    }
                }
                
                for (pod_name, namespace) in to_remove {
                    state_clone.active_pods.write().await.remove(&pod_name);
                    
                    if let Err(e) = state_clone.k8s_client.delete_pod(&namespace, &pod_name).await {
                        error!("Failed to cleanup expired pod {}: {}", pod_name, e);
                    } else {
                        info!("Cleaned up expired pod: {}", pod_name);
                    }
                }
            }
        });
    }
    
    let app = create_sidecar_router(state);
    
    println!("🚀 Starting Paygress Sidecar Service");
    println!("📍 Listening on: {}", bind_addr);
    println!("💰 Payment rate: {} sats/hour", config.payment_rate_sats_per_hour);
    println!("⏱️  Default duration: {} minutes", config.default_pod_duration_minutes);
    println!("🔐 SSH port: {}", config.ssh_port);
    println!("📋 Endpoints:");
    println!("   GET  /healthz      - Health check");
    println!("   GET  /auth         - Auth verification for ingress");
    println!("   POST /spawn-pod    - Spawn new SSH pod");
    println!("   GET  /pods         - List active pods");
    println!("   GET  /pods/:name   - Get pod info");
    
    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    axum::serve(listener, app).await?;
    
    Ok(())
}
