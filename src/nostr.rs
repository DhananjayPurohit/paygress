use anyhow::{Context, Result};
use nostr_sdk::{Client, Keys, Filter, Kind, RelayPoolNotification, Url};
// Serde not needed in this module
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use tracing::{error, info, warn};

use crate::NostrEvent;

pub struct NostrRelaySubscriber {
    client: Client,
}

#[derive(Debug, Clone)]
pub struct RelayConfig {
    pub urls: Vec<String>,
    pub secret_key: Option<String>, // Optional: provide existing key or generate new one
}

// Type alias for event handler function
pub type EventHandler = Box<dyn Fn(NostrEvent) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> + Send + Sync>;

impl NostrRelaySubscriber {
    pub async fn new(config: RelayConfig) -> Result<Self> {
        // Create or load keys
        let keys = if let Some(sk) = config.secret_key {
            Keys::parse(&sk).context("Invalid secret key format")?
        } else {
            Keys::generate()
        };

        info!("Nostr public key: {}", keys.public_key());

        // Create client
        let client = Client::new(keys);

        // Add relays
        for url_str in &config.urls {
            let url = Url::parse(url_str).context(format!("Invalid relay URL: {}", url_str))?;
            client.add_relay(url).await
                .context(format!("Failed to add relay: {}", url_str))?;
            info!("Added relay: {}", url_str);
        }

        // Connect to relays
        client.connect().await;
        info!("Connected to {} relays", config.urls.len());

        Ok(Self { client })
    }

    pub async fn subscribe_to_pod_events<F>(&self, handler: F) -> Result<()> 
    where
        F: Fn(NostrEvent) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> + Send + Sync
    {
        // Create filter for pod provisioning events (kind 1000)
        let filter = Filter::new()
            .kind(Kind::Custom(1000));

        self.subscribe_with_filter_and_handler(filter, handler).await
    }

    pub async fn subscribe_with_filter_and_handler<F>(&self, filter: Filter, handler: F) -> Result<()>
    where
        F: Fn(NostrEvent) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> + Send + Sync
    {
        info!("Subscribing with filter: {:?}", filter);

        // Subscribe to events
        self.client.subscribe(vec![filter], None).await
            .context("Failed to subscribe to events")?;

        // Listen for notifications
        let mut notifications = self.client.notifications();

        info!("Listening for Nostr events...");

        while let Ok(notification) = notifications.recv().await {
            match notification {
                RelayPoolNotification::Event { relay_url, event, .. } => {
                    info!("Received event {} from relay: {}", event.id, relay_url);
                    
                    // Convert nostr-sdk Event to your NostrEvent struct
                    let custom_event = self.convert_event(&event);
                    
                    // Call the provided handler
                    match handler(custom_event).await {
                        Ok(()) => {
                            info!("Successfully processed event {}", event.id);
                        },
                        Err(e) => {
                            error!("Failed to process event {}: {}", event.id, e);
                        }
                    }
                },
                RelayPoolNotification::Message { relay_url, message } => {
                    info!("Message from {}: {:?}", relay_url, message);
                },
                RelayPoolNotification::Shutdown => {
                    warn!("Relay pool shutdown");
                    break;
                },
                _ => {
                    // Handle other notification types if needed
                }
            }
        }

        Ok(())
    }

    // Simple method for custom filters without handler - just logs events
    pub async fn subscribe_and_log(&self, filter: Filter) -> Result<()> {
        info!("Subscribing and logging with filter: {:?}", filter);

        self.client.subscribe(vec![filter], None).await
            .context("Failed to subscribe with custom filter")?;

        let mut notifications = self.client.notifications();

        while let Ok(notification) = notifications.recv().await {
            match notification {
                RelayPoolNotification::Event { relay_url, event, .. } => {
                    let custom_event = self.convert_event(&event);
                    info!("Received event {} from relay {}: content={}", 
                        custom_event.id, relay_url, custom_event.content);
                },
                RelayPoolNotification::Shutdown => break,
                _ => {}
            }
        }

        Ok(())
    }

    pub async fn disconnect(&self) {
        self.client.disconnect().await
            .unwrap_or_else(|e| error!("Error disconnecting: {}", e));
        info!("Disconnected from all relays");
    }

    pub fn get_public_key(&self) -> String {
        // For nostr-sdk 0.33, we need to access the keys differently
        // This is a workaround since the API changed
        "public_key_not_accessible_in_0.33".to_string()
    }

    pub async fn get_relay_status(&self) -> HashMap<String, String> {
        // This would require access to relay pool status
        // For now, return a placeholder
        HashMap::new()
    }

    fn convert_event(&self, event: &nostr_sdk::Event) -> NostrEvent {
        NostrEvent {
            id: event.id.to_string(),
            pubkey: event.pubkey.to_string(),
            created_at: event.created_at.as_u64(),
            kind: event.kind.as_u32(),
            tags: event.tags.iter().map(|tag| tag.as_vec().to_vec()).collect(),
            content: event.content.clone(),
            sig: event.sig.to_string(),
        }
    }
}

// Helper function to create default relay configuration
pub fn default_relay_config() -> RelayConfig {
    RelayConfig {
        urls: vec![
            "wss://relay.damus.io".to_string(),
            "wss://nos.lol".to_string(),
            "wss://relay.nostr.band".to_string(),
        ],
        secret_key: None,
    }
}

// Helper function to create relay configuration with custom relays
pub fn custom_relay_config(
    relay_urls: Vec<String>, 
    secret_key: Option<String>
) -> RelayConfig {
    RelayConfig {
        urls: relay_urls,
        secret_key,
    }
}
