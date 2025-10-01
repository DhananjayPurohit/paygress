// MCP Server binary for Pod Provisioning
use anyhow::Result;
use serde_json::Value;
use std::env;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tracing_subscriber::{self, EnvFilter};

mod mcp_server;
mod mcp_tools;

use crate::mcp_server::PodProvisioningService;

/// Main entry point for the MCP server
/// Usage: cargo run --bin paygress-mcp-server
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize the tracing subscriber with file and stdout logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    tracing::info!("Starting Paygress MCP Server");

    // Get configuration from environment (same as main.rs)
    let config = get_sidecar_config();
    
    // Validate configuration
    if config.pod_specs.is_empty() {
        eprintln!("❌ Error: No pod specifications configured");
        eprintln!("   Please provide at least one pod specification in pod-specs.json file");
        std::process::exit(1);
    }

    tracing::info!("Loaded {} pod specifications", config.pod_specs.len());
    for spec in &config.pod_specs {
        tracing::info!("  - {}: {} msats/sec ({} CPU, {} MB)", 
                      spec.name, spec.rate_msats_per_sec, spec.cpu_millicores, spec.memory_mb);
    }

    // Create the pod provisioning service
    let service = Arc::new(PodProvisioningService::new(config).await?);
    
    tracing::info!("Starting simple JSON-RPC MCP server");
    
    // Run a simple JSON-RPC server that implements MCP protocol
    run_simple_mcp_server(service).await?;
    
    tracing::info!("MCP server shutdown");
    Ok(())
}

async fn run_simple_mcp_server(service: Arc<PodProvisioningService>) -> Result<()> {
    use tokio::io::{AsyncBufReadExt, BufReader};
    use serde_json::json;

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin);
    let mut writer = stdout;

    loop {
        let mut line = String::new();
        match reader.read_line(&mut line).await {
            Ok(0) => break, // EOF
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                if let Err(e) = handle_mcp_request(&service, trimmed, &mut writer).await {
                    tracing::error!("Error handling request: {}", e);
                }
            }
            Err(e) => {
                tracing::error!("Error reading from stdin: {}", e);
                break;
            }
        }
    }

    Ok(())
}

async fn handle_mcp_request(
    service: &PodProvisioningService,
    line: &str,
    writer: &mut tokio::io::Stdout,
) -> Result<()> {
    use serde_json::json;

    let request: Value = match serde_json::from_str(line) {
        Ok(req) => req,
        Err(e) => {
            tracing::error!("Failed to parse JSON request: {}", e);
            return Ok(());
        }
    };

    let method = request["method"].as_str().unwrap_or("");
    let id = request["id"].clone();

    let response = match method {
        "initialize" => handle_initialize(&request),
        "tools/list" => handle_tools_list(&request),
        "tools/call" => handle_tools_call(service, &request).await,
        "notifications/cancelled" => {
            // This is a notification, no response needed
            tracing::info!("Received cancellation notification");
            return Ok(());
        },
        _ => {
            // Check if this is a notification (no id field) or a request
            if id.is_null() {
                tracing::info!("Received notification: {}", method);
                return Ok(());
            } else {
                tracing::error!("Unknown method: {}", method);
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {
                        "code": -32601,
                        "message": "Method not found"
                    }
                })
            }
        }
    };

    let response_str = serde_json::to_string(&response)?;
    writer.write_all(response_str.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await?;

    // Send initialized notification after initialize response
    if method == "initialize" {
        let notification = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });
        let notification_str = serde_json::to_string(&notification)?;
        writer.write_all(notification_str.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;
    }

    Ok(())
}

fn handle_initialize(request: &Value) -> Value {
    use serde_json::json;
    
    json!({
        "jsonrpc": "2.0",
        "id": request["id"],
        "result": {
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "paygress-mcp-server",
                "version": "0.1.0"
            }
        }
    })
}

