// Nostr client for receiving pod provisioning events with NIP-17 encrypted private messaging
use anyhow::{Context, Result};
use nostr_sdk::{Client, Keys, Filter, Kind, RelayPoolNotification, Url, EventBuilder, Tag, nip44};
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
        // Subscribe to NIP-17 gift wraps (kind 1059) for private pod provisioning and top-up requests
        let filter = Filter::new()
            .kind(Kind::Custom(1059))
            .limit(0);

        let _ = self.client.subscribe(vec![filter], None).await;
        info!("Subscribed to NIP-17 gift wraps (kind 1059) for private pod provisioning and top-up requests");

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

    // NEW: Send access details via private message
    pub async fn send_access_details_private_message(
        &self, 
        request_pubkey: &str,
        details: AccessDetailsContent
    ) -> Result<String> {
        // Serialize the access details
        let details_json = serde_json::to_string(&details)?;
        
        // Send as NIP-17 private direct message with NIP-44 encryption and NIP-59 seals/gift wraps
        let request_pubkey_parsed = nostr_sdk::PublicKey::parse(request_pubkey)?;
        let event_id = self.send_nip17_message(request_pubkey_parsed, details_json).await?;
        
        info!("Sent access details via private message to {}: {:?}", request_pubkey, event_id);
        Ok(event_id.to_hex())
    }

    // Send NIP-17 message with NIP-44 encryption and NIP-59 seals/gift wraps
    async fn send_nip17_message(&self, recipient: nostr_sdk::PublicKey, content: String) -> Result<nostr_sdk::EventId> {
        use nostr_sdk::{EventBuilder, Kind, Tag};
        
        // Create the unsigned kind 14 message
        let message_content = EventBuilder::new(Kind::Custom(14), content, [Tag::pubkey(recipient, None)])
            .to_unsigned_event(self.client.keys().public_key());
        
        // Seal the message (kind 13) with NIP-44 encryption
        let sealed_content = nip44::encrypt(
            self.client.keys().secret_key(),
            &recipient,
            &serde_json::to_string(&message_content)?
        )?;
        
        let seal = EventBuilder::new(Kind::Custom(13), sealed_content, [])
            .to_signed_event(self.client.keys())?;
        
        // Gift wrap the sealed message (kind 1059)
        let gift_wrap_keys = Keys::generate();
        let gift_wrap_content = nip44::encrypt(
            gift_wrap_keys.secret_key(),
            &recipient,
            &serde_json::to_string(&seal)?
        )?;
        
        let gift_wrap = EventBuilder::new(Kind::Custom(1059), gift_wrap_content, [Tag::pubkey(recipient, None)])
            .to_signed_event(&gift_wrap_keys)?;
        
        // Publish the gift wrap
        let event_id = self.client.send_event(gift_wrap).await?;
        
        Ok(event_id)
    }

    // NEW: Get content from private messages (decrypt NIP-17 messages)
    pub fn get_private_message_content(&self, event: &NostrEvent) -> Result<String> {
        // For NIP-17 messages, we need to decrypt the gift wrap content
        if event.kind == 1059 {
            // This is a gift wrap - decrypt it to get the sealed message
            let decrypted_content = nip44::decrypt(
                self.client.keys().secret_key(),
                &event.pubkey,
                &event.content
            )?;
            
            let seal: nostr_sdk::Event = serde_json::from_str(&decrypted_content)?;
            
            if seal.kind == 13 {
                // Decrypt the sealed message to get the original kind 14 message
                let original_content = nip44::decrypt(
                    self.client.keys().secret_key(),
                    &seal.pubkey,
                    &seal.content
                )?;
                
                let original_message: nostr_sdk::Event = serde_json::from_str(&original_content)?;
                
                if original_message.kind == 14 {
                    Ok(original_message.content)
                } else {
                    Err(anyhow::anyhow!("Expected kind 14 message, got kind {}", original_message.kind))
                }
            } else {
                Err(anyhow::anyhow!("Expected sealed message (kind 13), got kind {}", seal.kind))
            }
        } else {
            // Fallback for non-NIP-17 messages
            Ok(event.content.clone())
        }
    }

    // NEW: Check if event is a NIP-17 gift wrap
    pub fn is_private_message(&self, event: &NostrEvent) -> bool {
        event.kind == 1059 // Kind 1059 is NIP-17 Gift Wrap
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
    pub ssh_username: Option<String>,
    pub ssh_password: Option<String>,
}

// NEW: Encrypted top-up request structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedTopUpPodRequest {
    pub pod_npub: String,    // Changed from pod_name to pod_npub
    pub cashu_token: String,
}

// NEW: Helper function to send private message provisioning request
pub async fn send_provisioning_request_private_message(
    client: &Client,
    service_pubkey: &str,
    request: EncryptedSpawnPodRequest,
) -> Result<String> {
    let request_json = serde_json::to_string(&request)?;
    
    // Send as NIP-17 private message with NIP-44 encryption and NIP-59 seals/gift wraps
    let service_pubkey_parsed = nostr_sdk::PublicKey::parse(service_pubkey)?;
    let event_id = send_nip17_message_with_client(&client, service_pubkey_parsed, request_json).await?;

    Ok(event_id.to_hex())
}

// Helper function to send NIP-17 message with a client
async fn send_nip17_message_with_client(client: &Client, recipient: nostr_sdk::PublicKey, content: String) -> Result<nostr_sdk::EventId> {
    use nostr_sdk::{EventBuilder, Kind, Tag};
    
    // Create the unsigned kind 14 message
    let message_content = EventBuilder::new(Kind::Custom(14), content, [Tag::pubkey(recipient, None)])
        .to_unsigned_event(client.keys().public_key());
    
    // Seal the message (kind 13) with NIP-44 encryption
    let sealed_content = nip44::encrypt(
        client.keys().secret_key(),
        &recipient,
        &serde_json::to_string(&message_content)?
    )?;
    
    let seal = EventBuilder::new(Kind::Custom(13), sealed_content, [])
        .to_signed_event(client.keys())?;
    
    // Gift wrap the sealed message (kind 1059)
    let gift_wrap_keys = Keys::generate();
    let gift_wrap_content = nip44::encrypt(
        gift_wrap_keys.secret_key(),
        &recipient,
        &serde_json::to_string(&seal)?
    )?;
    
    let gift_wrap = EventBuilder::new(Kind::Custom(1059), gift_wrap_content, [Tag::pubkey(recipient, None)])
        .to_signed_event(&gift_wrap_keys)?;
    
    // Publish the gift wrap
    let event_id = client.send_event(gift_wrap).await?;
    
    Ok(event_id)
}

// NEW: Helper function to parse private message content
pub fn parse_private_message_content(content: &str) -> Result<EncryptedSpawnPodRequest> {
    let request: EncryptedSpawnPodRequest = serde_json::from_str(content)?;
    Ok(request)
}
