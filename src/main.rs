use paygress::sidecar_service::{start_sidecar_service, SidecarConfig};
use paygress::nostr::{
    default_relay_config, custom_relay_config, NostrRelaySubscriber, 
    OfferEventContent, EncryptedSpawnPodRequest, EncryptedTopUpPodRequest,
    create_encrypted_provisioning_request, decrypt_provisioning_request
};
use paygress::sidecar_service::{SidecarState, PodInfo};
use chrono::Utc;
use std::env;
use tracing_subscriber::fmt::init;
use tracing::info;
use kube::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    init();

    // Get configuration from environment
    let bind_addr = env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
    let cashu_db_path = env::var("CASHU_DB_PATH").unwrap_or_else(|_| "./cashu.db".to_string());

    // Enhanced sidecar service with SSH pod provisioning
    let config = SidecarConfig {
        cashu_db_path,
        pod_namespace: env::var("POD_NAMESPACE")
            .unwrap_or_else(|_| "user-workloads".to_string()),
        payment_rate_sats_per_hour: env::var("PAYMENT_RATE_SATS_PER_HOUR")
            .unwrap_or_else(|_| "100".to_string())
            .parse()
            .unwrap_or(100),
        default_pod_duration_minutes: env::var("DEFAULT_POD_DURATION_MINUTES")
            .unwrap_or_else(|_| "60".to_string())
            .parse()
            .unwrap_or(60),
        ssh_base_image: env::var("SSH_BASE_IMAGE")
            .unwrap_or_else(|_| "linuxserver/openssh-server:latest".to_string()),
        ssh_host: env::var("SSH_HOST")
            .unwrap_or_else(|_| "localhost".to_string()),
        ssh_port_range_start: env::var("SSH_PORT_RANGE_START")
            .unwrap_or_else(|_| "30000".to_string())
            .parse()
            .unwrap_or(30000),
        ssh_port_range_end: env::var("SSH_PORT_RANGE_END")
            .unwrap_or_else(|_| "31000".to_string())
            .parse()
            .unwrap_or(31000),
        enable_cleanup_task: env::var("ENABLE_CLEANUP_TASK")
            .unwrap_or_else(|_| "true".to_string())
            .parse()
            .unwrap_or(true),
        whitelisted_mints: env::var("WHITELISTED_MINTS")
            .unwrap_or_else(|_| "https://mint.cashu.space,https://mint.f7z.io,https://legend.lnbits.com/cashu/api/v1".to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
    };
    
    let run_mode = env::var("RUN_MODE").unwrap_or_else(|_| "nostr".to_string());
    
    println!("🔧 RUN_MODE environment variable: '{}'", run_mode);
    println!("🔧 RUN_MODE comparison: nostr == '{}' -> {}", run_mode, run_mode == "nostr");

    if run_mode.trim() == "nostr" {
        println!("🚀 Starting in Nostr mode with encryption...");
        run_encrypted_nostr_mode(config).await?;
    } else {
        println!("🌐 Starting in HTTP mode...");
        start_sidecar_service(&bind_addr, config).await?;
    }

    Ok(())
}

