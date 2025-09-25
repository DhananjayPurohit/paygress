use paygress::sidecar_service::{start_sidecar_service, SidecarConfig};
use paygress::nostr::{
    custom_relay_config, NostrRelaySubscriber, 
    OfferEventContent, EncryptedSpawnPodRequest, EncryptedTopUpPodRequest,
    parse_private_message_content
};
use paygress::sidecar_service::{SidecarState, PodInfo};
use chrono::Utc;
use std::env;
use tracing_subscriber::fmt::init;
use nostr_sdk::ToBech32;
use axum::http::StatusCode;
use axum::body;
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
        whitelisted_mints: {
            let mints_str = match env::var("WHITELISTED_MINTS") {
                Ok(mints) => mints,
                Err(_) => {
                    eprintln!("❌ Error: WHITELISTED_MINTS environment variable is required");
                    eprintln!("   Please set WHITELISTED_MINTS with comma-separated mint URLs");
                    eprintln!("   Example: WHITELISTED_MINTS=https://mint.cashu.space,https://mint.f7z.io");
                    std::process::exit(1);
                }
            };
            
            let mints: Vec<String> = mints_str
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
                
            if mints.is_empty() {
                eprintln!("❌ Error: WHITELISTED_MINTS contains no valid mint URLs");
                eprintln!("   WHITELISTED_MINTS value: {}", mints_str);
                std::process::exit(1);
            }
            
            mints
        },
        pod_specs: get_pod_specs_from_env(),
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

    // Validate that we have at least one pod specification
    if config.pod_specs.is_empty() {
        eprintln!("❌ Error: No pod specifications configured");
        eprintln!("   Please provide at least one pod specification in pod-specs.json file");
        std::process::exit(1);
    }

    // Publish an initial offer
    let offer = OfferEventContent {
        minimum_duration_seconds: config.minimum_pod_duration_seconds,
        whitelisted_mints: config.whitelisted_mints.clone(),
        pod_specs: config.pod_specs.clone(),
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
    let nostr_for_handler = nostr.clone();
        let handler = move |event: paygress::nostr::NostrEvent| {
        let state_clone = state.clone();
        let nostr_for_handler = nostr_for_handler.clone();
        Box::pin(async move {
            // Content is already unwrapped in the notification handler
            let message_content = event.content.clone();

            // Debug: Log the received message content
            tracing::debug!("Received message content: {}", message_content);

            // Try to parse as pod creation request first
            match parse_private_message_content(&message_content) {
                Ok(request) => {
                    tracing::info!("Successfully parsed as pod creation request");
                    handle_spawn_pod_request(state_clone, request, &event.pubkey, nostr_for_handler.clone()).await?;
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
                    handle_top_up_request(state_clone, request, nostr_for_handler.clone()).await?;
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
    // Select pod specification
    let pod_spec = if let Some(spec_id) = &request.pod_spec_id {
        state_clone.config.pod_specs.iter().find(|s| s.id == *spec_id)
    } else {
        state_clone.config.pod_specs.first()
    };
    
    let pod_spec = match pod_spec {
        Some(spec) => spec,
        None => {
            let error = paygress::nostr::ErrorResponseContent {
                error_type: "invalid_spec".to_string(),
                message: format!("Pod specification '{}' not found", request.pod_spec_id.as_deref().unwrap_or("default")),
                details: Some("Please check available specifications in the offer event".to_string()),
            };
            if let Err(e) = nostr_client.send_error_response_private_message(user_pubkey, error).await {
                tracing::error!("Failed to send error response: {}", e);
            }
            return Ok(());
        }
    };

    // Decode token to get amount and duration
    let payment_amount_msats = match paygress::sidecar_service::extract_token_value(&request.cashu_token).await {
        Ok(msats) => msats,
        Err(e) => {
            let error = paygress::nostr::ErrorResponseContent {
                error_type: "invalid_token".to_string(),
                message: "Failed to decode Cashu token".to_string(),
                details: Some(format!("Token decode error: {}", e)),
            };
            if let Err(e) = nostr_client.send_error_response_private_message(user_pubkey, error).await {
                tracing::error!("Failed to send error response: {}", e);
            }
            return Ok(());
        }
    };
    
    // Check if payment is sufficient for minimum duration with selected spec
    let minimum_payment = state_clone.config.minimum_pod_duration_seconds * pod_spec.rate_msats_per_sec;
    if payment_amount_msats < minimum_payment {
        let error = paygress::nostr::ErrorResponseContent {
            error_type: "insufficient_payment".to_string(),
            message: format!("Insufficient payment: {} msats", payment_amount_msats),
            details: Some(format!("Minimum required: {} msats for {} seconds with {} spec (rate: {} msats/sec)", 
                minimum_payment,
                state_clone.config.minimum_pod_duration_seconds,
                pod_spec.name,
                pod_spec.rate_msats_per_sec)),
        };
        if let Err(e) = nostr_client.send_error_response_private_message(user_pubkey, error).await {
            tracing::error!("Failed to send error response: {}", e);
        }
        return Ok(());
    }

    // Calculate duration based on payment and selected spec rate
    let duration_seconds = payment_amount_msats / pod_spec.rate_msats_per_sec;

    // Verify token validity (1 msat sanity)
    match paygress::cashu::verify_cashu_token(&request.cashu_token, 1, &state_clone.config.whitelisted_mints).await {
        Ok(true) => {}
        Ok(false) => {
            let error = paygress::nostr::ErrorResponseContent {
                error_type: "invalid_token".to_string(),
                message: "Cashu token verification failed".to_string(),
                details: Some("Token is invalid or not from a whitelisted mint".to_string()),
            };
            if let Err(e) = nostr_client.send_error_response_private_message(user_pubkey, error).await {
                tracing::error!("Failed to send error response: {}", e);
            }
            return Ok(());
        }
        Err(e) => {
            let (error_type, message, details) = if e.contains("already been used") {
                (
                    "token_already_used".to_string(),
                    "Cashu token has already been used".to_string(),
                    Some("This token was already processed in a previous request".to_string())
                )
            } else {
                (
                    "verification_error".to_string(),
                    "Failed to verify Cashu token".to_string(),
                    Some(format!("Verification error: {}", e))
                )
            };
            
            let error = paygress::nostr::ErrorResponseContent {
                error_type,
                message,
                details,
            };
            if let Err(e) = nostr_client.send_error_response_private_message(user_pubkey, error).await {
                tracing::error!("Failed to send error response: {}", e);
            }
            return Ok(());
        }
    }

    // Generate NPUB first and use it as pod name
    let pod_keys = nostr_sdk::Keys::generate();
    let pod_npub = pod_keys.public_key().to_bech32().unwrap();
    let pod_nsec = pod_keys.secret_key().unwrap().to_secret_hex();
    
    // Create Kubernetes-safe pod name from NPUB (take first 8 chars after npub1 prefix)
    let pod_name = format!("pod-{}", pod_npub.replace("npub1", "").chars().take(8).collect::<String>());
    let username = request.ssh_username;
    let password = request.ssh_password;
    let image = request.pod_image;
    let ssh_port = match state_clone.generate_ssh_port() {
        Ok(port) => port,
        Err(e) => {
            let error = paygress::nostr::ErrorResponseContent {
                error_type: "resource_unavailable".to_string(),
                message: "Failed to allocate SSH port".to_string(),
                details: Some(format!("Port allocation error: {}", e)),
            };
            if let Err(e) = nostr_client.send_error_response_private_message(user_pubkey, error).await {
                tracing::error!("Failed to send error response: {}", e);
            }
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
        pod_spec.memory_mb,
        pod_spec.cpu_millicores,
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
                pod_npub: pod_npub.clone(),
                node_port,
                expires_at: expires_at.to_rfc3339(),
                cpu_millicores: pod_spec.cpu_millicores,
                memory_mb: pod_spec.memory_mb,
                pod_spec_name: pod_spec.name.clone(),
                pod_spec_description: pod_spec.description.clone(),
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
                    format!("   Specification: {} ({})", pod_spec.name, pod_spec.description),
                    format!("   CPU: {} millicores", pod_spec.cpu_millicores),
                    format!("   Memory: {} MB", pod_spec.memory_mb),
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
            let error = paygress::nostr::ErrorResponseContent {
                error_type: "pod_creation_failed".to_string(),
                message: "Failed to create pod".to_string(),
                details: Some(format!("Pod creation error: {}", e)),
            };
            if let Err(e) = nostr_client.send_error_response_private_message(user_pubkey, error).await {
                tracing::error!("Failed to send error response: {}", e);
            }
        }
    }

    Ok(())
}

// Handle top-up request in Nostr mode
async fn handle_top_up_request(
    state_clone: SidecarState,
    request: EncryptedTopUpPodRequest,
    nostr_client: paygress::nostr::NostrRelaySubscriber,
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
    
    // Send appropriate response based on status
    match response.status() {
        StatusCode::OK => {
            // Send success response
            let success_response = paygress::nostr::TopUpResponseContent {
                success: true,
                pod_npub: pod_npub.clone(),
                extended_duration_seconds: 0, // We'll calculate this properly later
                new_expires_at: "2025-12-31T23:59:59Z".to_string(), // Placeholder
                message: "Pod successfully topped up!".to_string(),
            };
            if let Err(e) = nostr_client.send_topup_response_private_message(&pod_npub, success_response).await {
                tracing::error!("Failed to send top-up success response: {}", e);
            }
            info!("✅ Top-up request processed successfully for NPUB: {}", pod_npub);
        }
        StatusCode::NOT_FOUND => {
            let error = paygress::nostr::ErrorResponseContent {
                error_type: "pod_not_found".to_string(),
                message: "Pod not found or expired".to_string(),
                details: Some(format!("Pod with NPUB '{}' not found or already expired", pod_npub)),
            };
            if let Err(e) = nostr_client.send_error_response_private_message(&pod_npub, error).await {
                tracing::error!("Failed to send error response: {}", e);
            }
            tracing::warn!("❌ Pod with NPUB {} not found or expired", pod_npub);
        }
        StatusCode::PAYMENT_REQUIRED => {
            let error = paygress::nostr::ErrorResponseContent {
                error_type: "payment_failed".to_string(),
                message: "Payment verification failed".to_string(),
                details: Some("Cashu token verification failed or insufficient payment".to_string()),
            };
            if let Err(e) = nostr_client.send_error_response_private_message(&pod_npub, error).await {
                tracing::error!("Failed to send error response: {}", e);
            }
            tracing::warn!("❌ Payment verification failed for NPUB: {}", pod_npub);
        }
        StatusCode::INTERNAL_SERVER_ERROR => {
            let error = paygress::nostr::ErrorResponseContent {
                error_type: "internal_error".to_string(),
                message: "Internal server error".to_string(),
                details: Some("Failed to process top-up request".to_string()),
            };
            if let Err(e) = nostr_client.send_error_response_private_message(&pod_npub, error).await {
                tracing::error!("Failed to send error response: {}", e);
            }
            tracing::error!("❌ Internal error processing top-up for NPUB: {}", pod_npub);
        }
        _ => {
            let error = paygress::nostr::ErrorResponseContent {
                error_type: "unknown_error".to_string(),
                message: "Unexpected error occurred".to_string(),
                details: Some("Unknown error processing top-up request".to_string()),
            };
            if let Err(e) = nostr_client.send_error_response_private_message(&pod_npub, error).await {
                tracing::error!("Failed to send error response: {}", e);
            }
            tracing::warn!("❌ Unexpected response for top-up request NPUB: {}", pod_npub);
        }
    }
    
    Ok(())
}

// Function to get pod specifications from JSON file
fn get_pod_specs_from_env() -> Vec<paygress::nostr::PodSpec> {
    // Get the pod specs file path from environment variable
    let specs_file = env::var("POD_SPECS_FILE").unwrap_or_else(|_| "/app/pod-specs.json".to_string());
    
    // Read the JSON file
    match std::fs::read_to_string(&specs_file) {
        Ok(specs_json) => {
            match serde_json::from_str::<Vec<paygress::nostr::PodSpec>>(&specs_json) {
                Ok(specs) => {
                    if !specs.is_empty() {
                        println!("✅ Loaded {} pod specifications from {}", specs.len(), specs_file);
                        return specs;
                    } else {
                        eprintln!("❌ Error: Pod specifications file '{}' contains empty array", specs_file);
                    }
                }
                Err(e) => {
                    eprintln!("❌ Error: Failed to parse pod specifications from '{}': {}", specs_file, e);
                    eprintln!("   Please ensure the JSON file contains valid pod specifications");
                }
            }
        }
        Err(e) => {
            eprintln!("❌ Error: Failed to read pod specifications file '{}': {}", specs_file, e);
            eprintln!("   Please ensure the file exists and is readable");
            eprintln!("   You can set POD_SPECS_FILE environment variable to specify a different file path");
        }
    }
    
    eprintln!("❌ Error: No valid pod specifications found");
    eprintln!("   Expected file: {}", specs_file);
    eprintln!("   Example pod-specs.json content:");
    eprintln!(r#"   [
     {{
       "id": "basic",
       "name": "Basic",
       "description": "Basic VPS - 1 CPU core, 1GB RAM",
       "cpu_millicores": 1000,
       "memory_mb": 1024,
       "rate_msats_per_sec": 100
     }}
   ]"#);
    std::process::exit(1);
}

// Function to get relay configuration from environment variables
fn get_relay_config() -> paygress::nostr::RelayConfig {
    // Get relays from environment variable (comma-separated)
    let relays_str = match env::var("NOSTR_RELAYS") {
        Ok(relays) => relays,
        Err(_) => {
            eprintln!("❌ Error: NOSTR_RELAYS environment variable is required");
            eprintln!("   Please set NOSTR_RELAYS with comma-separated relay URLs");
            eprintln!("   Example: NOSTR_RELAYS=wss://relay.damus.io,wss://nos.lol,wss://relay.nostr.band");
            std::process::exit(1);
        }
    };
    
    let relays: Vec<String> = relays_str
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    
    if relays.is_empty() {
        eprintln!("❌ Error: NOSTR_RELAYS contains no valid relay URLs");
        eprintln!("   NOSTR_RELAYS value: {}", relays_str);
        std::process::exit(1);
    }
    
    // Get private key from environment (nsec format)
    let private_key = match env::var("NOSTR_PRIVATE_KEY") {
        Ok(key) => Some(key),
        Err(_) => {
            eprintln!("❌ Error: NOSTR_PRIVATE_KEY environment variable is required");
            eprintln!("   Please set NOSTR_PRIVATE_KEY with your Nostr private key (nsec format)");
            std::process::exit(1);
        }
    };
    
    custom_relay_config(relays, private_key)
}
