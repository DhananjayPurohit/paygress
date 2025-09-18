// Nostr client for receiving pod provisioning events with NIP-44 encryption
use anyhow::{Context, Result};
use nostr_sdk::{Client, Keys, Filter, Kind, RelayPoolNotification, Url, EventBuilder, Tag};
use nostr_sdk::nips::nip44;
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
    config: RelayConfig,
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
            let url = Url::parse(relay_url)
                .with_context(|| format!("Invalid relay URL: {}", relay_url))?;
            client.add_relay(url).await?;
        }

        client.connect().await;
        info!("Connected to {} relays", config.relays.len());
        info!("Service public key (npub): {}", keys.public_key().to_hex());
        // Note: Private key (nsec) is not logged for security

        Ok(Self { client, keys, config })
    }

    pub async fn subscribe_to_pod_events<F>(&self, handler: F) -> Result<()>
    where
        F: Fn(NostrEvent) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>> + Send + Sync + 'static,
    {
        // Subscribe to pod provisioning events (kind 1000) and top-up events (kind 1002)
        let filter = Filter::new()
            .kind(Kind::Custom(1000))
            .limit(0);
        
        let topup_filter = Filter::new()
            .kind(Kind::Custom(1002))
            .limit(0);

        let _ = self.client.subscribe(vec![filter, topup_filter], None).await;
        info!("Subscribed to encrypted pod provisioning events (kind 1000) and top-up events (kind 1002)");

        // Handle incoming events
        self.client.handle_notifications(|notification| async {
            if let RelayPoolNotification::Event { relay_url: _, subscription_id: _, event } = notification {
                let nostr_event = self.convert_event(&event);
                
                match handler(nostr_event).await {
                    Ok(()) => {
                        info!("Successfully processed encrypted event: {}", event.id);
                    }
                    Err(e) => {
                        error!("Failed to process encrypted event {}: {}", event.id, e);
                    }
                }
            }
            Ok(false) // Continue listening
        }).await?;

        Ok(())
    }

    pub async fn publish_offer(&self, offer: OfferEventContent) -> Result<String> {
        let content = serde_json::to_string(&offer)?;
        let tags = vec![
            Tag::hashtag("paygress"),
            Tag::hashtag("offer"),
        ];
        let builder = EventBuilder::new(Kind::Custom(999), content, tags);
        let event = builder.to_event(&self.keys)?;
        let event_id = event.id.to_hex();
        self.client.send_event(event).await?;
        info!("Published offer event: {}", event_id);
        Ok(event_id)
    }

    // NEW: Encrypted access details publishing
    pub async fn publish_encrypted_access_details(
        &self, 
        request_event_id: &str, 
        request_pubkey: &str,
        details: AccessDetailsContent
    ) -> Result<String> {
        // Serialize the access details
        let details_json = serde_json::to_string(&details)?;
        
        // Encrypt the content using NIP-04
        let request_pubkey_parsed = nostr_sdk::PublicKey::parse(request_pubkey)?;
        let encrypted_content = nip44::encrypt(
            self.keys.secret_key()?,
            &request_pubkey_parsed,
            &details_json,
            nip44::Version::V2
        )?;

        let tags = vec![
            Tag::event(request_event_id.parse()?),
            Tag::hashtag("paygress"),
            Tag::hashtag("response"),
            Tag::hashtag("encrypted"),
        ];
        
        let builder = EventBuilder::new(Kind::Custom(1001), encrypted_content, tags);
        let event = builder.to_event(&self.keys)?;
        let event_id = event.id.to_hex();
        self.client.send_event(event).await?;
        info!("Published encrypted access details event in reply to {}: {}", request_event_id, event_id);
        Ok(event_id)
    }

    // NEW: Decrypt incoming encrypted events
    pub fn decrypt_event_content(&self, event: &NostrEvent) -> Result<String> {
        // Decrypt the content using NIP-44
        let sender_pubkey = nostr_sdk::PublicKey::parse(&event.pubkey)?;
        let decrypted = nip44::decrypt(
            self.keys.secret_key()?,
            &sender_pubkey,
            &event.content
        )?;
        Ok(decrypted)
    }

    // NEW: Check if event is encrypted
    pub fn is_encrypted_event(&self, event: &NostrEvent) -> bool {
        // Check if the event has encryption tags or if content looks encrypted
        event.tags.iter().any(|tag| {
            tag.len() >= 2 && tag[0] == "encrypted"
        }) || event.content.starts_with("nip44:")
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
    pub kind: String, // "offer"
    pub rate_sats_per_hour: u64,
    pub default_duration_minutes: u64,
    pub pod_namespace: String,
    pub image: String,
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
    pub ssh_username: Option<String>,
    pub pod_image: Option<String>,
}

// NEW: Encrypted top-up request structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedTopUpPodRequest {
    pub pod_name: String,
    pub cashu_token: String,
}

// NEW: Helper function to create encrypted provisioning request
pub fn create_encrypted_provisioning_request(
    service_pubkey: &str,
    user_keys: &Keys,
    request: EncryptedSpawnPodRequest,
) -> Result<String> {
    let request_json = serde_json::to_string(&request)?;
    
    // Encrypt the request content
    let service_pubkey_parsed = nostr_sdk::PublicKey::parse(service_pubkey)?;
    let encrypted_content = nip44::encrypt(
        user_keys.secret_key()?,
        &service_pubkey_parsed,
        &request_json,
        nip44::Version::V2
    )?;

    Ok(encrypted_content)
}

// NEW: Helper function to decrypt provisioning request
pub fn decrypt_provisioning_request(
    service_keys: &Keys,
    sender_pubkey: &str,
    encrypted_content: &str,
) -> Result<EncryptedSpawnPodRequest> {
    let sender_pubkey = nostr_sdk::PublicKey::parse(sender_pubkey)?;
    let decrypted = nip44::decrypt(
        service_keys.secret_key()?,
        &sender_pubkey,
        encrypted_content
    )?;
    
    let request: EncryptedSpawnPodRequest = serde_json::from_str(&decrypted)?;
    Ok(request)
}