async fn run_encrypted_nostr_mode(config: SidecarConfig) -> Result<(), Box<dyn std::error::Error>> {
    let state = SidecarState::new(config.clone()).await?;

    // Pod cleanup is now handled by Kubernetes TTL annotations
    // No need for polling-based cleanup loops

    // Get relay configuration from environment
    let relay_cfg = get_relay_config();
    let nostr = NostrRelaySubscriber::new(relay_cfg).await?;

    // Publish an initial offer
    let offer = OfferEventContent {
        kind: "offer".into(),
        rate_sats_per_hour: config.payment_rate_sats_per_hour,
        default_duration_minutes: config.default_pod_duration_minutes,
        pod_namespace: config.pod_namespace.clone(),
        image: config.ssh_base_image.clone(),
    };
    let _ = nostr.publish_offer(offer).await;

    // Subscribe and handle encrypted provisioning requests (kind 1000)
    let nostr_clone = nostr.clone();
    let handler = move |event: paygress::nostr::NostrEvent| {
        let state_clone = state.clone();
        let nostr_clone = nostr_clone.clone();
        Box::pin(async move {
            // Check if event is encrypted
            // if !nostr_clone.is_encrypted_event(&event) {
            //     tracing::warn!("Received unencrypted event, ignoring for security");
            //     return Ok(());
            // }

            // Decrypt the event content
            let decrypted_content = match nostr_clone.decrypt_event_content(&event) {
                Ok(content) => content,
                Err(e) => {
                    tracing::warn!("Failed to decrypt event content: {}", e);
                    return Ok(());
                }
            };

            // Parse the decrypted request based on event kind
            if event.kind == 1000 {
                // Pod creation request
                let request: Result<EncryptedSpawnPodRequest, _> = serde_json::from_str(&decrypted_content);
                let request = match request {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::warn!("Invalid decrypted spawn request content: {}", e);
                        return Ok(());
                    }
                };
                
                handle_spawn_pod_request(state_clone, request, &event.pubkey).await?;
            } else if event.kind == 1002 {
                // Top-up request
                let request: Result<EncryptedTopUpPodRequest, _> = serde_json::from_str(&decrypted_content);
                let request = match request {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::warn!("Invalid decrypted top-up request content: {}", e);
                        return Ok(());
                    }
                };
                
                handle_top_up_request(state_clone, request).await?;
            } else {
                tracing::warn!("Unsupported event kind: {}", event.kind);
                return Ok(());
            }

            Ok(())
        }) as std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send>>
    };

    // Kick off subscription loop (await runs forever)
    let _ = nostr.subscribe_to_pod_events(handler).await;
    Ok(())
}

// Handle pod spawn request in Nostr mode
async fn handle_spawn_pod_request(
    state_clone: SidecarState,
    request: EncryptedSpawnPodRequest,
    user_pubkey: &str,
) -> Result<(), anyhow::Error> {
    // Decode token to get amount and duration
    let payment_amount_sats = match paygress::sidecar_service::extract_token_value(&request.cashu_token).await {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("Failed token decode: {}", e);
            return Ok(());
        }
    };
    // Calculate duration based on payment only
    let duration_minutes = state_clone.calculate_duration_from_payment(payment_amount_sats);

    if duration_minutes == 0 { 
        tracing::warn!("Invalid duration: 0 minutes");
        return Ok(());
    }

    // Verify token validity (1 msat sanity)
    match paygress::cashu::verify_cashu_token(&request.cashu_token, 1, &state_clone.config.whitelisted_mints).await {
        Ok(true) => {}
        _ => { return Ok(()); }
    }

    // Prepare pod attributes
    let pod_name = format!("ssh-pod-{}", uuid::Uuid::new_v4().to_string()[..8].to_string());
    let username = request.ssh_username.unwrap_or_else(|| format!("user-{}", &pod_name[8..16]));
    let password = SidecarState::generate_password();
    let image = request.pod_image.unwrap_or_else(|| state_clone.config.ssh_base_image.clone());
    let ssh_port = state_clone.generate_ssh_port();

    let now = Utc::now();
    let expires_at = now + chrono::Duration::minutes(duration_minutes as i64);

    match state_clone.k8s_client.create_ssh_pod(
        &state_clone.config,
        &state_clone.config.pod_namespace,
        &pod_name,
        &image,
        ssh_port,
        &username,
        &password,
        duration_minutes,
        user_pubkey, // Pass user's public key
    ).await {
        Ok((node_port, pod_npub, pod_nsec)) => {
            let pod_info = PodInfo {
                pod_name: pod_name.clone(),
                namespace: state_clone.config.pod_namespace.clone(),
                created_at: now,
                expires_at,
                allocated_port: ssh_port, // The allocated port from port pool (this is the SSH port)
                ssh_username: username.clone(),
                ssh_password: password.clone(),
                payment_amount_sats,
                duration_minutes,
                node_port: Some(node_port),
                nostr_public_key: pod_npub,
                nostr_private_key: pod_nsec,
            };
            state_clone.active_pods.write().await.insert(pod_name.clone(), pod_info.clone());

            info!("Pod {} created and will send its own access event", pod_name);
        }
        Err(e) => {
            tracing::error!("Failed to create pod: {}", e);
        }
    }

    Ok(())
}

