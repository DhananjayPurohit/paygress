// Nostr Interface for Paygress
//
// Handles NIP-17 encrypted private messages for pod provisioning
// using a shared PodProvisioningService instance.

use anyhow::Result;
use std::sync::Arc;
use tracing::{info, error, warn};
use nostr_sdk::ToBech32;

use crate::pod_provisioning::PodProvisioningService;
use crate::nostr::{
    custom_relay_config, NostrRelaySubscriber,
    OfferEventContent, EncryptedSpawnPodRequest, EncryptedTopUpPodRequest,
    parse_private_message_content
};
use crate::sidecar_service::{SidecarState, PodInfo};

/// Run the Nostr interface
pub async fn run_nostr_interface(service: Arc<PodProvisioningService>) -> Result<()> {
    info!("🌐 Starting Nostr interface...");
    
    let config = service.get_config();
    
    // Validate configuration
    if config.pod_specs.is_empty() {
        error!("❌ No pod specifications configured for Nostr interface");
        return Err(anyhow::anyhow!("No pod specifications configured"));
    }

    // Get relay configuration from environment
    let relay_cfg = get_relay_config()?;
    let nostr = NostrRelaySubscriber::new(relay_cfg).await?;

    // Publish initial offer event
    if let Err(e) = publish_offer_event(&nostr, config).await {
        warn!("Failed to publish offer event: {}", e);
    }

    // Set up message handlers
    let service_for_spawn = Arc::clone(&service);
    let service_for_topup = Arc::clone(&service);

    info!("✅ Nostr interface ready - listening for encrypted messages");

    // Start listening for messages
    nostr.subscribe_to_pod_events(move |event| {
        let service_spawn = Arc::clone(&service_for_spawn);
        let service_topup = Arc::clone(&service_for_topup);
        
        Box::pin(async move {
            // Try to parse as spawn pod request first
            if let Ok(spawn_request) = parse_private_message_content(&event.content) {
                handle_spawn_pod_request(service_spawn, spawn_request).await
            }
            // Try to parse as topup request
            else if let Ok(topup_request) = serde_json::from_str::<EncryptedTopUpPodRequest>(&event.content) {
                handle_topup_pod_request(service_topup, topup_request).await
            }
            else {
                warn!("Failed to parse private message content: {}", event.content);
                Ok(())
            }
        })
    }).await?;

    Ok(())
}

/// Handle spawn pod request from Nostr
async fn handle_spawn_pod_request(
    service: Arc<PodProvisioningService>,
    request: EncryptedSpawnPodRequest,
) -> Result<()> {
    info!("📨 Received spawn pod request via Nostr");

    // Convert to MCP tool format and call the shared service
    let spawn_tool = crate::pod_provisioning::SpawnPodTool {
        cashu_token: request.cashu_token,
        pod_spec_id: request.pod_spec_id,
        pod_image: request.pod_image,
        ssh_username: request.ssh_username,
        ssh_password: request.ssh_password,
        user_pubkey: None, // Nostr requests don't include user_pubkey
    };

    match service.spawn_pod(spawn_tool).await {
        Ok(response) => {
            if response.success {
                info!("✅ Pod created successfully via Nostr: {}", 
                      response.pod_npub.as_deref().unwrap_or("unknown"));
            } else {
                warn!("❌ Failed to create pod via Nostr: {}", response.message);
            }
        }
        Err(e) => {
            error!("❌ Error creating pod via Nostr: {}", e);
        }
    }

    Ok(())
}

/// Handle topup pod request from Nostr
async fn handle_topup_pod_request(
    service: Arc<PodProvisioningService>,
    request: EncryptedTopUpPodRequest,
) -> Result<()> {
    info!("📨 Received topup pod request via Nostr");

    // Convert to MCP tool format and call the shared service
    let topup_tool = crate::pod_provisioning::TopUpPodTool {
        pod_npub: request.pod_npub,
        cashu_token: request.cashu_token,
    };

    match service.topup_pod(topup_tool).await {
        Ok(response) => {
            if response.success {
                info!("✅ Pod topped up successfully via Nostr: {}", response.pod_npub);
            } else {
                warn!("❌ Failed to topup pod via Nostr: {}", response.message);
            }
        }
        Err(e) => {
            error!("❌ Error topping up pod via Nostr: {}", e);
        }
    }

    Ok(())
}

/// Publish offer event to Nostr relays
async fn publish_offer_event(
    nostr: &NostrRelaySubscriber,
    config: &crate::sidecar_service::SidecarConfig,
) -> Result<()> {
    let offer_content = OfferEventContent {
        minimum_duration_seconds: config.minimum_pod_duration_seconds,
        whitelisted_mints: config.whitelisted_mints.clone(),
        pod_specs: config.pod_specs.clone(),
    };

    nostr.publish_offer(offer_content).await?;
    info!("📢 Published offer event to Nostr relays");

    Ok(())
}

/// Get relay configuration from environment variables
fn get_relay_config() -> Result<crate::nostr::RelayConfig> {
    // Get relays from environment
    let relays_str = std::env::var("NOSTR_RELAYS")
        .unwrap_or_else(|_| "wss://relay.damus.io,wss://nos.lol".to_string());
    
    let relays: Vec<String> = relays_str
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();

    if relays.is_empty() {
        return Err(anyhow::anyhow!("No Nostr relays configured"));
    }

    // Get private key from environment (nsec format)
    let private_key = match std::env::var("NOSTR_PRIVATE_KEY") {
        Ok(key) => Some(key),
        Err(_) => {
            error!("❌ Error: NOSTR_PRIVATE_KEY environment variable is required");
            error!("   Please set NOSTR_PRIVATE_KEY with your Nostr private key (nsec format)");
            return Err(anyhow::anyhow!("NOSTR_PRIVATE_KEY not set"));
        }
    };

    Ok(custom_relay_config(relays, private_key))
}
