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
use chrono::{DateTime, Utc};
use nostr_sdk::{Keys, Client, Url, EventBuilder, Kind, Tag};
use kube::{Api, api::ListParams};
use k8s_openapi::api::core::v1::Pod;
// Using NIP-17 private direct messages - no manual encryption needed
use std::sync::Mutex;

use crate::{cashu, initialize_cashu};

// Configuration for the sidecar service
#[derive(Clone, Debug)]
pub struct SidecarConfig {
    pub cashu_db_path: String,
    pub pod_namespace: String,
    pub payment_rate_msats_per_sec: u64, // Payment rate in msats per second
    pub minimum_pod_duration_seconds: u64, // Minimum pod duration in seconds
    pub base_image: String, // Base image for pods
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
            payment_rate_msats_per_sec: 100, // 100 msats per second
            minimum_pod_duration_seconds: 60, // 1 minute minimum
            base_image: "linuxserver/openssh-server:latest".to_string(),
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
    pub pod_npub: String,    // Use NPUB instead of pod_name
    pub namespace: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub allocated_port: u16, // Port allocated from the port pool (this is the SSH port)
    pub ssh_username: String,
    pub ssh_password: String,
    pub payment_amount_msats: u64,
    pub duration_seconds: u64,
    pub node_port: Option<u16>,
    pub nostr_public_key: String,  // Pod's npub
    pub nostr_private_key: String, // Pod's nsec
}

// Request to spawn a pod
#[derive(Debug, Deserialize)]
pub struct SpawnPodRequest {
    pub cashu_token: String,
    pub pod_image: Option<String>, // Optional: Uses base image if not specified
    pub ssh_username: Option<String>, // Optional custom username
    pub ssh_password: Option<String>, // Optional custom password
}

