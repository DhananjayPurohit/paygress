// MCP Protocol Implementation
// 
// This module contains the data structures and protocol handlers
// for the Model Context Protocol (MCP) server implementation.

use serde_json::Value;
use crate::pod_provisioning::PodProvisioningService;
use crate::mcp::http_client::{PaywalledHttpClient, SpawnPodRequest};

/// Handle MCP initialization request
pub fn handle_initialize(request: &Value) -> Value {
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
                "name": "Paygress",
                "version": "0.1.0"
            },
            "instructions": "Paygress - Lightning-powered VM provisioning service with complete pay-as-you-go model using Cashu payments",
            "tags": [
                ["name", "Paygress"], // Optional: Human-readable server name
                ["about", "Lightning-powered VM provisioning service with complete pay-as-you-go model using Cashu payments"], // Optional: Server description
            ]
        }
    })
}

/// Handle MCP tools/list request
pub fn handle_tools_list(request: &Value) -> Value {
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
                    "name": "get_offers",
                    "description": "Get available pod specifications and pricing information",
                    "inputSchema": {"type": "object", "properties": {}}
                },
                {
                    "name": "get_pod_status",
                    "description": "Get pod status, time remaining, and specifications by NPUB",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "pod_npub": {"type": "string", "description": "Pod's NPUB identifier"}
                        },
                        "required": ["pod_npub"]
                    }
                }
            ]
        }
    })
}

