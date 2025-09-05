# Paygress True Ingress Controller Plugins (WASM)

🦀 **Real Rust plugins that run INSIDE your ingress controllers!**

These are **true plugins** - your Rust code gets compiled to WASM and loaded directly into the ingress controller process, not external services.

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    Ingress Controller Process                   │
│  ┌─────────────────┐    ┌─────────────────┐    ┌──────────────┐ │
│  │ NGINX Core      │    │  Traefik Core   │    │ Envoy Core   │ │
│  │                 │    │                 │    │              │ │
│  │ ┌─────────────┐ │    │ ┌─────────────┐ │    │ ┌──────────┐ │ │
│  │ │ Paygress    │ │    │ │ Paygress    │ │    │ │Paygress  │ │ │
│  │ │ WASM Plugin │ │    │ │ WASM Plugin │ │    │ │WASM Plugin│ │ │
│  │ │ (Rust)      │ │    │ │ (Rust)      │ │    │ │(Rust)    │ │ │
│  │ └─────────────┘ │    │ └─────────────┘ │    │ └──────────┘ │ │
│  └─────────────────┘    └─────────────────┘    └──────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

## 🚀 Build All WASM Plugins

```bash
# Build all WASM plugins at once
./build-wasm-plugins.sh

# Or build individually:
wasm-pack build --target web --features nginx-wasm --out-dir pkg/nginx-wasm
wasm-pack build --target web --features traefik-wasm --out-dir pkg/traefik-wasm  
cargo build --target wasm32-unknown-unknown --features envoy-wasm --release
```

## 🔧 Plugin Integration

### **1. NGINX Ingress Controller WASM Plugin**

**Deploy:**
```bash
kubectl apply -f k8s/nginx-wasm-plugin.yaml
```

**How it works:**
- Your Rust code compiles to `pkg/nginx-wasm/paygress.wasm`
- Gets loaded directly into NGINX via `load_module`
- Called on every request to protected paths
- Verifies Cashu tokens and provisions pods **in-process**

**Configuration:**
```yaml
nginx.ingress.kubernetes.io/configuration-snippet: |
  access_by_wasm_call paygress paygress_auth_handler '{"amount":1000}';
```

### **2. Traefik WASM Plugin**

**Deploy:**
```bash
kubectl apply -f k8s/traefik-wasm-plugin.yaml
```

**How it works:**
- Your Rust code compiles to `pkg/traefik-wasm/paygress.wasm`
- Registered as a Traefik middleware plugin
- Processes HTTP requests before they reach your backend
- Full access to request/response pipeline

**Configuration:**
```yaml
apiVersion: traefik.containo.us/v1alpha1
kind: Middleware
metadata:
  name: paygress-wasm-auth
spec:
  plugin:
    paygress:
      amount: 1000
      enable_pod_provisioning: true
```

### **3. Envoy/Istio WASM Plugin**

**Deploy:**
```bash
kubectl apply -f k8s/envoy-wasm-plugin.yaml
```

**How it works:**
- Your Rust code compiles to `target/wasm32-unknown-unknown/release/paygress.wasm`
- Loaded as an Envoy HTTP filter via `EnvoyFilter`
- Runs in every Envoy sidecar proxy
- Integrates with Istio security policies

**Configuration:**
```yaml
apiVersion: networking.istio.io/v1alpha3
kind: EnvoyFilter
metadata:
  name: paygress-wasm-filter
spec:
  configPatches:
  - applyTo: HTTP_FILTER
    patch:
      operation: INSERT_BEFORE
      value:
        name: envoy.filters.http.wasm
```

## 🎯 Key Advantages

### **True Plugin Benefits:**
| Feature | WASM Plugin ✅ | External Service ❌ |
|---------|----------------|---------------------|
| **Performance** | Native speed | Network latency |
| **Resource Usage** | Minimal | Extra pods/memory |
| **Network Calls** | Zero | HTTP requests |
| **Failure Mode** | Fail-fast | Network timeouts |
| **Deployment** | Single binary | Multiple services |
| **Scaling** | Auto with ingress | Manual scaling |

