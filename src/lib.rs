// Paygress Library
//
// Exports modules for use in binaries
// Payment verification handled by ngx_l402 at nginx layer

// Module declarations
pub mod cashu;
pub mod nostr;
pub mod sidecar_service;
pub mod pod_provisioning;

// Re-export public types and functions
pub use nostr::{NostrRelaySubscriber, RelayConfig, default_relay_config, custom_relay_config};
pub use cashu::initialize_cashu;

// IngressPlugin removed - legacy code not used in current architecture
// Current architecture: nginx + ngx_l402 → PodProvisioningService