/// Handle MCP tools/call request
pub async fn handle_tools_call(service: &PodProvisioningService, request: &Value) -> Value {
    use serde_json::json;
    
    let params = &request["params"];
    let tool_name = params["name"].as_str().unwrap_or("");
    let arguments = &params["arguments"];

    let result = match tool_name {
        "spawn_pod" => call_spawn_pod(service, arguments).await,
        "topup_pod" => call_topup_pod(service, arguments).await,
        "get_offers" => call_get_offers(service).await,
        "get_pod_status" => call_get_pod_status(service, arguments).await,
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

/// Call spawn_pod tool
async fn call_spawn_pod(service: &PodProvisioningService, arguments: &Value) -> Value {
    use serde_json::json;
    
    let cashu_token = arguments["cashu_token"].as_str().unwrap_or("");
    let pod_spec_id = arguments["pod_spec_id"].as_str();
    let pod_image = arguments["pod_image"].as_str().unwrap_or("");
    let ssh_username = arguments["ssh_username"].as_str().unwrap_or("");
    let ssh_password = arguments["ssh_password"].as_str().unwrap_or("");
    let user_pubkey = arguments["user_pubkey"].as_str();

    let request = crate::pod_provisioning::SpawnPodTool {
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
                            "text": format!("✅ Pod created successfully!\n\n🔑 **Access Details:**\n- SSH Host: {}\n- SSH Port: {}\n- Username: {}\n- Password: {}\n- Expires: {}\n- Spec: {}\n\n📋 **Instructions:**\n{}", 
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

/// Call topup_pod tool
async fn call_topup_pod(service: &PodProvisioningService, arguments: &Value) -> Value {
    use serde_json::json;
    
    let pod_npub = arguments["pod_npub"].as_str().unwrap_or("");
    let cashu_token = arguments["cashu_token"].as_str().unwrap_or("");

    let request = crate::pod_provisioning::TopUpPodTool {
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


/// Call get_offers tool
async fn call_get_offers(service: &PodProvisioningService) -> Value {
    use serde_json::json;
    
    let request = crate::pod_provisioning::GetOffersTool {};
    
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

/// Call get_pod_status tool
async fn call_get_pod_status(service: &PodProvisioningService, arguments: &Value) -> Value {
    use serde_json::json;
    
    let pod_npub = arguments["pod_npub"].as_str().unwrap_or("");

    let request = crate::pod_provisioning::GetPodStatusTool {
        pod_npub: pod_npub.to_string(),
    };
    
    match service.get_pod_status(request).await {
        Ok(response) => {
            if response.found {
                let status_text = if let Some(time_remaining) = response.time_remaining_seconds {
                    if time_remaining > 0 {
                        let hours = time_remaining / 3600;
                        let minutes = (time_remaining % 3600) / 60;
                        let seconds = time_remaining % 60;
                        
                        format!(
                            "📊 **Pod Status for {}**\n\n✅ **Status:** {}\n⏰ **Time Remaining:** {}h {}m {}s\n📅 **Created:** {}\n📅 **Expires:** {}\n\n⚙️ **Specifications:**\n- CPU: {} millicores\n- Memory: {} MB\n- Spec: {}",
                            response.pod_npub,
                            response.status.as_deref().unwrap_or("unknown"),
                            hours, minutes, seconds,
                            response.created_at.as_deref().unwrap_or("N/A"),
                            response.expires_at.as_deref().unwrap_or("N/A"),
                            response.cpu_millicores.map(|c| c.to_string()).as_deref().unwrap_or("N/A"),
                            response.memory_mb.map(|m| m.to_string()).as_deref().unwrap_or("N/A"),
                            response.pod_spec_name.as_deref().unwrap_or("N/A")
                        )
                    } else {
                        format!(
                            "⏰ **Pod Status for {}**\n\n❌ **Status:** Expired\n📅 **Created:** {}\n📅 **Expired:** {}\n\nThis pod has expired and is no longer accessible.",
                            response.pod_npub,
                            response.created_at.as_deref().unwrap_or("N/A"),
                            response.expires_at.as_deref().unwrap_or("N/A")
                        )
                    }
                } else {
                    format!(
                        "📊 **Pod Status for {}**\n\n⚠️ **Status:** Unknown\n📅 **Created:** {}\n\nPod exists but status is unclear.",
                        response.pod_npub,
                        response.created_at.as_deref().unwrap_or("N/A")
                    )
                };
                
                json!({
                    "content": [
                        {
                            "type": "text",
                            "text": status_text
                        }
                    ],
                    "isError": false
                })
            } else {
                json!({
                    "content": [
                        {
                            "type": "text",
                            "text": format!("❌ **Pod Not Found**\n\nPod with NPUB `{}` was not found. It may have expired, been deleted, or the NPUB is incorrect.", response.pod_npub)
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
                        "text": format!("❌ Internal error getting pod status: {}", e)
                    }
                ],
                "isError": true
            })
        }
    }
}

/// Handle MCP tools/call request using HTTP client (with L402 paywall support)
pub async fn handle_tools_call_http(http_client: &PaywalledHttpClient, request: &Value) -> Value {
    use serde_json::json;
    use tracing::{info, error};
    
    let params = &request["params"];
    let tool_name = params["name"].as_str().unwrap_or("");
    let arguments = &params["arguments"];

    info!("🔧 MCP tool call (via HTTP): {}", tool_name);

    let result = match tool_name {
        "spawn_pod" => {
            let spawn_request = SpawnPodRequest {
                cashu_token: arguments["cashu_token"].as_str().unwrap_or("").to_string(),
                pod_spec_id: arguments["pod_spec_id"].as_str().map(|s| s.to_string()),
                pod_image: arguments["pod_image"].as_str().unwrap_or("").to_string(),
                ssh_username: arguments["ssh_username"].as_str().unwrap_or("").to_string(),
                ssh_password: arguments["ssh_password"].as_str().unwrap_or("").to_string(),
                user_pubkey: arguments["user_pubkey"].as_str().map(|s| s.to_string()),
            };

            match http_client.spawn_pod(spawn_request).await {
                Ok(response) => {
                    if response["success"].as_bool().unwrap_or(false) {
                        let pod_npub = response["pod_npub"].as_str().unwrap_or("N/A");
                        let ssh_host = response["ssh_host"].as_str().unwrap_or("N/A");
                        let ssh_port = response["ssh_port"].as_u64().unwrap_or(0);
                        let ssh_username = response["ssh_username"].as_str().unwrap_or("N/A");
                        let ssh_password = response["ssh_password"].as_str().unwrap_or("N/A");
                        let expires_at = response["expires_at"].as_str().unwrap_or("N/A");
                        let pod_spec_name = response["pod_spec_name"].as_str().unwrap_or("N/A");
                        let instructions = response["instructions"].as_str().unwrap_or("N/A");

                        json!({
                            "content": [
                                {
                                    "type": "text",
                                    "text": format!(
                                        "✅ **Pod Spawned Successfully!**\n\n\
                                        **Pod Details:**\n\
                                        - **Pod ID (NPUB):** `{}`\n\
                                        - **Spec:** {}\n\
                                        - **Expires:** {}\n\n\
                                        **SSH Access:**\n\
                                        ```bash\n\
                                        ssh {}@{} -p {}\n\
                                        Password: {}\n\
                                        ```\n\n\
                                        {}",
                                        pod_npub, pod_spec_name, expires_at,
                                        ssh_username, ssh_host, ssh_port, ssh_password, instructions
                                    )
                                }
                            ],
                            "isError": false
                        })
                    } else {
                        let message = response["message"].as_str().unwrap_or("Unknown error");
                        json!({
                            "content": [
                                {
                                    "type": "text",
                                    "text": format!("❌ Failed to spawn pod: {}", message)
                                }
                            ],
                            "isError": true
                        })
                    }
                }
                Err(e) => {
                    error!("Failed to spawn pod via HTTP: {}", e);
                    json!({
                        "content": [
                            {
                                "type": "text",
                                "text": format!("❌ HTTP request failed: {}\n\nThis might be due to L402 payment requirement. Check logs for details.", e)
                            }
                        ],
                        "isError": true
                    })
                }
            }
        }
        "topup_pod" => {
            let pod_npub = arguments["pod_npub"].as_str().unwrap_or("").to_string();
            let cashu_token = arguments["cashu_token"].as_str().unwrap_or("").to_string();

            match http_client.topup_pod(pod_npub, cashu_token).await {
                Ok(response) => {
                    if response["success"].as_bool().unwrap_or(false) {
                        let extended_duration = response["extended_duration_seconds"].as_u64().unwrap_or(0);
                        let new_expires_at = response["new_expires_at"].as_str().unwrap_or("N/A");

                        json!({
                            "content": [
                                {
                                    "type": "text",
                                    "text": format!(
                                        "✅ **Pod Topped Up Successfully!**\n\n\
                                        - **Extended by:** {} seconds\n\
                                        - **New expiration:** {}",
                                        extended_duration, new_expires_at
                                    )
                                }
                            ],
                            "isError": false
                        })
                    } else {
                        let message = response["message"].as_str().unwrap_or("Unknown error");
                        json!({
                            "content": [
                                {
                                    "type": "text",
                                    "text": format!("❌ Failed to topup pod: {}", message)
                                }
                            ],
                            "isError": true
                        })
                    }
                }
                Err(e) => {
                    error!("Failed to topup pod via HTTP: {}", e);
                    json!({
                        "content": [
                            {
                                "type": "text",
                                "text": format!("❌ HTTP request failed: {}", e)
                            }
                        ],
                        "isError": true
                    })
                }
            }
        }
        "get_offers" => {
            match http_client.get_offers().await {
                Ok(response) => {
                    let min_duration = response["minimum_duration_seconds"].as_u64().unwrap_or(0);
                    let whitelisted_mints = response["whitelisted_mints"].as_array()
                        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(", "))
                        .unwrap_or_else(|| "None".to_string());
                    
                    let empty_vec = vec![];
                    let pod_specs = response["pod_specs"].as_array().unwrap_or(&empty_vec);
                    let mut specs_text = String::new();
                    
                    for spec in pod_specs {
                        let name = spec["name"].as_str().unwrap_or("Unknown");
                        let cpu = spec["cpu_millicores"].as_u64().unwrap_or(0);
                        let memory = spec["memory_mb"].as_u64().unwrap_or(0);
                        let rate = spec["rate_msats_per_sec"].as_u64().unwrap_or(0);
                        let description = spec["description"].as_str().unwrap_or("");
                        
                        specs_text.push_str(&format!(
                            "\n### {}\n{}\n- **CPU:** {} millicores\n- **Memory:** {} MB\n- **Rate:** {} msats/sec\n",
                            name, description, cpu, memory, rate
                        ));
                    }

                    json!({
                        "content": [
                            {
                                "type": "text",
                                "text": format!(
                                    "📦 **Available Pod Offerings**\n\n\
                                    **Minimum Duration:** {} seconds\n\
                                    **Whitelisted Mints:** {}\n\
                                    {}", 
                                    min_duration, whitelisted_mints, specs_text
                                )
                            }
                        ],
                        "isError": false
                    })
                }
                Err(e) => {
                    error!("Failed to get offers via HTTP: {}", e);
                    json!({
                        "content": [
                            {
                                "type": "text",
                                "text": format!("❌ HTTP request failed: {}", e)
                            }
                        ],
                        "isError": true
                    })
                }
            }
        }
        "get_pod_status" => {
            let pod_npub = arguments["pod_npub"].as_str().unwrap_or("").to_string();

            match http_client.get_pod_status(pod_npub).await {
                Ok(response) => {
                    if response["found"].as_bool().unwrap_or(false) {
                        let pod_npub = response["pod_npub"].as_str().unwrap_or("N/A");
                        let status = response["status"].as_str().unwrap_or("unknown");
                        let time_remaining = response["time_remaining_seconds"].as_i64().unwrap_or(0);
                        let expires_at = response["expires_at"].as_str().unwrap_or("N/A");
                        let pod_spec_name = response["pod_spec_name"].as_str().unwrap_or("N/A");
                        let cpu = response["cpu_millicores"].as_u64().unwrap_or(0);
                        let memory = response["memory_mb"].as_u64().unwrap_or(0);

                        json!({
                            "content": [
                                {
                                    "type": "text",
                                    "text": format!(
                                        "📊 **Pod Status**\n\n\
                                        - **Pod NPUB:** `{}`\n\
                                        - **Status:** {}\n\
                                        - **Spec:** {}\n\
                                        - **Resources:** {} millicores CPU, {} MB RAM\n\
                                        - **Time Remaining:** {} seconds\n\
                                        - **Expires At:** {}",
                                        pod_npub, status, pod_spec_name, cpu, memory, time_remaining, expires_at
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
                                    "text": "❌ Pod not found or expired"
                                }
                            ],
                            "isError": false
                        })
                    }
                }
                Err(e) => {
                    error!("Failed to get pod status via HTTP: {}", e);
                    json!({
                        "content": [
                            {
                                "type": "text",
                                "text": format!("❌ HTTP request failed: {}", e)
                            }
                        ],
                        "isError": true
                    })
                }
            }
        }
        _ => {
            json!({
                "content": [
                    {
                        "type": "text",
                        "text": format!("❌ Unknown tool: {}", tool_name)
                    }
                ],
                "isError": true
            })
        }
    };

    json!({
        "jsonrpc": "2.0",
        "id": request["id"],
        "result": result
    })
}