// Request to top up/extend a pod
#[derive(Debug, Deserialize)]
pub struct TopUpPodRequest {
    pub pod_npub: String,    // Changed from pod_name to pod_npub
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
    pub extended_duration_seconds: Option<u64>,
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
        pod_npub: &str,        // Add NPUB parameter
        pod_nsec: &str,        // Add NSEC parameter
        image: &str,
        ssh_port: u16,
        username: &str,
        password: &str,
        duration_seconds: u64,
        memory_mb: u64,
        cpu_millicores: u64,
        user_pubkey: &str, // User's public key for sending access events
    ) -> Result<u16, String> { // Return only node_port since we have NPUB
        use k8s_openapi::api::core::v1::{
            Container, Pod, PodSpec, EnvVar, ContainerPort, Service, Volume,
        };
        use kube::api::PostParams;
        use kube::Api;

        let pods: Api<Pod> = Api::namespaced(self.client.clone(), namespace);
        let _services: Api<Service> = Api::namespaced(self.client.clone(), namespace);
        let _configmaps: Api<k8s_openapi::api::core::v1::ConfigMap> = Api::namespaced(self.client.clone(), namespace);

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
                value: Some(pod_npub.to_string()),
                value_from: None,
            },
            EnvVar {
                name: "POD_NSEC".to_string(),
                value: Some(pod_nsec.to_string()),
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
            EnvVar {
                name: "SSH_PORT".to_string(),
                value: Some(ssh_port.to_string()),
                value_from: None,
            },
        ];

        // Create pod labels and annotations
        let mut labels = BTreeMap::new();
        labels.insert("app".to_string(), "paygress-ssh-pod".to_string());
        labels.insert("managed-by".to_string(), "paygress-sidecar".to_string());
        labels.insert("pod-type".to_string(), "ssh-access".to_string());
        labels.insert("pod-name".to_string(), pod_name.to_string());
        // Extract hex part from NPUB and truncate to fit Kubernetes label limit (63 chars)
        let npub_hex = if pod_npub.starts_with("npub1") {
            &pod_npub[5..] // Remove "npub1" prefix
        } else {
            pod_npub // Already hex or different format
        };
        // Truncate to 63 characters max for Kubernetes labels
        let truncated_hex = if npub_hex.len() > 63 {
            &npub_hex[..63]
        } else {
            npub_hex
        };
        labels.insert("pod-npub".to_string(), truncated_hex.to_string()); // Add NPUB hex as label

        let mut annotations = BTreeMap::new();
        annotations.insert("paygress.io/created-at".to_string(), Utc::now().to_rfc3339());
        annotations.insert("paygress.io/expires-at".to_string(), 
            (Utc::now() + chrono::Duration::seconds(duration_seconds as i64)).to_rfc3339());
        annotations.insert("paygress.io/duration-seconds".to_string(), duration_seconds.to_string());
        annotations.insert("paygress.io/ssh-username".to_string(), username.to_string());
        // Note: No TTL annotations needed - activeDeadlineSeconds handles pod termination

        // Create volumes
        let _volumes: Vec<Volume> = Vec::new();

        // Create containers with host networking
        let containers = vec![Container {
            name: "ssh-server".to_string(),
            image: Some(image.to_string()),
            ports: Some(vec![ContainerPort {
                container_port: ssh_port as i32, // SSH port inside container matches allocated port
                host_port: Some(ssh_port as i32), // External port matches container port (required for host networking)
                name: Some("ssh".to_string()),
                protocol: Some("TCP".to_string()),
                ..Default::default()
            }]),
            env: Some(env_vars),
            image_pull_policy: Some("IfNotPresent".to_string()),
            resources: Some(k8s_openapi::api::core::v1::ResourceRequirements {
                limits: Some({
                    let mut limits = std::collections::BTreeMap::new();
                    limits.insert("memory".to_string(), k8s_openapi::apimachinery::pkg::api::resource::Quantity(format!("{}Mi", memory_mb)));
                    limits.insert("cpu".to_string(), k8s_openapi::apimachinery::pkg::api::resource::Quantity(format!("{}m", cpu_millicores)));
                    limits
                }),
                requests: Some({
                    let mut requests = std::collections::BTreeMap::new();
                    requests.insert("memory".to_string(), k8s_openapi::apimachinery::pkg::api::resource::Quantity(format!("{}Mi", memory_mb)));
                    requests.insert("cpu".to_string(), k8s_openapi::apimachinery::pkg::api::resource::Quantity(format!("{}m", cpu_millicores)));
                    requests
                }),
                claims: None,
            }),
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
                active_deadline_seconds: Some(duration_seconds as i64), // Kubernetes will auto-terminate after this time
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
            duration_seconds = %duration_seconds,
            ssh_port = %ssh_port,
            username = %username,
            "Created SSH pod with service"
        );

        // Send access event from the service (not the pod) for security
        let _ = Self::send_pod_access_event_from_service(
            &pod_npub,
            user_pubkey,
            &pod_name,
            username,
            password,
            node_port,
            duration_seconds,
            &config.ssh_host,
        ).await;

        Ok(node_port)
    }

    // Function to send access event from the service (secure)
    async fn send_pod_access_event_from_service(
        pod_npub: &str,
        user_pubkey: &str,
        _pod_name: &str,
        username: &str,
        password: &str,
        node_port: u16,
        duration_seconds: u64,
        ssh_host: &str,
    ) -> Result<(), String> {
        use nostr_sdk::prelude::*;
        use std::env;
        
        // Create access details
        let access_details = serde_json::json!({
            "kind": "access_details",
            "pod_npub": pod_npub,        // Use NPUB instead of pod_name
            "ssh_username": username,
            "ssh_password": password,
            "node_port": node_port,
            "expires_at": (Utc::now() + chrono::Duration::seconds(duration_seconds as i64)).to_rfc3339(),
            "instructions": vec![
                "🚀 SSH access available:".to_string(),
                "".to_string(),
                "Direct access (no kubectl needed):".to_string(),
                format!("   ssh -o PreferredAuthentications=password -o PubkeyAuthentication=no {}@{} -p {}", username, ssh_host, node_port),
                "".to_string(),
                "⚠️  Pod expires at:".to_string(),
                format!("   {}", (Utc::now() + chrono::Duration::seconds(duration_seconds as i64)).format("%Y-%m-%d %H:%M:%S UTC")),
                "".to_string(),
                "📋 Pod Details:".to_string(),
                format!("   Pod NPUB: {}", pod_npub),
                format!("   Duration: {} seconds", duration_seconds),
            ]
        });

        // Get service keys from environment
        let service_nsec = env::var("NOSTR_PRIVATE_KEY")
            .map_err(|_| "NOSTR_PRIVATE_KEY not found in environment")?;
        let service_keys = Keys::parse(&service_nsec)
            .map_err(|e| format!("Invalid service nsec: {}", e))?;
        
        let client = Client::new(&service_keys);
        
        // Add relays
        let relay_urls = env::var("NOSTR_RELAYS")
            .unwrap_or_else(|_| "wss://relay.damus.io,wss://nos.lol,wss://relay.nostr.band".to_string());
        for relay_url in relay_urls.split(',') {
            let relay = relay_url.trim();
            if !relay.is_empty() {
                let _ = client.add_relay(relay).await;
            }
        }
        
        client.connect().await;
        
        // Parse user public key
        let user_pubkey_parsed = nostr_sdk::PublicKey::parse(user_pubkey)
            .map_err(|e| format!("Invalid user pubkey: {}", e))?;
        
        // Send as kind 14 event
        let tags = vec![
            Tag::hashtag("paygress"),
            Tag::hashtag("access_details"),
        ];
        
        let builder = EventBuilder::new(Kind::Custom(14), access_details.to_string(), tags);
        let event = builder.to_event(&service_keys)
            .map_err(|e| format!("Failed to create event: {}", e))?;
        
        match client.send_event(event).await {
            Ok(event_id) => {
                info!("Service sent access details via kind 14 event with ID: {:?}", event_id);
            }
            Err(e) => {
                error!("Failed to send kind 14 event from service: {}", e);
                return Err(format!("Failed to send kind 14 event: {}", e));
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

    pub async fn extend_pod_deadline(&self, namespace: &str, pod_name: &str, additional_duration_seconds: u64) -> Result<(), String> {
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
        
        let new_deadline_seconds = current_deadline_seconds + additional_duration_seconds as i64;
        let new_expires_at = Utc::now() + chrono::Duration::seconds(additional_duration_seconds as i64);

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
            additional_seconds = %additional_duration_seconds,
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
    pub fn calculate_duration_from_payment(&self, payment_msats: u64) -> u64 {
        let msats_per_sec = self.config.payment_rate_msats_per_sec.max(1);
        
        // Calculate duration in seconds: payment_msats / msats_per_sec
        payment_msats / msats_per_sec
    }

    // Check if payment is sufficient for minimum duration
    pub fn is_payment_sufficient(&self, payment_msats: u64) -> bool {
        let calculated_duration = self.calculate_duration_from_payment(payment_msats);
        calculated_duration >= self.config.minimum_pod_duration_seconds
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
    let total_amount_msats: u64 = match token_decoded.unit() {
        Some(unit) => match unit {
            cdk::nuts::CurrencyUnit::Sat => u64::from(total_amount) * 1000, // Convert sat to msat
            cdk::nuts::CurrencyUnit::Msat => u64::from(total_amount), // Already in msat
            _ => return Err(format!("Unsupported token unit: {:?}", unit)),
        },
        None => return Err("Token has no unit specified".to_string()),
    };
    
    Ok(total_amount_msats)
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
    let payment_amount_msats = match extract_token_value(&token).await {
        Ok(msats) => msats,
        Err(e) => {
            warn!("Failed to decode token: {}", e);
            return StatusCode::BAD_REQUEST.into_response();
        }
    };

    // Check if payment is sufficient for minimum duration
    if !state.is_payment_sufficient(payment_amount_msats) {
        warn!("❌ Insufficient payment: {} msats (minimum required: {} msats for {} seconds)", 
            payment_amount_msats, 
            state.config.minimum_pod_duration_seconds * state.config.payment_rate_msats_per_sec,
            state.config.minimum_pod_duration_seconds);
        return StatusCode::PAYMENT_REQUIRED.into_response();
    }

    // Calculate duration based on payment
    let duration_seconds = state.calculate_duration_from_payment(payment_amount_msats);

    // Verify Cashu token validity (not amount, just validity)
    match cashu::verify_cashu_token(&token, 1, &state.config.whitelisted_mints).await {
        Ok(true) => {
            info!("✅ Payment verified: {} msats → {} seconds", payment_amount_msats, duration_seconds);
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
    let payment_amount_msats = match extract_token_value(&request.cashu_token).await {
        Ok(msats) => msats,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, Json(SpawnPodResponse {
                success: false,
                message: format!("Failed to decode Cashu token: {}", e),
                pod_info: None,
            })).into_response();
        }
    };

    // Check if payment is sufficient for minimum duration
    if !state.is_payment_sufficient(payment_amount_msats) {
        let minimum_required = state.config.minimum_pod_duration_seconds * state.config.payment_rate_msats_per_sec;
        return (StatusCode::PAYMENT_REQUIRED, Json(SpawnPodResponse {
            success: false,
            message: format!("Insufficient payment: {} msats. Minimum required: {} msats for {} seconds", 
                payment_amount_msats, minimum_required, state.config.minimum_pod_duration_seconds),
            pod_info: None,
        })).into_response();
    }

    // Calculate duration based on payment amount
    let duration_seconds = state.calculate_duration_from_payment(payment_amount_msats);

    info!("💰 Payment: {} msats → ⏱️ Duration: {} seconds", payment_amount_msats, duration_seconds);

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
            info!("✅ Payment verified: {} msats for {} seconds", payment_amount_msats, duration_seconds);
        }
    }

    // Generate NPUB first and use it as pod name
    let pod_keys = nostr_sdk::Keys::generate();
    let pod_npub = pod_keys.public_key().to_hex();
    let pod_nsec = pod_keys.secret_key().unwrap().to_secret_hex();
    
    // Create Kubernetes-safe pod name from NPUB
    let pod_name = format!("pod-{}", pod_npub.replace("npub1", "").chars().take(8).collect::<String>());
    let username = request.ssh_username.unwrap_or_else(|| format!("user-{}", &pod_name[4..12]));
    let password = request.ssh_password.unwrap_or_else(|| SidecarState::generate_password());
    let image = request.pod_image.unwrap_or_else(|| state.config.base_image.clone());
    let allocated_port = match state.generate_ssh_port() {
        Ok(port) => port,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(SpawnPodResponse {
                success: false,
                message: format!("Failed to allocate port: {}", e),
                pod_info: None,
            })).into_response();
        }
    };

    let now = Utc::now();
    let expires_at = now + chrono::Duration::seconds(duration_seconds as i64);

    // Create the pod
    match state.k8s_client.create_ssh_pod(
        &state.config,
        &state.config.pod_namespace,
        &pod_name,
        &pod_npub,
        &pod_nsec,
        &image,
        allocated_port,
        &username,
        &password,
        duration_seconds,
        1024, // 1GB memory
        1000, // 1 CPU core
        "http-mode-user", // Dummy user pubkey for HTTP mode
    ).await {
        Ok(node_port) => {
            let pod_info = PodInfo {
                pod_npub: pod_npub.clone(),
                namespace: state.config.pod_namespace.clone(),
                created_at: now,
                expires_at,
                allocated_port,
                ssh_username: username.clone(),
                ssh_password: password.clone(),
                payment_amount_msats: payment_amount_msats,
                duration_seconds,
                node_port: Some(node_port),
                nostr_public_key: pod_npub.clone(),
                nostr_private_key: pod_nsec,
            };

            // Store pod info
            state.active_pods.write().await.insert(pod_npub.clone(), pod_info.clone());

            // Pod will be automatically deleted by Kubernetes based on TTL annotation
            // No need for manual scheduling - Kubernetes handles this natively

            (StatusCode::CREATED, Json(SpawnPodResponse {
                success: true,
                message: format!(
                    "Pod created successfully. SSH access available for {} seconds. External port: {}",
                    duration_seconds, node_port
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
pub async fn top_up_pod_handler(
    state: SidecarState,
    request: TopUpPodRequest,
) -> Response {
    info!("Pod top-up request received for NPUB: {}", request.pod_npub);

    // Find pod by NPUB label in Kubernetes
    let pods_api: Api<Pod> = Api::namespaced(state.k8s_client.client.clone(), &state.config.pod_namespace);
    let pods = match pods_api.list(&ListParams::default()).await {
        Ok(pods) => pods,
        Err(e) => {
            error!("Failed to list pods: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(TopUpPodResponse {
                success: false,
                message: format!("Failed to list pods: {}", e),
                pod_info: None,
                extended_duration_seconds: None,
            })).into_response();
        }
    };

    // Find pod by NPUB label (compare truncated hex parts)
    let target_pod = match pods.items.iter().find(|pod| {
        pod.metadata.labels.as_ref()
            .and_then(|labels| labels.get("pod-npub"))
            .map(|stored_hex| {
                // Extract hex from the requested NPUB
                let requested_hex = if request.pod_npub.starts_with("npub1") {
                    &request.pod_npub[5..] // Remove "npub1" prefix
                } else {
                    &request.pod_npub // Already hex or different format
                };
                // Truncate both to 63 chars for comparison
                let stored_truncated = if stored_hex.len() > 63 {
                    &stored_hex[..63]
                } else {
                    stored_hex
                };
                let requested_truncated = if requested_hex.len() > 63 {
                    &requested_hex[..63]
                } else {
                    requested_hex
                };
                stored_truncated == requested_truncated
            })
            .unwrap_or(false)
    }) {
        Some(pod) => pod,
        None => {
            return (StatusCode::NOT_FOUND, Json(TopUpPodResponse {
                success: false,
                message: format!("Pod with NPUB '{}' not found or already expired", request.pod_npub),
                pod_info: None,
                extended_duration_seconds: None,
            })).into_response();
        }
    };

    // Get pod name from metadata
    let pod_name = match &target_pod.metadata.name {
        Some(name) => name.clone(),
        None => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(TopUpPodResponse {
                success: false,
                message: "Pod has no name in metadata".to_string(),
                pod_info: None,
                extended_duration_seconds: None,
            })).into_response();
        }
    };

    // Check if pod has already expired by looking at activeDeadlineSeconds
    let now = Utc::now();
    let pod_expires_at = match &target_pod.spec {
        Some(spec) => match spec.active_deadline_seconds {
            Some(deadline_secs) => {
                // Calculate expiration time from pod creation time
                match &target_pod.metadata.creation_timestamp {
                    Some(creation_time) => {
                        let creation_utc = creation_time.0;
                        creation_utc + chrono::Duration::seconds(deadline_secs)
                    }
                    None => now + chrono::Duration::seconds(deadline_secs), // Fallback
                }
            }
            None => {
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(TopUpPodResponse {
                    success: false,
                    message: "Pod has no active deadline set".to_string(),
                    pod_info: None,
                    extended_duration_seconds: None,
                })).into_response();
            }
        }
        None => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(TopUpPodResponse {
                success: false,
                message: "Pod has no spec".to_string(),
                pod_info: None,
                extended_duration_seconds: None,
            })).into_response();
        }
    };

    if now > pod_expires_at {
        // Pod has expired, deallocate its port if we can find it
        let allocated_port = match target_pod.spec.as_ref()
            .and_then(|spec| spec.containers.first())
            .and_then(|container| container.ports.as_ref())
            .and_then(|ports| ports.first())
            .and_then(|port| port.host_port) {
            Some(port) => port as u16,
            None => {
                warn!("Could not determine port for expired pod {}", pod_name);
                0
            }
        };
        
        if allocated_port > 0 {
            state.port_pool.lock().unwrap().deallocate_port(allocated_port);
        }
        if let Err(e) = state.deallocate_ssh_port(allocated_port) {
            error!("Failed to deallocate port {} for expired pod {}: {}", allocated_port, pod_name, e);
        }
        return (StatusCode::GONE, Json(TopUpPodResponse {
            success: false,
            message: format!("Pod '{}' (NPUB: {}) has already expired and cannot be extended", pod_name, request.pod_npub),
            pod_info: None,
            extended_duration_seconds: None,
        })).into_response();
    }

    // Extract payment amount from token
    let payment_amount_msats = match extract_token_value(&request.cashu_token).await {
        Ok(msats) => msats,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, Json(TopUpPodResponse {
                success: false,
                message: format!("Failed to decode Cashu token: {}", e),
                pod_info: None,
                extended_duration_seconds: None,
            })).into_response();
        }
    };

    // Calculate additional duration from payment
    let additional_duration_seconds = state.calculate_duration_from_payment(payment_amount_msats);
    
    if additional_duration_seconds == 0 {
        return (StatusCode::PAYMENT_REQUIRED, Json(TopUpPodResponse {
            success: false,
            message: format!("Insufficient payment: {} msats. Minimum required: {} msats for 1 second extension", 
                payment_amount_msats, state.config.payment_rate_msats_per_sec),
            pod_info: None,
            extended_duration_seconds: None,
        })).into_response();
    }

    info!("💰 Top-up payment: {} msats → ⏱️ Additional duration: {} seconds", payment_amount_msats, additional_duration_seconds);

    // Verify payment token validity
    match cashu::verify_cashu_token(&request.cashu_token, 1, &state.config.whitelisted_mints).await {
        Ok(false) => {
            return (StatusCode::PAYMENT_REQUIRED, Json(TopUpPodResponse {
                success: false,
                message: "Cashu token verification failed - invalid token".to_string(),
                pod_info: None,
                extended_duration_seconds: None,
            })).into_response();
        },
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(TopUpPodResponse {
                success: false,
                message: format!("Payment verification error: {}", e),
                pod_info: None,
                extended_duration_seconds: None,
            })).into_response();
        },
        Ok(true) => {
            info!("✅ Top-up payment verified: {} msats for {} additional seconds", payment_amount_msats, additional_duration_seconds);
        }
    }

    // Update the pod's activeDeadlineSeconds in Kubernetes
    if let Err(e) = state.k8s_client.extend_pod_deadline(&state.config.pod_namespace, &pod_name, additional_duration_seconds).await {
        error!("Failed to extend pod deadline in Kubernetes: {}", e);
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(TopUpPodResponse {
            success: false,
            message: format!("Failed to extend pod deadline: {}", e),
            pod_info: None,
            extended_duration_seconds: None,
        })).into_response();
    }

    // Calculate new expiration time
    let new_expires_at = pod_expires_at + chrono::Duration::seconds(additional_duration_seconds as i64);
    
    info!(
        "🔄 Pod '{}' (NPUB: {}) extended: {} → {} (added {} seconds)",
        pod_name,
        request.pod_npub,
        pod_expires_at.format("%H:%M:%S UTC"),
        new_expires_at.format("%H:%M:%S UTC"),
        additional_duration_seconds
    );

    // Calculate new expiration time
    let new_expires_at = pod_expires_at + chrono::Duration::seconds(additional_duration_seconds as i64);
    
    (StatusCode::OK, Json(TopUpPodResponse {
        success: true,
        message: format!(
            "Pod '{}' (NPUB: {}) successfully extended by {} seconds. New expiration: {}",
            pod_name,
            request.pod_npub,
            additional_duration_seconds,
            new_expires_at.format("%Y-%m-%d %H:%M:%S UTC")
        ),
        pod_info: None, // We don't maintain in-memory state anymore
        extended_duration_seconds: Some(additional_duration_seconds),
    })).into_response()
}

