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
use std::collections::{HashMap, BTreeMap, HashSet};
use tokio::time::{sleep, Duration};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use nostr_sdk::{Keys, Client, EventBuilder, Kind, Tag, Url};
use nostr_sdk::nips::nip44;
use std::sync::Mutex;

use crate::{cashu, initialize_cashu};

// Configuration for the sidecar service
#[derive(Clone, Debug)]
pub struct SidecarConfig {
    pub cashu_db_path: String,
    pub pod_namespace: String,
    pub payment_rate_sats_per_hour: u64, // Legacy field - now using 1 sat = 1 minute
    pub default_pod_duration_minutes: u64, // Default duration if not specified
    pub ssh_base_image: String, // Base image with SSH server
    pub ssh_host: String, // SSH host for connections
    pub ssh_port_range_start: u16, // Start of port range for pod allocation
    pub ssh_port_range_end: u16, // End of port range for pod allocation
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
            ssh_host: "localhost".to_string(),
            ssh_port_range_start: 30000,
            ssh_port_range_end: 31000,
            enable_cleanup_task: true,
            whitelisted_mints: vec![
                "https://mint.cashu.space".to_string(),
                "https://mint.f7z.io".to_string(),
                "https://legend.lnbits.com/cashu/api/v1".to_string(),
            ],
        }
    }
}

// Port pool for managing SSH port allocation
#[derive(Debug)]
pub struct PortPool {
    available_ports: HashSet<u16>,
    allocated_ports: HashSet<u16>,
    range_start: u16,
    range_end: u16,
}

impl PortPool {
    pub fn new(range_start: u16, range_end: u16) -> Self {
        let mut available_ports = HashSet::new();
        for port in range_start..=range_end {
            available_ports.insert(port);
        }
        
        Self {
            available_ports,
            allocated_ports: HashSet::new(),
            range_start,
            range_end,
        }
    }
    
    pub fn allocate_port(&mut self) -> Option<u16> {
        if let Some(&port) = self.available_ports.iter().next() {
            self.available_ports.remove(&port);
            self.allocated_ports.insert(port);
            Some(port)
        } else {
            None
        }
    }
    
    pub fn deallocate_port(&mut self, port: u16) {
        if self.allocated_ports.remove(&port) {
            self.available_ports.insert(port);
        }
    }
    
    pub fn is_allocated(&self, port: u16) -> bool {
        self.allocated_ports.contains(&port)
    }
    
    pub fn available_count(&self) -> usize {
        self.available_ports.len()
    }
    
    pub fn allocated_count(&self) -> usize {
        self.allocated_ports.len()
    }
}

// Sidecar service state
#[derive(Clone)]
pub struct SidecarState {
    pub config: SidecarConfig,
    pub k8s_client: Arc<PodManager>,
    pub active_pods: Arc<tokio::sync::RwLock<HashMap<String, PodInfo>>>,
    pub port_pool: Arc<Mutex<PortPool>>,
}

// Information about active pods
#[derive(Clone, Debug, Serialize)]
pub struct PodInfo {
    pub pod_name: String,
    pub namespace: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub allocated_port: u16, // Port allocated from the port pool (this is the SSH port)
    pub ssh_username: String,
    pub ssh_password: String,
    pub payment_amount_sats: u64,
    pub duration_minutes: u64,
    pub node_port: Option<u16>,
    pub nostr_public_key: String,  // Pod's npub
    pub nostr_private_key: String, // Pod's nsec
}

// Request to spawn a pod
#[derive(Debug, Deserialize)]
pub struct SpawnPodRequest {
    pub cashu_token: String,
    pub pod_image: Option<String>, // Optional custom image
    pub ssh_username: Option<String>, // Optional custom username
}

// Request to top up/extend a pod
#[derive(Debug, Deserialize)]
pub struct TopUpPodRequest {
    pub pod_name: String,
    pub cashu_token: String,
}

// Response from pod spawning
#[derive(Debug, Serialize)]
pub struct SpawnPodResponse {
    pub success: bool,
    pub message: String,
    pub pod_info: Option<PodInfo>,
}

