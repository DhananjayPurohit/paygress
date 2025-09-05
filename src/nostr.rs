// Nostr client for receiving pod provisioning events
use anyhow::{Context, Result};
use nostr_sdk::{Client, Keys, Filter, Kind, RelayPoolNotification, Url};
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

pub struct NostrRelaySubscriber {
    client: Client,
    config: RelayConfig,
}

impl NostrRelaySubscriber {
    pub async fn new(config: RelayConfig) -> Result<Self> {
        let keys = if let Some(private_key_hex) = &config.private_key {
            Keys::parse(private_key_hex)
                .context("Invalid private key format")?
        } else {
            Keys::generate()
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

        Ok(Self { client, config })
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
