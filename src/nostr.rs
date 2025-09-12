// Nostr client for receiving pod provisioning events
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
    config: RelayConfig,
}

impl NostrRelaySubscriber {
    pub async fn new(config: RelayConfig) -> Result<Self> {
        let keys = match &config.private_key {
            Some(private_key_hex) if !private_key_hex.is_empty() => {
                Keys::parse(private_key_hex)
                    .context("Invalid private key format")?
            }
            _ => {
                // Always generate a new key for security
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

        Ok(Self { client, keys, config })
    }

    pub async fn subscribe_to_pod_events<F>(&self, handler: F) -> Result<()>
    where
        F: Fn(NostrEvent) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>> + Send + Sync + 'static,
    {
        // Subscribe to pod provisioning events (kind 1000)
        let filter = Filter::new()
            .kind(Kind::Custom(1000))
            .limit(0);

        self.client.subscribe(vec![filter], None).await;
        info!("Subscribed to pod provisioning events (kind 1000)");

        // Handle incoming events
        self.client.handle_notifications(|notification| async {
            if let RelayPoolNotification::Event { relay_url: _, subscription_id: _, event } = notification {
                let nostr_event = self.convert_event(&event);
                
                match handler(nostr_event).await {
                    Ok(()) => {
                        info!("Successfully processed event: {}", event.id);
                    }
                    Err(e) => {
                        error!("Failed to process event {}: {}", event.id, e);
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
        let builder = EventBuilder::new(Kind::Custom(20000), content, tags);
        let event = builder.to_event(&self.keys)?;
        let event_id = event.id.to_hex();
        self.client.send_event(event).await?;
        info!("Published offer event: {}", event_id);
        Ok(event_id)
    }

    pub async fn publish_access_details(&self, request_event_id: &str, details: AccessDetailsContent) -> Result<String> {
        let content = serde_json::to_string(&details)?;
        let tags = vec![
            Tag::event(request_event_id.parse()?),
            Tag::hashtag("paygress"),
            Tag::hashtag("response"),
        ];
        let builder = EventBuilder::new(Kind::Custom(1001), content, tags);
        let event = builder.to_event(&self.keys)?;
        let event_id = event.id.to_hex();
        self.client.send_event(event).await?;
        info!("Published access details event in reply to {}: {}", request_event_id, event_id);
        Ok(event_id)
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
    pub ssh_port: u16,
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
    pub ssh_port: u16,
    pub node_port: Option<u16>,
    pub expires_at: String,
    pub instructions: Vec<String>,
}
