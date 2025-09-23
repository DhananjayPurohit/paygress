use paygress::sidecar_service::{start_sidecar_service, SidecarConfig};
use paygress::nostr::{
    default_relay_config, custom_relay_config, NostrRelaySubscriber, 
    OfferEventContent, EncryptedSpawnPodRequest, EncryptedTopUpPodRequest,
    parse_private_message_content
};
use paygress::sidecar_service::{SidecarState, PodInfo};
use chrono::Utc;
use std::env;
use tracing_subscriber::fmt::init;
use nostr_sdk;
use axum::http::StatusCode;
use tracing::info;

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
        payment_rate_msats_per_sec: env::var("PAYMENT_RATE_MSATS_PER_SEC")
            .unwrap_or_else(|_| "100".to_string())
            .parse()
            .unwrap_or(100),
        minimum_pod_duration_seconds: env::var("MINIMUM_POD_DURATION_SECONDS")
            .unwrap_or_else(|_| "60".to_string())
            .parse()
            .unwrap_or(60),
        base_image: env::var("BASE_IMAGE")
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
        println!("🚀 Starting in Nostr mode with private messaging...");
        run_private_message_nostr_mode(config).await?;
    } else {
        println!("🌐 Starting in HTTP mode...");
        start_sidecar_service(&bind_addr, config).await?;
    }

    Ok(())
}

