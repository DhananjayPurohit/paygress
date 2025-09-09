use paygress::sidecar_service::{start_sidecar_service, SidecarConfig};
use paygress::nostr::{default_relay_config, NostrRelaySubscriber, OfferEventContent, AccessDetailsContent};
use paygress::sidecar_service::{SidecarState, PodInfo};
use chrono::Utc;
use std::env;
use tracing_subscriber::fmt::init;

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
    };
    
    let run_mode = env::var("RUN_MODE").unwrap_or_else(|_| "http".to_string());
    
    println!("🔧 RUN_MODE environment variable: '{}'", run_mode);
    println!("🔧 RUN_MODE comparison: nostr == '{}' -> {}", run_mode, run_mode == "nostr");

    if run_mode.trim() == "nostr" {
        println!("🚀 Starting in Nostr mode...");
        // Nostr-only mode: publish offer loop and handle provisioning requests
        run_nostr_mode(config).await?;
    } else {
        println!("🌐 Starting in HTTP mode...");
        start_sidecar_service(&bind_addr, config).await?;
    }

    Ok(())
}

async fn run_nostr_mode(config: SidecarConfig) -> Result<(), Box<dyn std::error::Error>> {
    let state = SidecarState::new(config.clone()).await?;

    let relay_cfg = default_relay_config();
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
            match paygress::cashu::verify_cashu_token(&request.cashu_token, 1).await {
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

                    // Publish access details in a reply event (kind 1001)
                    let details = AccessDetailsContent {
                        kind: "access_details".into(),
                        pod_name: pod_name.clone(),
                        namespace: state_clone.config.pod_namespace.clone(),
                        ssh_username: username.clone(),
                        ssh_password: password.clone(),
                        ssh_port,
                        node_port,
                        expires_at: pod_info.expires_at.to_rfc3339(),
                        instructions: vec![
                            format!("kubectl -n {} port-forward svc/{}-ssh {}:{}", state_clone.config.pod_namespace, pod_name, ssh_port, ssh_port),
                            format!("ssh -o PreferredAuthentications=password -o PubkeyAuthentication=no {}@localhost -p {}", username, ssh_port),
                            format!("Password: {}", password),
                        ],
                    };
                    let _ = nostr_clone.publish_access_details(&event.id, details).await;
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