// Response from pod top-up
#[derive(Debug, Serialize)]
pub struct TopUpPodResponse {
    pub success: bool,
    pub message: String,
    pub pod_info: Option<PodInfo>,
    pub extended_duration_minutes: Option<u64>,
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
        config: &SidecarConfig,
        namespace: &str,
        pod_name: &str,
        image: &str,
        ssh_port: u16,
        username: &str,
        password: &str,
        duration_minutes: u64,
        user_pubkey: &str, // User's public key for sending access events
    ) -> Result<(u16, String, String), String> { // Return (node_port, pod_npub, pod_nsec)
        use k8s_openapi::api::core::v1::{
            Container, Pod, PodSpec, EnvVar, ContainerPort, Service, ServiceSpec, ServicePort,
            Volume,
        };
        use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
        use kube::api::PostParams;
        use kube::Api;

        let pods: Api<Pod> = Api::namespaced(self.client.clone(), namespace);
        let services: Api<Service> = Api::namespaced(self.client.clone(), namespace);
        let _configmaps: Api<k8s_openapi::api::core::v1::ConfigMap> = Api::namespaced(self.client.clone(), namespace);

        // Generate Nostr keys for this pod
        let pod_keys = Keys::generate();
        let pod_npub = pod_keys.public_key().to_hex();
        let pod_nsec = pod_keys.secret_key().unwrap().to_secret_hex();

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
            // Nostr keys for the pod
            EnvVar {
                name: "POD_NPUB".to_string(),
                value: Some(pod_npub.clone()),
                value_from: None,
            },
            EnvVar {
                name: "POD_NSEC".to_string(),
                value: Some(pod_nsec.clone()),
                value_from: None,
            },
            EnvVar {
                name: "USER_PUBKEY".to_string(),
                value: Some(user_pubkey.to_string()),
                value_from: None,
            },
            EnvVar {
                name: "NOSTR_RELAYS".to_string(),
                value: Some("wss://relay.damus.io,wss://nos.lol,wss://relay.nostr.band".to_string()),
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
        // Note: No TTL annotations needed - activeDeadlineSeconds handles pod termination

        // Create volumes
        let _volumes: Vec<Volume> = Vec::new();

        // Create containers with host networking
        let containers = vec![Container {
            name: "ssh-server".to_string(),
            image: Some(image.to_string()),
            ports: Some(vec![ContainerPort {
                container_port: 22, // Always use port 22 internally
                host_port: Some(ssh_port as i32), // Direct port on host
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
                active_deadline_seconds: Some((duration_minutes * 60) as i64), // Kubernetes will auto-terminate after this time
                host_network: Some(true), // Use host networking for direct port access
                ..Default::default()
            }),
            ..Default::default()
        };

        // Create the pod
        let pp = PostParams::default();
        pods.create(&pp, &pod).await.map_err(|e| format!("Failed to create pod: {}", e))?;

        // With host networking, the port is directly accessible on the host
        // No service needed - SSH is accessible at <host-ip>:<allocated-port>
        let node_port = ssh_port; // The allocated port is directly accessible

        info!(
            pod_name = %pod_name, 
            namespace = %namespace, 
            duration_minutes = %duration_minutes,
            ssh_port = %ssh_port,
            username = %username,
            "Created SSH pod with service"
        );

        // Send access event from the pod itself
        let _ = Self::send_pod_access_event(
            &config,
            &pod_npub,
            &pod_nsec,
            user_pubkey,
            &pod_name,
            username,
            password,
            node_port,
            duration_minutes,
        ).await;

        Ok((node_port, pod_npub, pod_nsec))
    }

    // Function to send access event from the pod itself
    async fn send_pod_access_event(
        config: &SidecarConfig,
        _pod_npub: &str,
        pod_nsec: &str,
        user_pubkey: &str,
        pod_name: &str,
        username: &str,
        password: &str,
        node_port: u16,
        duration_minutes: u64,
    ) -> Result<(), String> {
        // Create access details
        let access_details = serde_json::json!({
            "kind": "access_details",
            "pod_name": pod_name,
            "ssh_username": username,
            "ssh_password": password,
            "node_port": node_port,
            "expires_at": (Utc::now() + chrono::Duration::minutes(duration_minutes as i64)).to_rfc3339(),
            "instructions": vec![
                "🚀 SSH access available:".to_string(),
                "".to_string(),
                "Direct access (no kubectl needed):".to_string(),
                format!("   ssh -o PreferredAuthentications=password -o PubkeyAuthentication=no {}@<your-public-ip> -p {}", username, node_port),
                format!("   Password: {}", password),
                "".to_string(),
                "Note: Replace <your-public-ip> with your actual public IP address".to_string(),
                format!("Port {} is directly accessible on your public IP", node_port),
                "".to_string()
            ]
        });

        // Encrypt the access details
        let pod_keys = Keys::parse(pod_nsec).map_err(|e| format!("Invalid pod nsec: {}", e))?;
        let user_pubkey_parsed = nostr_sdk::PublicKey::parse(user_pubkey).map_err(|e| format!("Invalid user pubkey: {}", e))?;
        let encrypted_content = match nip44::encrypt(
            pod_keys.secret_key().map_err(|e| format!("Invalid pod secret key: {}", e))?,
            &user_pubkey_parsed,
            &access_details.to_string(),
            nip44::Version::V2
        ) {
            Ok(content) => content,
            Err(e) => {
                error!("Failed to encrypt access details: {}", e);
                return Err(format!("Encryption failed: {}", e));
            }
        };

        // Send the encrypted event
        let pod_keys = Keys::parse(pod_nsec).map_err(|e| format!("Invalid pod nsec: {}", e))?;
        let client = Client::new(&pod_keys);

        // Connect to relays
        let relays = vec![
            "wss://relay.damus.io",
            "wss://nos.lol",
            "wss://relay.nostr.band",
        ];

        for relay in relays {
            if let Ok(url) = relay.parse::<Url>() {
                let _ = client.add_relay(url).await;
            }
        }

        client.connect().await;

        // Create and send the event
        let tags = vec![
            Tag::hashtag("paygress"),
            Tag::hashtag("access"),
            Tag::hashtag("encrypted"),
        ];

        let builder = EventBuilder::new(Kind::Custom(1001), encrypted_content, tags);
        match builder.to_event(&pod_keys) {
            Ok(event) => {
                let event_id = event.id;
                if let Err(e) = client.send_event(event).await {
                    error!("Failed to send access event: {}", e);
                    return Err(format!("Failed to send event: {}", e));
                }
                info!("Pod {} sent encrypted access event with ID: {}", pod_name, event_id);
            }
            Err(e) => {
                error!("Failed to create access event: {}", e);
                return Err(format!("Failed to create event: {}", e));
            }
        }

        Ok(())
    }

    pub async fn delete_pod(&self, namespace: &str, pod_name: &str) -> Result<(), String> {
        use kube::api::DeleteParams;
        use kube::Api;
        use k8s_openapi::api::core::v1::{Pod, Service, ConfigMap};

        let pods: Api<Pod> = Api::namespaced(self.client.clone(), namespace);
        let services: Api<Service> = Api::namespaced(self.client.clone(), namespace);
        let _configmaps: Api<ConfigMap> = Api::namespaced(self.client.clone(), namespace);

        // Delete the pod
        let dp = DeleteParams::default();
        let _ = pods.delete(pod_name, &dp).await;

        // Delete the associated service
        let service_name = format!("{}-ssh", pod_name);
        let _ = services.delete(&service_name, &dp).await;


        info!(pod_name = %pod_name, namespace = %namespace, "Deleted pod and service");
        Ok(())
    }

    pub async fn extend_pod_deadline(&self, namespace: &str, pod_name: &str, additional_duration_minutes: u64) -> Result<(), String> {
        use kube::api::{Patch, PatchParams};
        use kube::Api;
        use k8s_openapi::api::core::v1::Pod;
        use serde_json::json;

        let pods: Api<Pod> = Api::namespaced(self.client.clone(), namespace);

        // Get current pod to read existing activeDeadlineSeconds
        let current_pod = pods.get(pod_name).await.map_err(|e| format!("Failed to get pod: {}", e))?;
        
        // Calculate new deadline
        let current_deadline_seconds = current_pod
            .spec
            .as_ref()
            .and_then(|spec| spec.active_deadline_seconds)
            .unwrap_or(0);
        
        let new_deadline_seconds = current_deadline_seconds + (additional_duration_minutes * 60) as i64;
        let new_expires_at = Utc::now() + chrono::Duration::minutes(additional_duration_minutes as i64);

        // Create patch to update activeDeadlineSeconds and annotations
        let patch = json!({
            "spec": {
                "activeDeadlineSeconds": new_deadline_seconds
            },
            "metadata": {
                "annotations": {
                    "paygress.io/expires-at": new_expires_at.to_rfc3339(),
                    "paygress.io/extended-at": Utc::now().to_rfc3339()
                }
            }
        });

        let pp = PatchParams::default();
        let _ = pods.patch(pod_name, &pp, &Patch::Merge(patch)).await
            .map_err(|e| format!("Failed to update pod deadline: {}", e))?;

        info!(
            pod_name = %pod_name, 
            namespace = %namespace,
            additional_minutes = %additional_duration_minutes,
            new_deadline_seconds = %new_deadline_seconds,
            "Extended pod activeDeadlineSeconds"
        );

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
        
        // Initialize port pool
        let port_pool = Arc::new(Mutex::new(PortPool::new(
            config.ssh_port_range_start,
            config.ssh_port_range_end,
        )));

        Ok(Self {
            config,
            k8s_client,
            active_pods,
            port_pool,
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

    // Validate port is actually available before allocation
    pub fn validate_port_available(&self, port: u16) -> Result<(), String> {
        use std::net::{TcpListener, SocketAddr};
        
        // Try to bind to the port to check if it's available
        let addr = SocketAddr::from(([0, 0, 0, 0], port));
        match TcpListener::bind(addr) {
            Ok(_) => Ok(()), // Port is available
            Err(_) => Err(format!("Port {} is already in use", port)),
        }
    }

    // Generate unique SSH port for each pod from the port pool with validation
    pub fn generate_ssh_port(&self) -> Result<u16, String> {
        let mut port_pool = self.port_pool.lock().map_err(|e| format!("Failed to lock port pool: {}", e))?;
        
        // Try to find an available port that's actually free
        let available_ports: Vec<u16> = port_pool.available_ports.iter().cloned().collect();
        
        for port in available_ports {
            // Validate port is actually available on the system
            if self.validate_port_available(port).is_ok() {
                // Port is free, allocate it
                port_pool.available_ports.remove(&port);
                port_pool.allocated_ports.insert(port);
                
                info!("Allocated port {} from pool ({} available, {} allocated)", 
                      port, port_pool.available_count(), port_pool.allocated_count());
                return Ok(port);
            } else {
                // Port is in use, remove from available pool and add to allocated
                port_pool.available_ports.remove(&port);
                port_pool.allocated_ports.insert(port);
                warn!("Port {} was marked available but is actually in use - removing from pool", port);
            }
        }
        
        Err("No available ports in the configured range".to_string())
    }
    
    // Deallocate a port back to the pool
    pub fn deallocate_ssh_port(&self, port: u16) -> Result<(), String> {
        let mut port_pool = self.port_pool.lock().map_err(|e| format!("Failed to lock port pool: {}", e))?;
        port_pool.deallocate_port(port);
        info!("Deallocated port {} back to pool ({} available, {} allocated)", 
              port, port_pool.available_count(), port_pool.allocated_count());
        Ok(())
    }
    
    // Clean up ports for expired pods
    pub async fn cleanup_expired_pods(&self) -> Result<(), String> {
        let mut pods = self.active_pods.write().await;
        let now = Utc::now();
        let mut expired_pods = Vec::new();
        
        // Find expired pods
        for (pod_name, pod_info) in pods.iter() {
            if now > pod_info.expires_at {
                expired_pods.push((pod_name.clone(), pod_info.allocated_port));
            }
        }
        
        // Remove expired pods and deallocate their ports
        for (pod_name, allocated_port) in expired_pods {
            pods.remove(&pod_name);
            if let Err(e) = self.deallocate_ssh_port(allocated_port) {
                error!("Failed to deallocate port {} for expired pod {}: {}", allocated_port, pod_name, e);
            } else {
                info!("Cleaned up expired pod {} and deallocated port {}", pod_name, allocated_port);
            }
        }
        
        Ok(())
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
    let allocated_port = state.generate_ssh_port()?; // Generate unique port for this pod

    let now = Utc::now();
    let expires_at = now + chrono::Duration::minutes(duration_minutes as i64);

    // Create the pod
    match state.k8s_client.create_ssh_pod(
        &state.config,
        &state.config.pod_namespace,
        &pod_name,
        &image,
        allocated_port,
        &username,
        &password,
        duration_minutes,
        "http-mode-user", // Dummy user pubkey for HTTP mode
    ).await {
        Ok((node_port, pod_npub, pod_nsec)) => {
            let pod_info = PodInfo {
                pod_name: pod_name.clone(),
                namespace: state.config.pod_namespace.clone(),
                created_at: now,
                expires_at,
                allocated_port,
                ssh_username: username.clone(),
                ssh_password: password.clone(),
                payment_amount_sats: payment_amount_sats,
                duration_minutes,
                node_port: Some(node_port),
                nostr_public_key: pod_npub,
                nostr_private_key: pod_nsec,
            };

            // Store pod info
            state.active_pods.write().await.insert(pod_name.clone(), pod_info.clone());

            // Pod will be automatically deleted by Kubernetes based on TTL annotation
            // No need for manual scheduling - Kubernetes handles this natively

            (StatusCode::CREATED, Json(SpawnPodResponse {
                success: true,
                message: format!(
                    "Pod created successfully. SSH access available for {} minutes. External port: {}",
                    duration_minutes, node_port
                ),
                pod_info: Some(pod_info),
            })).into_response()
        },
        Err(e) => {
            // Deallocate the port if pod creation failed
            if let Err(dealloc_err) = state.deallocate_ssh_port(allocated_port) {
                error!("Failed to deallocate port {} after pod creation failure: {}", allocated_port, dealloc_err);
            }
            (StatusCode::INTERNAL_SERVER_ERROR, Json(SpawnPodResponse {
                success: false,
                message: format!("Failed to create pod: {}", e),
                pod_info: None,
            })).into_response()
        }
    }
}

// Top up/extend a pod's duration - handler function
async fn top_up_pod_handler(
    state: SidecarState,
    request: TopUpPodRequest,
) -> Response {
    info!("Pod top-up request received for pod: {}", request.pod_name);

    // Check if pod exists
    let mut pods = state.active_pods.write().await;
    let pod_info = match pods.get_mut(&request.pod_name) {
        Some(pod) => pod,
        None => {
            return (StatusCode::NOT_FOUND, Json(TopUpPodResponse {
                success: false,
                message: format!("Pod '{}' not found or already expired", request.pod_name),
                pod_info: None,
                extended_duration_minutes: None,
            })).into_response();
        }
    };

    // Check if pod has already expired
    let now = Utc::now();
    if now > pod_info.expires_at {
        // Remove expired pod from active pods and deallocate its port
        let allocated_port = pod_info.allocated_port;
        pods.remove(&request.pod_name);
        if let Err(e) = state.deallocate_ssh_port(allocated_port) {
            error!("Failed to deallocate port {} for expired pod {}: {}", allocated_port, request.pod_name, e);
        }
        return (StatusCode::GONE, Json(TopUpPodResponse {
            success: false,
            message: format!("Pod '{}' has already expired and cannot be extended", request.pod_name),
            pod_info: None,
            extended_duration_minutes: None,
        })).into_response();
    }

    // Extract payment amount from token
    let payment_amount_sats = match extract_token_value(&request.cashu_token).await {
        Ok(sats) => sats,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, Json(TopUpPodResponse {
                success: false,
                message: format!("Failed to decode Cashu token: {}", e),
                pod_info: None,
                extended_duration_minutes: None,
            })).into_response();
        }
    };

    // Calculate additional duration from payment
    let additional_duration_minutes = state.calculate_duration_from_payment(payment_amount_sats);
    
    if additional_duration_minutes == 0 {
        return (StatusCode::PAYMENT_REQUIRED, Json(TopUpPodResponse {
            success: false,
            message: "Insufficient payment. Minimum required: 1 sat for 1 minute extension".to_string(),
            pod_info: None,
            extended_duration_minutes: None,
        })).into_response();
    }

    info!("💰 Top-up payment: {} sats → ⏱️ Additional duration: {} minutes", payment_amount_sats, additional_duration_minutes);

    // Verify payment token validity
    match cashu::verify_cashu_token(&request.cashu_token, 1, &state.config.whitelisted_mints).await {
        Ok(false) => {
            return (StatusCode::PAYMENT_REQUIRED, Json(TopUpPodResponse {
                success: false,
                message: "Cashu token verification failed - invalid token".to_string(),
                pod_info: None,
                extended_duration_minutes: None,
            })).into_response();
        },
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(TopUpPodResponse {
                success: false,
                message: format!("Payment verification error: {}", e),
                pod_info: None,
                extended_duration_minutes: None,
            })).into_response();
        },
        Ok(true) => {
            info!("✅ Top-up payment verified: {} sats for {} additional minutes", payment_amount_sats, additional_duration_minutes);
        }
    }

    // Extend the pod's expiration time in memory
    let old_expires_at = pod_info.expires_at;
    pod_info.expires_at = pod_info.expires_at + chrono::Duration::minutes(additional_duration_minutes as i64);
    pod_info.payment_amount_sats += payment_amount_sats;
    pod_info.duration_minutes += additional_duration_minutes;

    let updated_pod_info = pod_info.clone();
    drop(pods); // Release the write lock

    // Update the pod's activeDeadlineSeconds in Kubernetes
    if let Err(e) = state.k8s_client.extend_pod_deadline(&state.config.pod_namespace, &request.pod_name, additional_duration_minutes).await {
        error!("Failed to extend pod deadline in Kubernetes: {}", e);
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(TopUpPodResponse {
            success: false,
            message: format!("Failed to extend pod deadline: {}", e),
            pod_info: None,
            extended_duration_minutes: None,
        })).into_response();
    }

    info!(
        "🔄 Pod '{}' extended: {} → {} (added {} minutes)",
        request.pod_name,
        old_expires_at.format("%H:%M:%S UTC"),
        updated_pod_info.expires_at.format("%H:%M:%S UTC"),
        additional_duration_minutes
    );

    (StatusCode::OK, Json(TopUpPodResponse {
        success: true,
        message: format!(
            "Pod '{}' successfully extended by {} minutes. New expiration: {}",
            request.pod_name,
            additional_duration_minutes,
            updated_pod_info.expires_at.format("%Y-%m-%d %H:%M:%S UTC")
        ),
        pod_info: Some(updated_pod_info),
        extended_duration_minutes: Some(additional_duration_minutes),
    })).into_response()
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
                "ssh -o PreferredAuthentications=password -o PubkeyAuthentication=no {}@<your-public-ip> -p {}",
                pod_info.ssh_username, pod_info.node_port.unwrap_or(pod_info.allocated_port)
            );

            let ssh_command = format!(
                "ssh -o PreferredAuthentications=password -o PubkeyAuthentication=no {}@<your-public-ip> -p {}",
                pod_info.ssh_username, pod_info.node_port.unwrap_or(pod_info.allocated_port)
            );

            let response = serde_json::json!({
                "pod_name": pod_name,
                "allocated_port": pod_info.allocated_port,
                "node_port": pod_info.node_port,
                "direct_ssh_command": direct_ssh_command,
                "ssh_command": ssh_command,
                "instructions": [
                    "🚀 Direct SSH access (no kubectl needed):".to_string(),
                    direct_ssh_command,
                    format!("Password: {}", pod_info.ssh_password),
                    "".to_string(),
                    "Note: Replace <your-public-ip> with your actual public IP address".to_string(),
                    format!("Port {} is directly accessible on your public IP", pod_info.node_port.unwrap_or(pod_info.allocated_port))
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
    
    // Pod cleanup is now handled by Kubernetes TTL annotations
    // No need for polling-based cleanup loops
    
    let app = create_sidecar_router(state);
    
    println!("🚀 Starting Paygress Sidecar Service");
    println!("📍 Listening on: {}", bind_addr);
    println!("💰 Payment rate: {} sats/hour", config.payment_rate_sats_per_hour);
    println!("⏱️  Default duration: {} minutes", config.default_pod_duration_minutes);
    println!("🔐 SSH port range: {}-{}", config.ssh_port_range_start, config.ssh_port_range_end);
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
