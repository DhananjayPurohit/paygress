use paygress::sidecar_service::{start_sidecar_service, SidecarConfig};
use paygress::nostr::{default_relay_config, custom_relay_config, NostrRelaySubscriber, OfferEventContent, AccessDetailsContent};
use paygress::sidecar_service::{SidecarState, PodInfo};
use chrono::Utc;
use std::env;
use tracing_subscriber::fmt::init;
use kube::{Client, Api};
use k8s_openapi::api::core::v1::Pod;

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
        ssh_port: env::var("SSH_PORT")
            .unwrap_or_else(|_| "2222".to_string())
            .parse()
            .unwrap_or(2222),
        enable_cleanup_task: env::var("ENABLE_CLEANUP_TASK")
            .unwrap_or_else(|_| "true".to_string())
            .parse()
            .unwrap_or(true),
        enable_tor_sidecar: env::var("ENABLE_TOR_SIDECAR")
            .unwrap_or_else(|_| "false".to_string())
            .parse()
            .unwrap_or(false),
        tor_image: env::var("TOR_IMAGE")
            .unwrap_or_else(|_| "goldy/tor-hidden-service:latest".to_string()),
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
        println!("🚀 Starting in Nostr mode...");
        run_nostr_mode(config).await?;
    } else {
        println!("🌐 Starting in HTTP mode...");
        start_sidecar_service(&bind_addr, config).await?;
    }

    Ok(())
}