// Handle top-up request in Nostr mode
async fn handle_top_up_request(
    state_clone: SidecarState,
    request: EncryptedTopUpPodRequest,
) -> Result<(), anyhow::Error> {
    info!("Pod top-up request received for pod: {}", request.pod_name);

    // Check if pod exists
    let mut pods = state_clone.active_pods.write().await;
    let pod_info = match pods.get_mut(&request.pod_name) {
        Some(pod) => pod,
        None => {
            tracing::warn!("Pod '{}' not found or already expired", request.pod_name);
            return Ok(());
        }
    };

    // Check if pod has already expired
    let now = Utc::now();
    if now > pod_info.expires_at {
        // Remove expired pod from active pods
        pods.remove(&request.pod_name);
        tracing::warn!("Pod '{}' has already expired and cannot be extended", request.pod_name);
        return Ok(());
    }

    // Extract payment amount from token
    let payment_amount_sats = match paygress::sidecar_service::extract_token_value(&request.cashu_token).await {
        Ok(sats) => sats,
        Err(e) => {
            tracing::warn!("Failed to decode Cashu token: {}", e);
            return Ok(());
        }
    };

    // Calculate additional duration from payment
    let additional_duration_minutes = state_clone.calculate_duration_from_payment(payment_amount_sats);
    
    if additional_duration_minutes == 0 {
        tracing::warn!("Insufficient payment for top-up: {} sats", payment_amount_sats);
        return Ok(());
    }

    // Verify payment token validity
    match paygress::cashu::verify_cashu_token(&request.cashu_token, 1, &state_clone.config.whitelisted_mints).await {
        Ok(true) => {
            info!("✅ Top-up payment verified: {} sats for {} additional minutes", payment_amount_sats, additional_duration_minutes);
        }
        _ => {
            tracing::warn!("Top-up payment verification failed");
            return Ok(());
        }
    }

    // Extend the pod's expiration time in memory
    let old_expires_at = pod_info.expires_at;
    pod_info.expires_at = pod_info.expires_at + chrono::Duration::minutes(additional_duration_minutes as i64);
    pod_info.payment_amount_sats += payment_amount_sats;
    pod_info.duration_minutes += additional_duration_minutes;

    // Update the pod's activeDeadlineSeconds in Kubernetes
    if let Err(e) = state_clone.k8s_client.extend_pod_deadline(&state_clone.config.pod_namespace, &request.pod_name, additional_duration_minutes).await {
        tracing::error!("Failed to extend pod deadline in Kubernetes: {}", e);
        return Ok(());
    }

    info!(
        "🔄 Pod '{}' extended: {} → {} (added {} minutes)",
        request.pod_name,
        old_expires_at.format("%H:%M:%S UTC"),
        pod_info.expires_at.format("%H:%M:%S UTC"),
        additional_duration_minutes
    );

    Ok(())
}

// Function to get relay configuration from environment variables
fn get_relay_config() -> paygress::nostr::RelayConfig {
    // Get relays from environment variable (comma-separated)
    let relays_str = env::var("NOSTR_RELAYS").unwrap_or_else(|_| {
        "wss://relay.damus.io,wss://nos.lol,wss://relay.nostr.band".to_string()
    });
    
    let relays: Vec<String> = relays_str
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    
    // Get private key from environment (nsec format)
    let private_key = env::var("NOSTR_PRIVATE_KEY").ok();
    
    // If no relays specified, use default
    if relays.is_empty() {
        default_relay_config()
    } else {
        custom_relay_config(relays, private_key)
    }
}
