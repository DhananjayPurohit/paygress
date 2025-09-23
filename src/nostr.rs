// Nostr client for receiving pod provisioning events with private messaging
use anyhow::{Context, Result};
use nostr_sdk::{Client, Keys, Filter, Kind, RelayPoolNotification, Url, EventBuilder, Tag};
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use std::future::Future;
use tracing::{error, info};

#[derive(Clone, Debug)]
pub struct RelayConfig {
    pub relays: Vec<String>,
    pub private_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NostrEvent {
    pub id: String,
    pub pubkey: String,
    pub created_at: u64,
    pub kind: u32,
    pub tags: Vec<Vec<String>>,
    pub content: String,
    pub sig: String,
}

#[derive(Clone)]
pub struct NostrRelaySubscriber {
    client: Client,
    keys: Keys,
    // config field removed - not used in current implementation
}

impl NostrRelaySubscriber {
    pub async fn new(config: RelayConfig) -> Result<Self> {
        let keys = match &config.private_key {
            Some(private_key_hex) if !private_key_hex.is_empty() => {
                // Parse as nsec format (nostr private key)
                if private_key_hex.starts_with("nsec1") {
                    Keys::parse(private_key_hex)
                        .context("Invalid nsec private key format")?
                } else {
                    // Assume hex format for backward compatibility
                    Keys::parse(private_key_hex)
                        .context("Invalid private key format")?
                }
            }
            _ => {
                // Generate a new key if none provided
                Keys::generate()
            }
        };

        let client = Client::new(&keys);

        // Add relays
        for relay_url in &config.relays {
            info!("Adding relay: {}", relay_url);
            let url = Url::parse(relay_url)
                .with_context(|| format!("Invalid relay URL: {}", relay_url))?;
            client.add_relay(url).await?;
        }

        info!("Connecting to {} relays...", config.relays.len());
        client.connect().await;
        
        // Wait a moment for connections to establish
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
        
        info!("Connected to {} relays", config.relays.len());
        info!("Service public key (npub): {}", keys.public_key().to_hex());
        // Note: Private key (nsec) is not logged for security

        Ok(Self { client, keys })
    }

    pub async fn subscribe_to_pod_events<F>(&self, handler: F) -> Result<()>
    where
        F: Fn(NostrEvent) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>> + Send + Sync + 'static,
    {
        // Subscribe to NIP-17 private direct messages (kind 14) for private pod provisioning and top-up requests
        let filter = Filter::new()
            .kind(Kind::Custom(14))
            .limit(0);

        let _ = self.client.subscribe(vec![filter], None).await;
        info!("Subscribed to NIP-17 private direct messages for pod provisioning and top-up requests");

        // Handle incoming events
        self.client.handle_notifications(|notification| async {
            if let RelayPoolNotification::Event { relay_url: _, subscription_id: _, event } = notification {
                let nostr_event = self.convert_event(&event);
                
                match handler(nostr_event).await {
                    Ok(()) => {
                        info!("Successfully processed private message: {}", event.id);
                    }
                    Err(e) => {
                        error!("Failed to process private message {}: {}", event.id, e);
                    }
                }
            }
            Ok(false) // Continue listening
        }).await?;

        Ok(())
    }

    pub async fn publish_offer(&self, offer: OfferEventContent) -> Result<String> {
        let content = serde_json::to_string(&offer)?;
        info!("Publishing offer event with content: {}", content);
        
        let tags = vec![
            Tag::hashtag("paygress"),
            Tag::hashtag("offer"),
        ];
        
        info!("Creating event with kind 999 and {} tags", tags.len());
        let builder = EventBuilder::new(Kind::Custom(999), content, tags);
        let event = builder.to_event(&self.keys)?;
        let event_id = event.id.to_hex();
        
        info!("Event created with ID: {}", event_id);
        info!("Sending offer event to relays: {}", event_id);
        
        match self.client.send_event(event).await {
            Ok(res) => {
                info!("✅ Successfully published offer event: {} and {:?}", event_id, res);
                Ok(event_id)
            }
            Err(e) => {
                error!("❌ Failed to send offer event: {}", e);
                Err(e.into())
            }
        }
    }

<<<<<<< Updated upstream
    // NEW: Send access details via private message
=======
    // NEW: Send access details via private encrypted message (NIP-17 Gift Wrap)
>>>>>>> Stashed changes
    pub async fn send_access_details_private_message(
        &self, 
        request_pubkey: &str,
        details: AccessDetailsContent
    ) -> Result<String> {
        // Serialize the access details
        let details_json = serde_json::to_string(&details)?;
        
<<<<<<< Updated upstream
        // Send as NIP-17 private direct message (kind 14)
=======
        // Send as NIP-17 Gift Wrap private message
>>>>>>> Stashed changes
        let request_pubkey_parsed = nostr_sdk::PublicKey::parse(request_pubkey)?;
        let event_id = self.client.send_private_msg(request_pubkey_parsed, details_json, None).await?;
        
        info!("Sent access details via NIP-17 Gift Wrap private message to {}: {:?}", request_pubkey, event_id);
        Ok(event_id.to_hex())
    }


