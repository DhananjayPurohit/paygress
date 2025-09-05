// Traefik Middleware Plugin in Rust WASM
// This runs directly inside Traefik as a middleware

use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[derive(Deserialize)]
struct TraefikConfig {
    amount: u64,
    enable_pod_provisioning: bool,
    cashu_mint_url: String,
}

#[derive(Serialize)]
struct MiddlewareResponse {
    allowed: bool,
    status_code: u16,
    headers: std::collections::HashMap<String, String>,
    body: Option<String>,
}

// Traefik middleware entry point
#[wasm_bindgen]
pub fn paygress_middleware(
    config_json: &str,
    request_headers: &str,
    request_path: &str,
) -> String {
    log("Paygress Traefik middleware called");
    
    let config: TraefikConfig = match serde_json::from_str(config_json) {
        Ok(c) => c,
        Err(_) => {
            return create_error_response(500, "Invalid middleware configuration");
        }
    };

    let headers: std::collections::HashMap<String, String> = 
        match serde_json::from_str(request_headers) {
            Ok(h) => h,
            Err(_) => {
                return create_error_response(400, "Invalid request headers");
            }
        };

    // Get Cashu token from Authorization header
    let token = match headers.get("authorization").or_else(|| headers.get("Authorization")) {
        Some(auth) if auth.starts_with("Bearer ") => &auth[7..],
        _ => {
            log("Paygress: No valid Authorization header found");
            return create_payment_required_response(config.amount);
        }
    };

    // Verify Cashu token
    match verify_cashu_token_traefik(token, config.amount) {
        Ok(true) => {
            log("Paygress: Payment verified successfully");
            
            let mut response_headers = std::collections::HashMap::new();
            response_headers.insert("X-Payment-Verified".to_string(), "true".to_string());
            response_headers.insert("X-Payment-Amount".to_string(), config.amount.to_string());
            
            // Provision pod if enabled
            if config.enable_pod_provisioning {
                match provision_pod_traefik(request_path) {
                    Ok(pod_name) => {
                        log(&format!("Paygress: Pod provisioned: {}", pod_name));
                        response_headers.insert("X-Provisioned-Pod".to_string(), pod_name);
                    }
                    Err(e) => {
                        log(&format!("Paygress: Pod provisioning failed: {}", e));
                    }
                }
            }

            serde_json::to_string(&MiddlewareResponse {
                allowed: true,
                status_code: 200,
                headers: response_headers,
                body: None,
            }).unwrap()
        }
        Ok(false) => {
            log("Paygress: Invalid Cashu token");
            create_payment_required_response(config.amount)
        }
        Err(e) => {
            log(&format!("Paygress: Token verification error: {}", e));
            create_error_response(500, "Payment verification failed")
        }
    }
}

fn create_payment_required_response(amount: u64) -> String {
    let mut headers = std::collections::HashMap::new();
    headers.insert("Content-Type".to_string(), "application/json".to_string());
    headers.insert("X-Payment-Required".to_string(), amount.to_string());
    
    let body = serde_json::json!({
        "error": "Payment Required",
        "message": "Send valid Cashu token in Authorization header",
        "amount": amount,
        "currency": "satoshis"
    });

    serde_json::to_string(&MiddlewareResponse {
        allowed: false,
        status_code: 402,
        headers,
        body: Some(body.to_string()),
    }).unwrap()
}

fn create_error_response(status: u16, message: &str) -> String {
    let mut headers = std::collections::HashMap::new();
    headers.insert("Content-Type".to_string(), "application/json".to_string());
    
    let body = serde_json::json!({
        "error": "Plugin Error",
        "message": message
    });

    serde_json::to_string(&MiddlewareResponse {
        allowed: false,
        status_code: status,
        headers,
        body: Some(body.to_string()),
    }).unwrap()
}

fn verify_cashu_token_traefik(token: &str, amount: u64) -> Result<bool, String> {
    // Simplified Cashu verification for WASM
    if token.is_empty() {
        return Ok(false);
    }
    
    // In production, implement full Cashu verification
    if token.len() > 10 && token.contains(&amount.to_string()) {
        Ok(true)
    } else {
        Ok(false)
    }
}

fn provision_pod_traefik(request_path: &str) -> Result<String, String> {
    // Generate pod name based on request path
    let pod_name = format!("traefik-pod-{}-{}", 
        request_path.replace('/', "-").trim_matches('-'),
        js_sys::Date::now() as u64
    );
    
    log(&format!("Paygress: Simulating pod creation for Traefik: {}", pod_name));
    Ok(pod_name)
}

#[wasm_bindgen(start)]
pub fn traefik_init() {
    log("Paygress Traefik WASM Plugin initialized");
}
