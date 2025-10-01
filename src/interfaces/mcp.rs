// MCP Interface for Paygress
//
// Handles Model Context Protocol (MCP) requests for Context VM integration
// using a shared PodProvisioningService instance.

use anyhow::Result;
use std::sync::Arc;
use tracing::{info, error};

use crate::pod_provisioning::PodProvisioningService;
use crate::mcp::MCPServer;

/// Run the MCP interface
pub async fn run_mcp_interface(service: Arc<PodProvisioningService>) -> Result<()> {
    info!("🤖 Starting MCP interface...");

    // Create and run the MCP server
    let mcp_server = MCPServer::new(service);
    
    info!("✅ MCP interface ready - listening on stdio transport");
    
    // Run the MCP server (this blocks forever)
    if let Err(e) = mcp_server.run().await {
        error!("❌ MCP interface error: {}", e);
        return Err(e);
    }

    Ok(())
}