### **Your Rust Code Runs:**
- ✅ **Inside** the ingress controller process
- ✅ **Zero network latency** for auth checks  
- ✅ **Native performance** (compiled to WASM)
- ✅ **Direct memory access** to request data
- ✅ **Crash isolation** (WASM sandbox)

## 📋 Feature Comparison

| Ingress Controller | Plugin Type | Language | Performance | Hot Reload |
|-------------------|-------------|----------|-------------|------------|
| **NGINX** | WASM Module | Rust→WASM | ⚡ Native | ❌ |
| **Traefik** | Middleware | Rust→WASM | ⚡ Native | ✅ |
| **Envoy/Istio** | HTTP Filter | Rust→WASM | ⚡ Native | ✅ |

## 🔍 How Plugins are Imported

### **NGINX Ingress Controller:**
```nginx
# Auto-generated by ingress controller
load_module modules/ngx_wasm_module.so;

wasm {
    module paygress {
        file /etc/nginx/wasm/paygress.wasm;  # ← Your Rust plugin!
    }
}

location /premium {
    access_by_wasm_call paygress paygress_auth_handler;  # ← Direct function call!
    proxy_pass http://backend;
}
```

### **Traefik:**
```yaml
# Plugin registration
experimental:
  plugins:
    paygress:
      moduleName: local-wasm-plugin
      version: v1.0.0

# Middleware usage  
middlewares:
  paygress-auth:
    plugin:
      paygress:  # ← Your Rust plugin!
        amount: 1000
```

### **Envoy/Istio:**
```yaml
# HTTP Filter configuration
http_filters:
- name: envoy.filters.http.wasm
  typed_config:
    config:
      code:
        local:
          filename: "/etc/envoy/wasm/paygress.wasm"  # ← Your Rust plugin!
```

## 🧪 Testing Your Plugins

### **Test NGINX Plugin:**
```bash
# Build and deploy
./build-wasm-plugins.sh
kubectl apply -f k8s/nginx-wasm-plugin.yaml

# Test without payment
curl http://api.example.com/premium
# → 402 Payment Required

# Test with payment
curl -H "Authorization: Bearer <cashu-token>" http://api.example.com/premium
# → Access granted + pod provisioned
```

### **Test Traefik Plugin:**
```bash
# Deploy plugin
kubectl apply -f k8s/traefik-wasm-plugin.yaml

# Test the middleware
curl -H "Authorization: Bearer <cashu-token>" http://api.example.com/premium
```

### **Test Envoy Plugin:**
```bash
# Deploy to Istio
kubectl apply -f k8s/envoy-wasm-plugin.yaml

# Test with Envoy filter
curl -H "Authorization: Bearer <cashu-token>" http://api.example.com/premium
```

## 🚀 Production Deployment

### **1. Build optimized WASM:**
```bash
# Optimize for size and speed
wasm-pack build --target web --features nginx-wasm --out-dir pkg/nginx-wasm --release
```

### **2. Update ingress controller config:**
```bash
# Load your plugin into the ingress controller
kubectl patch configmap nginx-configuration -n ingress-nginx --patch-file wasm-config.yaml
```

### **3. Monitor plugin performance:**
```bash
# Check plugin logs
kubectl logs -n ingress-nginx deployment/ingress-nginx-controller -f

# Monitor WASM performance
kubectl top pods -n ingress-nginx
```

## 🎉 You Now Have True Ingress Plugins!

Your Rust code is now **literally running inside** the ingress controller:

- ✅ **NGINX Ingress Controller** - WASM module loaded directly
- ✅ **Traefik** - Middleware plugin in the core process  
- ✅ **Envoy/Istio** - HTTP filter in every sidecar

**No external services, no network calls, just pure Rust performance!** 🦀⚡

## 📚 Next Steps

1. **Choose your ingress controller** and deploy the corresponding WASM plugin
2. **Test payment flows** with real Cashu tokens  
3. **Monitor performance** and resource usage
4. **Scale your ingress** - plugins scale automatically
5. **Update your plugin** by rebuilding and redeploying WASM