    // NEW: Get content from private messages (already decrypted by client)
    pub fn get_private_message_content(&self, event: &NostrEvent) -> Result<String> {
        // For direct messages, the content is already decrypted by the client
        Ok(event.content.clone())
    }

    // NEW: Check if event is a NIP-17 private direct message
    pub fn is_private_message(&self, event: &NostrEvent) -> bool {
        event.kind == 14 // Kind 14 is NIP-17 Private Direct Message
    }

    // NEW: Get service public key for users
    pub fn get_service_public_key(&self) -> String {
        self.keys.public_key().to_hex()
    }

    fn convert_event(&self, event: &nostr_sdk::Event) -> NostrEvent {
        NostrEvent {
            id: event.id.to_hex(),
            pubkey: event.pubkey.to_hex(),
            created_at: event.created_at.as_u64(),
            kind: event.kind.as_u32(),
            tags: event.tags.iter().map(|tag| {
                tag.as_vec().iter().map(|s| s.to_string()).collect()
            }).collect(),
            content: event.content.clone(),
            sig: event.sig.to_string(),
        }
    }
}

pub fn default_relay_config() -> RelayConfig {
    RelayConfig {
        relays: vec![
            "wss://relay.damus.io".to_string(),
            "wss://nos.lol".to_string(),
            "wss://relay.nostr.band".to_string(),
        ],
        private_key: None,
    }
}

pub fn custom_relay_config(relays: Vec<String>, private_key: Option<String>) -> RelayConfig {
    RelayConfig { relays, private_key }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfferEventContent {
    pub rate_msats_per_sec: u64,
    pub minimum_duration_seconds: u64,
    pub memory_mb: u64, // Memory in MB
    pub cpu_millicores: u64, // CPU in millicores (1000 millicores = 1 CPU core)
    pub whitelisted_mints: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessDetailsContent {
    pub kind: String, // "access_details"
    pub pod_name: String,
    pub namespace: String,
    pub ssh_username: String,
    pub ssh_password: String,
    pub node_port: Option<u16>,
    pub expires_at: String,
    pub instructions: Vec<String>,
}

// NEW: Encrypted request structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedSpawnPodRequest {
    pub cashu_token: String,
    pub pod_image: Option<String>, // Optional: Uses base image if not specified
    pub ssh_username: String,
    pub ssh_password: String,
}

// NEW: Encrypted top-up request structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedTopUpPodRequest {
    pub pod_npub: String,    // Pod's NPUB identifier
    pub cashu_token: String,
}

// NEW: Helper function to send private message provisioning request
pub async fn send_provisioning_request_private_message(
    client: &Client,
    service_pubkey: &str,
    request: EncryptedSpawnPodRequest,
) -> Result<String> {
    let request_json = serde_json::to_string(&request)?;
    
    // Send as private message
    let service_pubkey_parsed = nostr_sdk::PublicKey::parse(service_pubkey)?;
    let event_id = client.send_private_msg(service_pubkey_parsed, request_json, None).await?;

    Ok(event_id.to_hex())
}

// NEW: Helper function to parse private message content
pub fn parse_private_message_content(content: &str) -> Result<EncryptedSpawnPodRequest> {
    match serde_json::from_str::<EncryptedSpawnPodRequest>(content) {
        Ok(request) => Ok(request),
        Err(e) => {
            // Provide detailed error information
            Err(anyhow::anyhow!("JSON parsing failed: {}. Content: '{}'", e, content))
        }
    }
}

