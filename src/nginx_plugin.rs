// NGINX Plugin with Embedded Nostr Listener
// cargo build --release --features nginx

use std::ffi::CStr;
use std::os::raw::{c_char, c_int};
use std::sync::{Arc, Mutex, Once};
use std::collections::HashMap;
use std::thread;
use tokio::runtime::Runtime;

// Global state for provisioned pods
static mut PROVISIONED_PODS: Option<Arc<Mutex<HashMap<String, PodInfo>>>> = None;
static mut NOSTR_RUNTIME: Option<Arc<Runtime>> = None;
static INIT: Once = Once::new();

#[derive(Clone, Debug)]
struct PodInfo {
    pod_id: String,
    payment_amount: u64,
    provisioned_at: u64,
}

// Initialize the plugin (called when NGINX loads the module)
#[no_mangle]
pub extern "C" fn paygress_init() -> c_int {
    INIT.call_once(|| {
        // Initialize global state
        unsafe {
            PROVISIONED_PODS = Some(Arc::new(Mutex::new(HashMap::new())));
            
            // Create Tokio runtime for async operations
            match Runtime::new() {
                Ok(rt) => {
                    let rt = Arc::new(rt);
                    NOSTR_RUNTIME = Some(rt.clone());
                    
                    // Start Nostr listener in background thread
                    thread::spawn(move || {
                        rt.block_on(async {
                            if let Err(e) = start_nostr_listener().await {
                                eprintln!("Failed to start Nostr listener: {}", e);
                            }
                        });
                    });
                }
                Err(e) => {
                    eprintln!("Failed to create Tokio runtime: {}", e);
                    return;
                }
            }
        }
    });
    0 // Success
}

// Nostr listener function (runs in background)
async fn start_nostr_listener() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use crate::nostr::{NostrRelaySubscriber, NostrEvent, default_relay_config};

    println!("🚀 Starting Nostr listener inside NGINX plugin...");

    // Create relay configuration
    let relay_config = default_relay_config();

    // Create Nostr client
    let nostr_client = NostrRelaySubscriber::new(relay_config).await?;

    // Handle incoming events
    let handler = |event: crate::nostr::NostrEvent| -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send>> {
        Box::pin(async move {
            match handle_pod_provision_event(event).await {
                Ok(pod_id) => {
                    println!("✅ Pod provisioned via Nostr: {}", pod_id);
                    Ok(())
                }
                Err(e) => {
                    eprintln!("❌ Failed to provision pod: {}", e);
                    Err(anyhow::anyhow!(e))
                }
            }
        })
    };

    // Start listening
    nostr_client.subscribe_to_pod_events(handler).await?;

    Ok(())
}

// Handle pod provisioning from Nostr event
async fn handle_pod_provision_event(event: crate::nostr::NostrEvent) -> Result<String, String> {
    // Parse event content
    let content: serde_json::Value = serde_json::from_str(&event.content)
        .map_err(|e| format!("Failed to parse event: {}", e))?;
    
    // Extract Cashu token and amount
    let cashu_token = content.get("cashu_token")
        .and_then(|v| v.as_str())
        .ok_or("Missing cashu_token")?;
    
    let amount = content.get("amount")
        .and_then(|v| v.as_u64())
        .ok_or("Missing amount")?;
    
    // Verify Cashu token
    if !verify_cashu_token_simple(cashu_token, amount) {
        return Err("Invalid Cashu token".to_string());
    }
    
    // Generate pod ID
    let pod_id = format!("pod-{:x}", rand::random::<u64>());
    
    // Provision pod (simplified - in production, call Kubernetes API)
    provision_pod_k8s(&pod_id, &content).await?;
    
    // Store pod info in global state
    unsafe {
        if let Some(ref pods) = PROVISIONED_PODS {
            let mut pods_lock = pods.lock().unwrap();
            pods_lock.insert(pod_id.clone(), PodInfo {
                pod_id: pod_id.clone(),
                payment_amount: amount,
                provisioned_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap().as_secs(),
            });
        }
    }
    
    println!("Pod {} provisioned for {} sats", pod_id, amount);
    Ok(pod_id)
}

// Simplified Cashu verification (replace with real implementation)
fn verify_cashu_token_simple(token: &str, amount: u64) -> bool {
    // For demo: token must contain the amount
    token.contains(&amount.to_string()) && !token.is_empty()
}

// Kubernetes pod provisioning (simplified)
async fn provision_pod_k8s(pod_id: &str, _spec: &serde_json::Value) -> Result<(), String> {
    // In production, this would use the Kubernetes API to create actual pods
    println!("Provisioning Kubernetes pod: {}", pod_id);
    
    // Simulate async work
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    Ok(())
}

// Payment verification for HTTP requests
#[no_mangle]
pub extern "C" fn paygress_verify_payment(token: *const c_char, amount: c_int) -> c_int {
    if token.is_null() {
        return 1; // Error: no token
    }

    unsafe {
        let token_str = match CStr::from_ptr(token).to_str() {
            Ok(s) => s,
            Err(_) => return 1, // Error: invalid token
        };

        // Verify Cashu token
        if verify_cashu_token_simple(token_str, amount as u64) {
            0 // Success: payment verified
        } else {
            1 // Error: payment failed
        }
    }
}

// Check if pod exists and is accessible
#[no_mangle]
pub extern "C" fn paygress_check_pod_access(pod_id: *const c_char) -> c_int {
    if pod_id.is_null() {
        return 1; // Error: no pod_id
    }

    unsafe {
        let pod_id_str = match CStr::from_ptr(pod_id).to_str() {
            Ok(s) => s,
            Err(_) => return 1, // Error: invalid pod_id
        };

        // Check if pod exists in our provisioned pods
        if let Some(ref pods) = PROVISIONED_PODS {
            let pods_lock = pods.lock().unwrap();
            if pods_lock.contains_key(pod_id_str) {
                0 // Success: pod exists and accessible
            } else {
                1 // Error: pod not found
            }
        } else {
            1 // Error: plugin not initialized
        }
    }
}

// Get list of provisioned pods (for debugging)
#[no_mangle]
pub extern "C" fn paygress_get_pod_count() -> c_int {
    unsafe {
        if let Some(ref pods) = PROVISIONED_PODS {
            let pods_lock = pods.lock().unwrap();
            pods_lock.len() as c_int
        } else {
            -1 // Plugin not initialized
        }
    }
}

// Get version info
#[no_mangle]
pub extern "C" fn paygress_version() -> *const c_char {
    b"paygress-1.0.0\0".as_ptr() as *const c_char
}
