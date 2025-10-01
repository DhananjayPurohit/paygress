// MCP (Model Context Protocol) Server for Paygress
// 
// This module provides a simple, manual implementation of the MCP server
// that bypasses the complex RMCP library issues and provides reliable
// communication with MCP clients like gateway-cli.

pub mod server;
pub mod protocol;

pub use server::MCPServer;
pub use protocol::*;
