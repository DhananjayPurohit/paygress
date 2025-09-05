// WASM modules for ingress controller plugins

#[cfg(target_arch = "wasm32")]
pub mod nginx_wasm;

#[cfg(target_arch = "wasm32")]
pub mod traefik_wasm;

#[cfg(target_arch = "wasm32")]
pub mod envoy_wasm;
