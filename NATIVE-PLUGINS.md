# Paygress Native Ingress Plugins

🚀 **Native Rust plugins for NGINX, Traefik, and Envoy ingress controllers**

Transform your existing Cashu payment system into true native ingress plugins that integrate directly with popular ingress controllers.

## 🏗️ Architecture

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   NGINX Plugin  │    │  Traefik Plugin  │    │  Envoy Plugin   │
│   (.so module)  │    │  (WASM module)   │    │ (WASM module)   │
├─────────────────┤    ├──────────────────┤    ├─────────────────┤
│                 │    │                  │    │                 │
│  ┌───────────┐  │    │  ┌────────────┐  │    │ ┌─────────────┐ │
│  │ C FFI API │  │    │  │ WASM API   │  │    │ │ Proxy-WASM │ │
│  └───────────┘  │    │  └────────────┘  │    │ │    API      │ │
│                 │    │                  │    │ └─────────────┘ │
└─────────────────┘    └──────────────────┘    └─────────────────┘
         │                       │                       │
         └───────────────────────┼───────────────────────┘
                                 │
                    ┌─────────────────────┐
                    │  Paygress Core      │
                    │  - Cashu Verify     │
                    │  - K8s Integration  │
                    │  - Pod Provisioning │
                    └─────────────────────┘
```

## 🛠️ Build Commands

### Individual Plugins

```bash
# Build NGINX plugin (dynamic module)
cargo build --release --features nginx --bin nginx-plugin

# Build Traefik plugin (WASM)
cargo build --release --features traefik --bin traefik-plugin

# Build Envoy plugin (WASM)
cargo build --release --features envoy --bin envoy-plugin

# Build external service (for comparison)
cargo build --release --features service --bin paygress-service
```

### WASM Targets

```bash
# Add WASM target (first time only)
rustup target add wasm32-unknown-unknown

# Build Traefik WASM module
cargo build --target wasm32-unknown-unknown --release --features traefik --bin traefik-plugin

# Build Envoy WASM module  
cargo build --target wasm32-unknown-unknown --release --features envoy --bin envoy-plugin
```

### All Plugins

```bash
# Build everything at once
cargo build --release --features all-plugins
```

## 🔧 Integration Guides

### NGINX Plugin

**1. Build the plugin:**
```bash
cargo build --release --features nginx --bin nginx-plugin
```

**2. Copy to NGINX modules directory:**
```bash
sudo cp target/release/nginx-plugin.so /etc/nginx/modules/ngx_http_paygress_module.so
```

**3. Configure NGINX:**
```nginx
# Load the module
load_module modules/ngx_http_paygress_module.so;

http {
    server {
        listen 80;
        
        # Enable Paygress for specific location
        location /premium {
            paygress_enable on;
            paygress_amount 1000;  # 1000 sats
            proxy_pass http://backend;
        }
        
        # Free tier
        location /free {
            proxy_pass http://backend;
        }
    }
}
```

### Traefik Plugin

**1. Build WASM module:**
```bash
cargo build --target wasm32-unknown-unknown --release --features traefik --bin traefik-plugin
```

**2. Configure Traefik:**
```yaml
# traefik.yml
experimental:
  plugins:
    paygress:
      moduleName: github.com/paygress/traefik-plugin
      version: v1.0.0

http:
  middlewares:
    paygress:
      plugin:
        paygress:
          amount: 1000
          
  routers:
    premium-router:
      rule: "Path(`/premium`)"
      middlewares:
        - paygress
      service: backend
```

### Envoy Plugin

**1. Build WASM module:**
```bash
cargo build --target wasm32-unknown-unknown --release --features envoy --bin envoy-plugin
```

**2. Configure Envoy:**
```yaml
static_resources:
  listeners:
  - name: listener_0
    address:
      socket_address:
        address: 0.0.0.0
        port_value: 10000
    filter_chains:
    - filters:
      - name: envoy.filters.network.http_connection_manager
        typed_config:
          "@type": type.googleapis.com/envoy.extensions.filters.network.http_connection_manager.v3.HttpConnectionManager
          http_filters:
          - name: envoy.filters.http.wasm
            typed_config:
              "@type": type.googleapis.com/envoy.extensions.filters.http.wasm.v3.Wasm
              config:
                name: "paygress"
                root_id: "paygress_root"
                vm_config:
                  vm_id: "paygress"
                  runtime: "envoy.wasm.runtime.v8"
                  code:
                    local:
                      filename: "target/wasm32-unknown-unknown/release/envoy-plugin.wasm"
                configuration:
                  "@type": type.googleapis.com/google.protobuf.StringValue
                  value: |
                    {
                      "amount": 1000
                    }
```

## 🎯 Features

### Core Functionality
- ✅ Cashu token verification
- ✅ Kubernetes pod provisioning  
- ✅ Payment amount configuration
- ✅ Multi-ingress support
- ✅ Native performance

### Per-Plugin Features

| Feature | NGINX | Traefik | Envoy |
|---------|-------|---------|-------|
| Native Performance | ⚡ C FFI | 🚀 WASM | 🚀 WASM |
| Hot Reload | ❌ | ✅ | ✅ |
| Configuration | Static | Dynamic | Dynamic |
| Deployment | Module | Plugin | Filter |

## 🚀 Quick Start

**1. Test all plugins:**
```bash
./test-all-plugins.sh
```

**2. Run a plugin demo:**
```bash
# NGINX
./target/debug/nginx-plugin

# Traefik  
./target/debug/traefik-plugin

# Envoy
./target/debug/envoy-plugin
```

**3. Deploy to production:**
```bash
# Build optimized versions
cargo build --release --features all-plugins

# Deploy using existing Docker/K8s setup
docker-compose up -d
```

## 📦 Deployment Options

### Option 1: Native Plugins (Recommended)
- Direct integration with ingress controller
- Maximum performance
- No external dependencies

### Option 2: External Service (Fallback)
- Works with any ingress controller
- Uses external auth protocol
- Easy to deploy and debug

```bash
# Deploy external service
cargo build --release --features service --bin paygress-service
./target/release/paygress-service
```

## 🔍 Debugging

```bash
# Check plugin builds
cargo check --features nginx
cargo check --features traefik  
cargo check --features envoy

# Verbose build
cargo build -v --features nginx --bin nginx-plugin

# Test individual components
cargo test --features nginx
```

## 📚 Next Steps

1. **Choose your ingress controller** and follow the integration guide above
2. **Build the appropriate plugin** using the build commands
3. **Configure your ingress** with the provided examples
4. **Deploy and test** with real Cashu payments
5. **Monitor and scale** using your existing K8s infrastructure

For more details, see:
- [Complete README](README-COMPLETE.md) - Full project documentation
- [Simple README](README-SIMPLE.md) - Quick start guide
- [Source code](src/) - Implementation details