async fn run_nostr_mode(config: SidecarConfig) -> Result<(), Box<dyn std::error::Error>> {
    let state = SidecarState::new(config.clone()).await?;

    // Get relay configuration from environment
    let relay_cfg = get_relay_config();
    let nostr = NostrRelaySubscriber::new(relay_cfg).await?;

    // Publish an initial offer
    let offer = OfferEventContent {
        kind: "offer".into(),
        rate_sats_per_hour: config.payment_rate_sats_per_hour,
        default_duration_minutes: config.default_pod_duration_minutes,
        ssh_port: config.ssh_port,
        pod_namespace: config.pod_namespace.clone(),
        image: config.ssh_base_image.clone(),
    };
    let _ = nostr.publish_offer(offer).await;

    // Subscribe and handle provisioning requests (kind 1000)
    let nostr_clone = nostr.clone();
    let handler = move |event: paygress::nostr::NostrEvent| {
        let state_clone = state.clone();
        let nostr_clone = nostr_clone.clone();
        Box::pin(async move {
            // Expect event.content to be JSON: { cashu_token, ssh_username?, pod_image? }
            let request: Result<paygress::sidecar_service::SpawnPodRequest, _> = serde_json::from_str(&event.content);
            let request = match request {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!("Invalid request content: {}", e);
                    return Ok(());
                }
            };

            // Decode token to get amount and duration
            let payment_amount_sats = match paygress::sidecar_service::extract_token_value(&request.cashu_token).await {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!("Failed token decode: {}", e);
                    return Ok(());
                }
            };
            let duration_minutes = state_clone.calculate_duration_from_payment(payment_amount_sats);
            if duration_minutes == 0 { return Ok(()); }

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
                &state_clone.config.pod_namespace,
                &pod_name,
                &image,
                ssh_port,
                &username,
                &password,
                duration_minutes,
                state_clone.config.enable_tor_sidecar,
                &state_clone.config.tor_image,
            ).await {
                Ok(node_port) => {
                    let pod_info = PodInfo {
                        pod_name: pod_name.clone(),
                        namespace: state_clone.config.pod_namespace.clone(),
                        created_at: now,
                        expires_at,
                        ssh_port,
                        ssh_username: username.clone(),
                        ssh_password: password.clone(),
                        payment_amount_sats,
                        duration_minutes,
                        node_port: Some(node_port),
                    };
                    state_clone.active_pods.write().await.insert(pod_name.clone(), pod_info.clone());

                    if state_clone.config.enable_tor_sidecar {
                        // EVENT 1: Immediate response - Pod created, Tor onion being generated
                        let immediate_instructions = vec![
                            "✅ Pod created successfully!".to_string(),
                            "".to_string(),
                            "🌐 DECENTRALIZED SSH ACCESS (No Public IP Required)".to_string(),
                            "".to_string(),
                            "⏳ Tor onion address is being generated...".to_string(),
                            "This usually takes 30-60 seconds.".to_string(),
                            "".to_string(),
                            "You'll receive another event when the onion address is ready.".to_string(),
                            "".to_string(),
                            "💡 Benefits:".to_string(),
                            "   - No public IP address required".to_string(),
                            "   - No kubectl or Kubernetes access needed".to_string(),
                            "   - NAT traversal handled by Tor".to_string(),
                            "   - Fully decentralized access".to_string(),
                            "   - Works from any remote machine".to_string(),
                        ];

                        let immediate_details = AccessDetailsContent {
                            kind: "access_details".into(),
                            pod_name: pod_name.clone(),
                            namespace: state_clone.config.pod_namespace.clone(),
                            ssh_username: username.clone(),
                            ssh_password: password.clone(),
                            ssh_port: 2222,
                            node_port: Some(node_port),
                            expires_at: pod_info.expires_at.to_rfc3339(),
                            instructions: immediate_instructions,
                        };
                        let _ = nostr_clone.publish_access_details(&event.id, immediate_details).await;

                        // EVENT 2: Delayed response - Tor onion address ready
                        let nostr_clone_2 = nostr_clone.clone();
                        let pod_name_clone = pod_name.clone();
                        let username_clone = username.clone();
                        let password_clone = password.clone();
                        let namespace_clone = state_clone.config.pod_namespace.clone();
                        let expires_at_clone = pod_info.expires_at.to_rfc3339();
                        
                        tokio::spawn(async move {
                            // Wait for Tor to generate onion address with retries
                            let onion_address = get_onion_address_with_retry(&namespace_clone, &pod_name_clone).await;
                            
                            let mut tor_instructions = vec![
                                "🌐 TOR ONION ADDRESS READY!".to_string(),
                                "".to_string(),
                                "✅ Decentralized access is now available:".to_string(),
                                "".to_string(),
                                "1. Install Tor on your machine: https://www.torproject.org/download/".to_string(),
                                "".to_string(),
                                "2. Connect via Tor:".to_string(),
                            ];

                            match onion_address {
                                Ok(addr) => {
                                    tor_instructions.extend(vec![
                                        format!("   torsocks ssh -o PreferredAuthentications=password -o PubkeyAuthentication=no {}@{} -p 2222", username_clone, addr),
                                        "".to_string(),
                                        format!("3. Password: {}", password_clone),
                                        "".to_string(),
                                        "💡 Benefits:".to_string(),
                                        "   - No public IP address required".to_string(),
                                        "   - No kubectl or Kubernetes access needed".to_string(),
                                        "   - NAT traversal handled by Tor".to_string(),
                                        "   - Fully decentralized access".to_string(),
                                        "   - Works from any remote machine".to_string(),
                                        "".to_string(),
                                        "🔄 If connection fails, wait a few more seconds for the onion address to propagate.".to_string(),
                                    ]);
                                },
                                Err(e) => {
                                    tor_instructions.extend(vec![
                                        "❌ Failed to retrieve onion address".to_string(),
                                        format!("Error: {}", e),
                                        "".to_string(),
                                        "The pod is running with Tor sidecar, but onion address retrieval failed.".to_string(),
                                        "You can still access the pod using:".to_string(),
                                        "".to_string(),
                                        "1. Port forwarding (fallback method):".to_string(),
                                        format!("   kubectl -n {} port-forward svc/{}-ssh {}:{}", namespace_clone, pod_name_clone, ssh_port, ssh_port),
                                        format!("   ssh -o PreferredAuthentications=password -o PubkeyAuthentication=no {}@localhost -p {}", username_clone, ssh_port),
                                        "".to_string(),
                                        "2. Check pod logs for details:".to_string(),
                                        format!("   kubectl -n {} logs {}-tor-sidecar", namespace_clone, pod_name_clone),
                                        "".to_string(),
                                        format!("Password: {}", password_clone),
                                    ]);
                                }
                            }

                            let tor_details = AccessDetailsContent {
                                kind: "access_details".into(),
                                pod_name: pod_name_clone.clone(),
                                namespace: namespace_clone.clone(),
                                ssh_username: username_clone.clone(),
                                ssh_password: password_clone.clone(),
                                ssh_port: 2222,
                                node_port: None,
                                expires_at: expires_at_clone,
                                instructions: tor_instructions,
                            };
                            let _ = nostr_clone_2.publish_access_details(&event.id, tor_details).await;
                        });
                    } else {
                        // Fallback: Traditional access (only if Tor is disabled)
                        let fallback_instructions = vec![
                            "⚠️  Traditional access (requires kubectl):".to_string(),
                            "".to_string(),
                            "Note: This method requires kubectl access and won't work on remote machines.".to_string(),
                            "Consider enabling Tor sidecar for decentralized access.".to_string(),
                            "".to_string(),
                            format!("kubectl -n {} port-forward svc/{}-ssh {}:{}", state_clone.config.pod_namespace, pod_name, ssh_port, ssh_port),
                            format!("ssh -o PreferredAuthentications=password -o PubkeyAuthentication=no {}@localhost -p {}", username, ssh_port),
                            format!("Password: {}", password),
                        ];

                        let fallback_details = AccessDetailsContent {
                            kind: "access_details".into(),
                            pod_name: pod_name.clone(),
                            namespace: state_clone.config.pod_namespace.clone(),
                            ssh_username: username.clone(),
                            ssh_password: password.clone(),
                            ssh_port,
                            node_port: Some(node_port),
                            expires_at: pod_info.expires_at.to_rfc3339(),
                            instructions: fallback_instructions,
                        };
                        let _ = nostr_clone.publish_access_details(&event.id, fallback_details).await;
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to create pod: {}", e);
                }
            }

            Ok(())
        }) as std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send>>
    };

    // Kick off subscription loop (await runs forever)
    let _ = nostr.subscribe_to_pod_events(handler).await;
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
    
    // Always generate a new private key for security (no configurable private key)
    // If no relays specified, use default
    if relays.is_empty() {
        default_relay_config()
    } else {
        custom_relay_config(relays, None) // Always None for private key
    }
}