fn handle_tools_list(request: &Value) -> Value {
    use serde_json::json;
    
    json!({
        "jsonrpc": "2.0",
        "id": request["id"],
        "result": {
            "tools": [
                {
                    "name": "spawn_pod",
                    "description": "Spawn a new SSH-accessible pod with Cashu payment verification",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cashu_token": {"type": "string", "description": "Valid Cashu token for payment verification"},
                            "pod_spec_id": {"type": "string", "description": "Optional pod specification ID"},
                            "pod_image": {"type": "string", "description": "Container image to use for the pod"},
                            "ssh_username": {"type": "string", "description": "SSH username for accessing the pod"},
                            "ssh_password": {"type": "string", "description": "SSH password for accessing the pod"},
                            "user_pubkey": {"type": "string", "description": "Optional user public key for identification"}
                        },
                        "required": ["cashu_token", "pod_image", "ssh_username", "ssh_password"]
                    }
                },
                {
                    "name": "topup_pod",
                    "description": "Extend the duration of an existing pod by providing additional Cashu payment",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "pod_npub": {"type": "string", "description": "Pod's NPUB identifier"},
                            "cashu_token": {"type": "string", "description": "Valid Cashu token for additional payment"}
                        },
                        "required": ["pod_npub", "cashu_token"]
                    }
                },
                {
                    "name": "list_pods",
                    "description": "List all currently active pods with their details",
                    "inputSchema": {"type": "object", "properties": {}}
                },
                {
                    "name": "get_offers",
                    "description": "Get available pod specifications and pricing information",
                    "inputSchema": {"type": "object", "properties": {}}
                }
            ]
        }
    })
}

async fn handle_tools_call(service: &PodProvisioningService, request: &Value) -> Value {
    use serde_json::json;
    
    let params = &request["params"];
    let tool_name = params["name"].as_str().unwrap_or("");
    let arguments = &params["arguments"];

    let result = match tool_name {
        "spawn_pod" => call_spawn_pod(service, arguments).await,
        "topup_pod" => call_topup_pod(service, arguments).await,
        "list_pods" => call_list_pods(service).await,
        "get_offers" => call_get_offers(service).await,
        _ => {
            return json!({
                "jsonrpc": "2.0",
                "id": request["id"],
                "error": {
                    "code": -32601,
                    "message": "Tool not found"
                }
            });
        }
    };

    json!({
        "jsonrpc": "2.0",
        "id": request["id"],
        "result": result
    })
}

async fn call_spawn_pod(service: &PodProvisioningService, arguments: &Value) -> Value {
    use serde_json::json;
    
    let cashu_token = arguments["cashu_token"].as_str().unwrap_or("");
    let pod_spec_id = arguments["pod_spec_id"].as_str();
    let pod_image = arguments["pod_image"].as_str().unwrap_or("");
    let ssh_username = arguments["ssh_username"].as_str().unwrap_or("");
    let ssh_password = arguments["ssh_password"].as_str().unwrap_or("");
    let user_pubkey = arguments["user_pubkey"].as_str();

    let request = crate::mcp_server::SpawnPodTool {
        cashu_token: cashu_token.to_string(),
        pod_spec_id: pod_spec_id.map(|s| s.to_string()),
        pod_image: pod_image.to_string(),
        ssh_username: ssh_username.to_string(),
        ssh_password: ssh_password.to_string(),
        user_pubkey: user_pubkey.map(|s| s.to_string()),
    };

    match service.spawn_pod(request).await {
        Ok(response) => {
            if response.success {
                json!({
                    "content": [
                        {
                            "type": "text",
                            "text": format!("✅ Pod created successfully!\n\n🔑 **Access Details:**\n- Pod NPUB: {}\n- SSH Host: {}\n- SSH Port: {}\n- Username: {}\n- Password: {}\n- Expires: {}\n- Spec: {}\n\n📋 **Instructions:**\n{}", 
                                response.pod_npub.as_deref().unwrap_or("N/A"),
                                response.ssh_host.as_deref().unwrap_or("N/A"),
                                response.ssh_port.map(|p| p.to_string()).as_deref().unwrap_or("N/A"),
                                response.ssh_username.as_deref().unwrap_or("N/A"),
                                response.ssh_password.as_deref().unwrap_or("N/A"),
                                response.expires_at.as_deref().unwrap_or("N/A"),
                                response.pod_spec_name.as_deref().unwrap_or("N/A"),
                                response.instructions.join("\n")
                            )
                        }
                    ],
                    "isError": false
                })
            } else {
                json!({
                    "content": [
                        {
                            "type": "text",
                            "text": format!("❌ Failed to create pod: {}\n\n📝 **Instructions:**\n{}", 
                                response.message,
                                response.instructions.join("\n")
                            )
                        }
                    ],
                    "isError": true
                })
            }
        }
        Err(e) => {
            json!({
                "content": [
                    {
                        "type": "text",
                        "text": format!("❌ Internal error spawning pod: {}", e)
                    }
                ],
                "isError": true
            })
        }
    }
}

