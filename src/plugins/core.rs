// Core plugin functionality shared across all ingress controllers
// Simplified version that works with existing codebase

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

// Re-export existing functionality  
pub use crate::cashu::{initialize_cashu, verify_cashu_token};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    pub cashu_db_path: String,
    pub default_amount: i64,
    pub enable_pod_provisioning: bool,
    pub pod_namespace: String,
    pub default_pod_image: String,
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            cashu_db_path: "/tmp/cashu.db".to_string(),
            default_amount: 1000,
            enable_pod_provisioning: false,
            pod_namespace: "user-workloads".to_string(),
            default_pod_image: "nginx:alpine".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PaymentRequest {
    pub token: String,
    pub amount: Option<i64>,
    pub create_pod: bool,
    pub headers: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct PaymentResponse {
    pub verified: bool,
    pub amount: i64,
    pub pod_name: Option<String>,
    pub error: Option<String>,
    pub headers: HashMap<String, String>,
}

// Core plugin engine
pub struct PaygressCore {
    pub config: PluginConfig,
}

impl PaygressCore {
    pub fn new(config: PluginConfig) -> Result<Self, String> {
        Ok(Self { config })
    }

    pub async fn verify_payment_async(&self, request: PaymentRequest) -> PaymentResponse {
        let amount = request.amount.unwrap_or(self.config.default_amount);
        
        // Verify Cashu token using existing function
        match verify_cashu_token(&request.token, amount).await {
            Ok(true) => {
                let mut response = PaymentResponse {
                    verified: true,
                    amount,
                    pod_name: None,
                    error: None,
                    headers: HashMap::new(),
                };
                
                response.headers.insert("X-Payment-Verified".to_string(), "true".to_string());
                response.headers.insert("X-Payment-Amount".to_string(), amount.to_string());
                
                // TODO: Add pod provisioning logic here if needed
                if request.create_pod && self.config.enable_pod_provisioning {
                    // Placeholder for pod creation
                    let pod_name = format!("pod-{}", uuid::Uuid::new_v4().to_string()[..8].to_string());
                    response.pod_name = Some(pod_name.clone());
                    response.headers.insert("X-Provisioned-Pod".to_string(), pod_name);
                }
                
                response
            },
            Ok(false) => {
                PaymentResponse {
                    verified: false,
                    amount,
                    pod_name: None,
                    error: Some("Payment verification failed".to_string()),
                    headers: HashMap::from([
                        ("X-Auth-Reason".to_string(), "Payment verification failed".to_string()),
                    ]),
                }
            },
            Err(e) => {
                PaymentResponse {
                    verified: false,
                    amount,
                    pod_name: None,
                    error: Some(format!("Payment verification error: {}", e)),
                    headers: HashMap::from([
                        ("X-Auth-Reason".to_string(), "Payment verification error".to_string()),
                    ]),
                }
            }
        }
    }

    pub fn extract_payment_request(&self, headers: &HashMap<String, String>) -> Option<PaymentRequest> {
        // Extract Cashu token
        let token = self.extract_cashu_token(headers)?;
        
        // Extract other parameters
        let amount = self.extract_payment_amount(headers);
        let create_pod = self.extract_create_pod_flag(headers);
        
        Some(PaymentRequest {
            token,
            amount,
            create_pod,
            headers: headers.clone(),
        })
    }

    fn extract_cashu_token(&self, headers: &HashMap<String, String>) -> Option<String> {
        // Check Authorization header
        if let Some(auth_header) = headers.get("authorization") {
            if auth_header.starts_with("Bearer ") {
                return Some(auth_header[7..].to_string());
            }
        }

        // Check X-Cashu-Token header
        if let Some(token) = headers.get("x-cashu-token") {
            return Some(token.clone());
        }

        None
    }

    fn extract_payment_amount(&self, headers: &HashMap<String, String>) -> Option<i64> {
        if let Some(amount_str) = headers.get("x-payment-amount") {
            amount_str.parse::<i64>().ok()
        } else {
            None
        }
    }

    fn extract_create_pod_flag(&self, headers: &HashMap<String, String>) -> bool {
        headers.get("x-create-pod")
            .map(|s| s.to_lowercase() == "true")
            .unwrap_or(false)
    }
}