// Function to get onion address with retries
async fn get_onion_address_with_retry(namespace: &str, pod_name: &str) -> Result<String, String> {
    let mut attempts = 0;
    let max_attempts = 6; // Try for up to 1 minute
    
    while attempts < max_attempts {
        match get_onion_address(namespace, pod_name).await {
            Ok(addr) => return Ok(addr),
            Err(e) => {
                attempts += 1;
                if attempts < max_attempts {
                    let delay = 10 * attempts; // 10, 20, 30, 40, 50 seconds
                    tracing::info!("Onion address not ready yet, waiting {} seconds... (attempt {}/{})", delay, attempts, max_attempts);
                    tokio::time::sleep(tokio::time::Duration::from_secs(delay)).await;
                } else {
                    return Err(format!("Failed to get onion address after {} attempts: {}", max_attempts, e));
                }
            }
        }
    }
    
    Err("Max attempts reached".to_string())
}

// Function to automatically get the onion address from the Tor sidecar using Kubernetes API
async fn get_onion_address(namespace: &str, pod_name: &str) -> Result<String, String> {
    use std::process::Command;
    
    // Use kubectl exec to get the onion address from the Tor sidecar
    let output = Command::new("kubectl")
        .args(&[
            "-n", namespace,
            "exec", pod_name,
            "-c", "tor-sidecar",
            "--", "cat", "/var/lib/tor/hidden_service/hostname"
        ])
        .output()
        .map_err(|e| format!("Failed to execute kubectl: {}", e))?;

    if !output.status.success() {
        return Err(format!("kubectl exec failed: {}", String::from_utf8_lossy(&output.stderr)));
    }

    let onion_address = String::from_utf8(output.stdout)
        .map_err(|e| format!("Failed to parse onion address: {}", e))?
        .trim()
        .to_string();

    if onion_address.is_empty() {
        return Err("Empty onion address from Tor sidecar".to_string());
    }

    // Validate that it looks like an onion address
    if !onion_address.ends_with(".onion") {
        return Err(format!("Invalid onion address format: {}", onion_address));
    }

    Ok(onion_address)
}