async fn call_topup_pod(service: &PodProvisioningService, arguments: &Value) -> Value {
    use serde_json::json;
    
    let pod_npub = arguments["pod_npub"].as_str().unwrap_or("");
    let cashu_token = arguments["cashu_token"].as_str().unwrap_or("");

    let request = crate::mcp_server::TopUpPodTool {
        pod_npub: pod_npub.to_string(),
        cashu_token: cashu_token.to_string(),
    };

    match service.topup_pod(request).await {
        Ok(response) => {
            if response.success {
                json!({
                    "content": [
                        {
                            "type": "text",
                            "text": format!("✅ Pod successfully topped up!\n\n🔄 **Extension Details:**\n- Pod NPUB: {}\n- Extended Duration: {} seconds\n- New Expires At: {}\n\n📝 **Message:** {}", 
                                response.pod_npub,
                                response.extended_duration_seconds.map(|d| d.to_string()).as_deref().unwrap_or("N/A"),
                                response.new_expires_at.as_deref().unwrap_or("N/A"),
                                response.message
                            )
                        }
                    ],
                    "isError": false
                })
            } else {
                json!({
                    "content": [
                        {
                            "type": "text",
                            "text": format!("❌ Failed to top-up pod: {}", response.message)
                        }
                    ],
                    "isError": true
                })
            }
        }
        Err(e) => {
            json!({
                "content": [
                    {
                        "type": "text",
                        "text": format!("❌ Internal error topping up pod: {}", e)
                    }
                ],
                "isError": true
            })
        }
    }
}

async fn call_list_pods(service: &PodProvisioningService) -> Value {
    use serde_json::json;
    
    let request = crate::mcp_server::ListPodsTool {};
    
    match service.list_pods(request).await {
        Ok(response) => {
            if response.total_active > 0 {
                let mut pod_list = format!("📋 **Active Pods ({}):**\n\n", response.total_active);
                
                for (i, pod) in response.pods.iter().enumerate() {
                    pod_list.push_str(&format!(
                        "**{}. Pod {}**\n- NPUB: {}\n- SSH: {}@{}:{}\n- Created: {}\n- Expires: {}\n- Duration: {} seconds\n- Namespace: {}\n\n",
                        i + 1,
                        pod.pod_spec_name.as_deref().unwrap_or("Unknown"),
                        pod.pod_npub,
                        pod.ssh_username,
                        pod.ssh_host,
                        pod.ssh_port,
                        pod.created_at,
                        pod.expires_at,
                        pod.duration_seconds,
                        pod.namespace
                    ));
                }
                
                json!({
                    "content": [
                        {
                            "type": "text",
                            "text": pod_list
                        }
                    ],
                    "isError": false
                })
            } else {
                json!({
                    "content": [
                        {
                            "type": "text",
                            "text": "📭 No active pods found."
                        }
                    ],
                    "isError": false
                })
            }
        }
        Err(e) => {
            json!({
                "content": [
                    {
                        "type": "text",
                        "text": format!("❌ Internal error listing pods: {}", e)
                    }
                ],
                "isError": true
            })
        }
    }
}

async fn call_get_offers(service: &PodProvisioningService) -> Value {
    use serde_json::json;
    
    let request = crate::mcp_server::GetOffersTool {};
    
    match service.get_offers(request).await {
        Ok(response) => {
            let mut offers_text = format!(
                "🏪 **Available Pod Specifications:**\n\n⏱️ **Minimum Duration:** {} seconds\n\n💰 **Whitelisted Mints:**\n",
                response.minimum_duration_seconds
            );
            
            for mint in &response.whitelisted_mints {
                offers_text.push_str(&format!("- {}\n", mint));
            }
            
            offers_text.push_str("\n📦 **Pod Specifications:**\n\n");
            
            for (i, spec) in response.pod_specs.iter().enumerate() {
                offers_text.push_str(&format!(
                    "**{}. {} (ID: {})**\n- Description: {}\n- CPU: {} millicores\n- Memory: {} MB\n- Rate: {} msats/second\n\n",
                    i + 1,
                    spec.name,
                    spec.id,
                    spec.description,
                    spec.cpu_millicores,
                    spec.memory_mb,
                    spec.rate_msats_per_sec
                ));
            }

            json!({
                "content": [
                    {
                        "type": "text",
                        "text": offers_text
                    }
                ],
                "isError": false
            })
        }
        Err(e) => {
            json!({
                "content": [
                    {
                        "type": "text",
                        "text": format!("❌ Internal error getting offers: {}", e)
                    }
                ],
                "isError": true
            })
        }
    }
}

