// MCP (Model Context Protocol) Server for Paygress
// 
// This module provides a simple, manual implementation of the MCP server
// that bypasses the complex RMCP library issues and provides reliable
// communication with MCP clients like gateway-cli.
//
// This version supports L402 paywalled HTTP endpoints.

pub mod server;
pub mod protocol;
pub mod http_client;

pub use server::MCPServer;
