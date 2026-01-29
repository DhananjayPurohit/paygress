// Paygress Library
//
// Exports modules for use in binaries
// Payment verification handled by ngx_l402 at nginx layer

// Core modules
pub mod cashu;
pub mod nostr;
pub mod sidecar_service;
pub mod pod_provisioning;

// Proxmox integration modules
pub mod proxmox;
pub mod provider;
pub mod discovery;
pub mod compute;
pub mod lxd;

// Re-export public types and functions
pub use nostr::{NostrRelaySubscriber, RelayConfig, default_relay_config, custom_relay_config};
pub use nostr::{ProviderOfferContent, HeartbeatContent, CapacityInfo, ProviderInfo, ProviderFilter, StatusRequestContent, StatusResponseContent, PrivateRequest, AccessDetailsContent, ErrorResponseContent};
pub use cashu::initialize_cashu;
pub use proxmox::ProxmoxClient;
pub use provider::{ProviderConfig, ProviderService};
pub use discovery::DiscoveryClient;
pub use compute::{ComputeBackend, ContainerConfig, NodeStatus};
pub use lxd::LxdBackend;

// Architecture notes:
// - K8s mode: nginx + ngx_l402 → PodProvisioningService
// - Proxmox mode: Nostr NIP-17 → ProviderService → ProxmoxClient
