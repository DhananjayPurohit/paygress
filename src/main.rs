use paygress::{
    nginx_auth::start_nginx_auth_service,
    complete_plugin::{start_complete_plugin, PluginConfig},
};
use std::env;
use tracing_subscriber::fmt::init;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    init();

    // Get configuration from environment
    let mode = env::var("PAYGRESS_MODE").unwrap_or_else(|_| "simple".to_string());
    let bind_addr = env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
    let cashu_db_path = env::var("CASHU_DB_PATH").unwrap_or_else(|_| "./cashu.db".to_string());

    match mode.as_str() {
        "simple" => {
            // Simple NGINX auth only
            start_nginx_auth_service(&bind_addr, &cashu_db_path).await?;
        },
        "complete" => {
            // Complete ingress plugin with all features
            let config = PluginConfig {
                cashu_db_path,
                enable_pod_provisioning: env::var("ENABLE_POD_PROVISIONING")
                    .unwrap_or_else(|_| "true".to_string())
                    .parse()
                    .unwrap_or(true),
                enable_nostr_events: env::var("ENABLE_NOSTR_EVENTS")
                    .unwrap_or_else(|_| "false".to_string())
                    .parse()
                    .unwrap_or(false),
                default_pod_image: env::var("DEFAULT_POD_IMAGE")
                    .unwrap_or_else(|_| "nginx:alpine".to_string()),
                pod_namespace: env::var("POD_NAMESPACE")
                    .unwrap_or_else(|_| "default".to_string()),
                nostr_relays: env::var("NOSTR_RELAYS")
                    .unwrap_or_else(|_| "wss://relay.damus.io".to_string())
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .collect(),
                nostr_secret_key: env::var("NOSTR_SECRET_KEY").ok(),
            };
            
            start_complete_plugin(&bind_addr, config).await?;
        },
        _ => {
            eprintln!("❌ Invalid mode: {}. Use 'simple' or 'complete'", mode);
            std::process::exit(1);
        }
    }

    Ok(())
}