// List all active pods
async fn list_pods(State(state): State<SidecarState>) -> Json<Vec<PodInfo>> {
    let pods = state.active_pods.read().await;
    Json(pods.values().cloned().collect())
}

// Get specific pod info
async fn get_pod_info(
    Path(pod_npub): Path<String>,
    State(state): State<SidecarState>,
) -> Result<Json<PodInfo>, StatusCode> {
    let pods = state.active_pods.read().await;

    match pods.get(&pod_npub) {
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
                "ssh -o PreferredAuthentications=password -o PubkeyAuthentication=no {}@{} -p {}",
                pod_info.ssh_username, state.config.ssh_host, pod_info.node_port.unwrap_or(pod_info.allocated_port)
            );

            let ssh_command = format!(
                "ssh -o PreferredAuthentications=password -o PubkeyAuthentication=no {}@{} -p {}",
                pod_info.ssh_username, state.config.ssh_host, pod_info.node_port.unwrap_or(pod_info.allocated_port)
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
                    format!("Port {} is directly accessible on {}", pod_info.node_port.unwrap_or(pod_info.allocated_port), state.config.ssh_host)
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
    println!("💰 Payment rate: {} msats/second", config.payment_rate_msats_per_sec);
    println!("⏱️  Minimum duration: {} seconds", config.minimum_pod_duration_seconds);
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