/// Get sidecar configuration from environment variables (same logic as main.rs)
fn get_sidecar_config() -> paygress::sidecar_service::SidecarConfig {
    use paygress::sidecar_service::SidecarConfig;

    let cashu_db_path = env::var("CASHU_DB_PATH").unwrap_or_else(|_| "./cashu.db".to_string());

    SidecarConfig {
        cashu_db_path,
        pod_namespace: env::var("POD_NAMESPACE")
            .unwrap_or_else(|_| "user-workloads".to_string()),
        minimum_pod_duration_seconds: env::var("MINIMUM_POD_DURATION_SECONDS")
            .unwrap_or_else(|_| "60".to_string())
            .parse()
            .unwrap_or(60),
        base_image: env::var("BASE_IMAGE")
            .unwrap_or_else(|_| "linuxserver/openssh-server:latest".to_string()),
        ssh_host: env::var("SSH_HOST")
            .unwrap_or_else(|_| "localhost".to_string()),
        ssh_port_range_start: env::var("SSH_PORT_RANGE_START")
            .unwrap_or_else(|_| "30000".to_string())
            .parse()
            .unwrap_or(30000),
        ssh_port_range_end: env::var("SSH_PORT_RANGE_END")
            .unwrap_or_else(|_| "31000".to_string())
            .parse()
            .unwrap_or(31000),
        enable_cleanup_task: env::var("ENABLE_CLEANUP_TASK")
            .unwrap_or_else(|_| "true".to_string())
            .parse()
            .unwrap_or(true),
        whitelisted_mints: {
            let mints_str = match env::var("WHITELISTED_MINTS") {
                Ok(mints) => mints,
                Err(_) => {
                    eprintln!("❌ Error: WHITELISTED_MINTS environment variable is required");
                    eprintln!("   Please set WHITELISTED_MINTS with comma-separated mint URLs");
                    eprintln!("   Example: WHITELISTED_MINTS=https://mint.cashu.space,https://mint.f7z.io");
                    std::process::exit(1);
                }
            };
            
            let mints: Vec<String> = mints_str
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
                
            if mints.is_empty() {
                eprintln!("❌ Error: WHITELISTED_MINTS contains no valid mint URLs");
                eprintln!("   WHITELISTED_MINTS value: {}", mints_str);
                std::process::exit(1);
            }
            
            mints
        },
        pod_specs: get_pod_specs_from_env(),
    }
}

/// Get pod specifications from JSON file (same logic as main.rs)
fn get_pod_specs_from_env() -> Vec<paygress::nostr::PodSpec> {
    use std::env;
    
    // Get the pod specs file path from environment variable
    let specs_file = env::var("POD_SPECS_FILE").unwrap_or_else(|_| "/app/pod-specs.json".to_string());
    
    // Read the JSON file
    match std::fs::read_to_string(&specs_file) {
        Ok(specs_json) => {
            match serde_json::from_str::<Vec<paygress::nostr::PodSpec>>(&specs_json) {
                Ok(specs) => {
                    if !specs.is_empty() {
                        println!("✅ Loaded {} pod specifications from {}", specs.len(), specs_file);
                        return specs;
                    } else {
                        eprintln!("❌ Error: Pod specifications file '{}' contains empty array", specs_file);
                    }
                }
                Err(e) => {
                    eprintln!("❌ Error: Failed to parse pod specifications from '{}': {}", specs_file, e);
                    eprintln!("   Please ensure the JSON file contains valid pod specifications");
                }
            }
        }
        Err(e) => {
            eprintln!("❌ Error: Failed to read pod specifications file '{}': {}", specs_file, e);
            eprintln!("   Please ensure the file exists and is readable");
            eprintln!("   You can set POD_SPECS_FILE environment variable to specify a different file path");
        }
    }
    
    eprintln!("❌ Error: No valid pod specifications found");
    eprintln!("   Expected file: {}", specs_file);
    eprintln!("   Example pod-specs.json content:");
    eprintln!(r#"   [
     {{
       "id": "basic",
       "name": "Basic",
       "description": "Basic VPS - 1 CPU core, 1GB RAM",
       "cpu_millicores": 1000,
       "memory_mb": 1024,
       "rate_msats_per_sec": 100
     }}
   ]"#);
    std::process::exit(1);
}