async fn run_private_message_nostr_mode(config: SidecarConfig) -> Result<(), Box<dyn std::error::Error>> {
    let state = SidecarState::new(config.clone()).await?;

    // Pod cleanup is now handled by Kubernetes TTL annotations
    // No need for polling-based cleanup loops

    // Get relay configuration from environment
    let relay_cfg = get_relay_config();
    let nostr = NostrRelaySubscriber::new(relay_cfg).await?;

    // Publish an initial offer
    let offer = OfferEventContent {
        rate_msats_per_sec: config.payment_rate_msats_per_sec,
        minimum_duration_seconds: config.minimum_pod_duration_seconds,
        memory_mb: 1024, // 1GB memory
        cpu_millicores: 1000, // 1 CPU core
        whitelisted_mints: config.whitelisted_mints.clone(),
    };
    match nostr.publish_offer(offer).await {
        Ok(event_id) => {
            println!("✅ Published offer event: {}", event_id);
        }
        Err(e) => {
            println!("❌ Failed to publish offer event: {}", e);
        }
    }

    // Subscribe and handle private messages for provisioning and top-up requests
    let nostr_clone = nostr.clone();
    let handler = move |event: paygress::nostr::NostrEvent| {
        let state_clone = state.clone();
        let nostr_clone = nostr_clone.clone();
        Box::pin(async move {
            // Check if event is a private message
            if !nostr_clone.is_private_message(&event) {
                tracing::warn!("Received non-private message event, ignoring for security");
                return Ok(());
            }

            // Get the content from private message (already decrypted by client)
            let message_content = match nostr_clone.get_private_message_content(&event) {
                Ok(content) => content,
                Err(e) => {
                    tracing::warn!("Failed to get private message content: {}", e);
                    return Ok(());
                }
            };

            // Debug: Log the received message content
            tracing::debug!("Received message content: {}", message_content);

            // Try to parse as pod creation request first
            match parse_private_message_content(&message_content) {
                Ok(request) => {
                    tracing::info!("Successfully parsed as pod creation request");
                    handle_spawn_pod_request(state_clone, request, &event.pubkey, nostr_clone.clone()).await?;
                    return Ok(());
                }
                Err(e) => {
                    tracing::warn!("Failed to parse as pod creation request: {}", e);
                }
            }

            // Try to parse as top-up request
            match serde_json::from_str::<EncryptedTopUpPodRequest>(&message_content) {
                Ok(request) => {
                    tracing::info!("Successfully parsed as top-up request");
                    handle_top_up_request(state_clone, request).await?;
                    return Ok(());
                }
                Err(e) => {
                    tracing::debug!("Failed to parse as top-up request: {}", e);
                }
            }

            tracing::warn!("Could not parse private message content as valid request. Content: {}", message_content);

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
    nostr_client: paygress::nostr::NostrRelaySubscriber,
) -> Result<(), anyhow::Error> {
    // Decode token to get amount and duration
    let payment_amount_msats = match paygress::sidecar_service::extract_token_value(&request.cashu_token).await {
        Ok(msats) => msats,
        Err(e) => {
            tracing::warn!("Failed token decode: {}", e);
            return Ok(());
        }
    };
    // Check if payment is sufficient for minimum duration
    if !state_clone.is_payment_sufficient(payment_amount_msats) {
        tracing::warn!("Insufficient payment: {} msats (minimum required: {} msats for {} seconds)", 
            payment_amount_msats, 
            state_clone.config.minimum_pod_duration_seconds * state_clone.config.payment_rate_msats_per_sec,
            state_clone.config.minimum_pod_duration_seconds);
        return Ok(());
    }

    // Calculate duration based on payment only
    let duration_seconds = state_clone.calculate_duration_from_payment(payment_amount_msats);

    // Verify token validity (1 msat sanity)
    match paygress::cashu::verify_cashu_token(&request.cashu_token, 1, &state_clone.config.whitelisted_mints).await {
        Ok(true) => {}
        _ => { return Ok(()); }
    }

    // Generate NPUB first and use it as pod name
    let pod_keys = nostr_sdk::Keys::generate();
    let pod_npub = pod_keys.public_key().to_bech32().unwrap();
    let pod_nsec = pod_keys.secret_key().unwrap().to_secret_hex();
    
    // Create Kubernetes-safe pod name from NPUB (take first 8 chars after npub1 prefix)
    let pod_name = format!("pod-{}", pod_npub.replace("npub1", "").chars().take(8).collect::<String>());
    let username = request.ssh_username;
    let password = request.ssh_password;
    let image = request.pod_image.unwrap_or_else(|| state_clone.config.base_image.clone());
    let ssh_port = match state_clone.generate_ssh_port() {
        Ok(port) => port,
        Err(e) => {
            tracing::error!("Failed to allocate SSH port: {}", e);
            return Ok(());
        }
    };

    let now = Utc::now();
    let expires_at = now + chrono::Duration::seconds(duration_seconds as i64);

    match state_clone.k8s_client.create_ssh_pod(
        &state_clone.config,
        &state_clone.config.pod_namespace,
        &pod_name,
        &pod_npub,
        &pod_nsec,
        &image,
        ssh_port,
        &username,
        &password,
        duration_seconds,
        1024, // 1GB memory
        1000, // 1 CPU core
        user_pubkey, // Pass user's public key
    ).await {
        Ok(node_port) => {
            let pod_info = PodInfo {
                pod_npub: pod_npub.clone(),
                namespace: state_clone.config.pod_namespace.clone(),
                created_at: now,
                expires_at,
                allocated_port: ssh_port, // The allocated port from port pool (this is the SSH port)
                ssh_username: username.clone(),
                ssh_password: password.clone(),
                payment_amount_msats,
                duration_seconds,
                node_port: Some(node_port),
                nostr_public_key: pod_npub.clone(),
                nostr_private_key: pod_nsec,
            };
            state_clone.active_pods.write().await.insert(pod_npub.clone(), pod_info.clone());

            // Send access details via NIP-17 Gift Wrap private message
            let access_details = paygress::nostr::AccessDetailsContent {
                kind: "access_details".to_string(),
                pod_name: pod_name.clone(),
                namespace: state_clone.config.pod_namespace.clone(),
                ssh_username: username.clone(),
                ssh_password: password.clone(),
                node_port: Some(node_port),
                expires_at: expires_at.to_rfc3339(),
                instructions: vec![
                    "🚀 SSH access available:".to_string(),
                    "".to_string(),
                    "Direct access (no kubectl needed):".to_string(),
                    format!("   ssh -o PreferredAuthentications=password -o PubkeyAuthentication=no {}@{} -p {}", username, state_clone.config.ssh_host, node_port),
                    "".to_string(),
                    "⚠️  Pod expires at:".to_string(),
                    format!("   {}", expires_at.format("%Y-%m-%d %H:%M:%S UTC")),
                    "".to_string(),
                    "📋 Pod Details:".to_string(),
                    format!("   Pod NPUB: {}", pod_npub),
                    format!("   Duration: {} seconds", duration_seconds),
                ],
            };

            match nostr_client.send_access_details_private_message(user_pubkey, access_details).await {
                Ok(event_id) => {
                    info!("✅ Sent access details via NIP-17 Gift Wrap private message to {}: {}", user_pubkey, event_id);
                }
                Err(e) => {
                    tracing::error!("❌ Failed to send access details via private message: {}", e);
                }
            }

            info!("Pod with NPUB {} created and access details sent via NIP-17 Gift Wrap private message", pod_npub);
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
    info!("Pod top-up request received for NPUB: {}", request.pod_npub);

    // Store NPUB for logging before moving it
    let pod_npub = request.pod_npub.clone();
    
    // Convert to HTTP request format and delegate to HTTP handler
    let top_up_request = paygress::sidecar_service::TopUpPodRequest {
        pod_npub: request.pod_npub,
        cashu_token: request.cashu_token,
    };
    
    // Call the HTTP handler logic directly
    let response = paygress::sidecar_service::top_up_pod_handler(state_clone, top_up_request).await;
    
    // Log the response for debugging
    match response.status() {
        StatusCode::OK => info!("✅ Top-up request processed successfully for NPUB: {}", pod_npub),
        StatusCode::NOT_FOUND => tracing::warn!("❌ Pod with NPUB {} not found or expired", pod_npub),
        StatusCode::PAYMENT_REQUIRED => tracing::warn!("❌ Payment verification failed for NPUB: {}", pod_npub),
        StatusCode::INTERNAL_SERVER_ERROR => tracing::error!("❌ Internal error processing top-up for NPUB: {}", pod_npub),
        _ => tracing::warn!("❌ Unexpected response for top-up request NPUB: {}", pod_npub),
    }
    
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
