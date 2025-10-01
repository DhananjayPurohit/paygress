// Unified Paygress Service Interfaces
//
// This module contains all the interface implementations (Nostr, MCP, HTTP)
// that share a single PodProvisioningService instance to prevent race conditions.

pub mod nostr;
pub mod mcp;
pub mod http;

use anyhow::Result;
use std::sync::Arc;
use tracing::{info, error};

use crate::pod_provisioning::PodProvisioningService;

/// Run all enabled interfaces concurrently
pub async fn run_all_interfaces(service: Arc<PodProvisioningService>) -> Result<()> {
    let mut tasks = Vec::new();

    // Check which interfaces are enabled via environment variables
    if is_interface_enabled("NOSTR") {
        info!("Starting Nostr interface...");
        let nostr_service = Arc::clone(&service);
        tasks.push(tokio::spawn(async move {
            nostr::run_nostr_interface(nostr_service).await
        }));
    }

    if is_interface_enabled("MCP") {
        info!("Starting MCP interface...");
        let mcp_service = Arc::clone(&service);
        tasks.push(tokio::spawn(async move {
            mcp::run_mcp_interface(mcp_service).await
        }));
    }

    if is_interface_enabled("HTTP") {
        info!("Starting HTTP interface...");
        let http_service = Arc::clone(&service);
        tasks.push(tokio::spawn(async move {
            http::run_http_interface(http_service).await
        }));
    }

    if tasks.is_empty() {
        error!("No interfaces enabled! Set ENABLE_NOSTR, ENABLE_MCP, or ENABLE_HTTP environment variables.");
        return Err(anyhow::anyhow!("No interfaces enabled"));
    }

    info!("Running {} interface(s) concurrently", tasks.len());

    // Wait for all interfaces to complete (they should run forever)
    tokio::select! {
        result = async {
            for task in tasks {
                if let Err(e) = task.await {
                    error!("Interface task failed: {}", e);
                }
            }
        } => {
            info!("All interfaces stopped");
        }
    }

    Ok(())
}

/// Check if an interface is enabled via environment variable
fn is_interface_enabled(interface: &str) -> bool {
    let env_var = format!("ENABLE_{}", interface);
    std::env::var(&env_var)
        .unwrap_or_else(|_| "true".to_string()) // Default to enabled
        .to_lowercase() == "true"
}

/// Get interface-specific configuration
pub fn get_interface_config() -> InterfaceConfig {
    InterfaceConfig {
        nostr_enabled: is_interface_enabled("NOSTR"),
        mcp_enabled: is_interface_enabled("MCP"),
        http_enabled: is_interface_enabled("HTTP"),
        http_port: std::env::var("HTTP_PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse()
            .unwrap_or(8080),
        http_bind_addr: std::env::var("HTTP_BIND_ADDR")
            .unwrap_or_else(|_| "0.0.0.0:8080".to_string()),
    }
}

#[derive(Debug, Clone)]
pub struct InterfaceConfig {
    pub nostr_enabled: bool,
    pub mcp_enabled: bool,
    pub http_enabled: bool,
    pub http_port: u16,
    pub http_bind_addr: String,
}
