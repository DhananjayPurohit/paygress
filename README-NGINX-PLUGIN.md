# Paygress NGINX Plugin

🔧 **Super Simple**: Rust → .so → NGINX Ingress

Just build a `.so` file and import it into NGINX. That's it!

## 🚀 Quick Start

### 1. Build the Plugin
```bash
./build-nginx.sh
```
This creates: `nginx-plugin/paygress.so`

### 2. Install in NGINX
```bash
# Copy to NGINX modules
sudo cp nginx-plugin/paygress.so /etc/nginx/modules/

# Add to nginx.conf
echo "load_module modules/paygress.so;" | sudo tee -a /etc/nginx/nginx.conf
```

### 3. Use in Your Config
```nginx
location /premium {
    access_by_lua_block {
        local ffi = require("ffi")
        ffi.cdef[[
            int paygress_verify_payment(const char* token, int amount);
        ]]
        
        local paygress = ffi.load("/etc/nginx/modules/paygress.so")
        local token = ngx.req.get_headers()["Authorization"] or ""
        
        if paygress.paygress_verify_payment(token, 1000) ~= 0 then
            ngx.exit(402)  -- Payment Required
        end
    }
    
    proxy_pass http://backend;
}
```

## 📋 Available Functions

Your `.so` exports these C functions:

```c
// Verify Cashu payment token
int paygress_verify_payment(const char* token, int amount);
// Returns: 0=success, 1=fail

// Provision Kubernetes pod  
int paygress_provision_pod(const char* namespace, const char* name);
// Returns: 0=success, 1=fail

// Get plugin version
const char* paygress_version();
// Returns: "paygress-1.0.0"
```

## 🧪 Test It

```bash
# Free content (works)
curl http://localhost/

# Premium content (fails - no token)
curl http://localhost/premium
# → 402 Payment Required

# Premium content (works - with token)
curl -H "Authorization: Bearer 1000sat-token" http://localhost/premium
# → Access granted
```

## 🎯 How It Works

1. **Build**: `cargo build --features nginx-plugin` → `libpaygress.so`
2. **Install**: Copy `.so` to `/etc/nginx/modules/`
3. **Load**: `load_module modules/paygress.so;` in nginx.conf
4. **Use**: Call functions via FFI in Lua blocks

## 📦 Kubernetes Deployment

```bash
kubectl apply -f k8s/nginx-simple.yaml
```

## 🔧 Cargo.toml Setup

```toml
[features]
nginx-plugin = ["libc"]

[dependencies]
libc = { version = "0.2", optional = true }

[lib]
crate-type = ["cdylib", "rlib"]
```

## ✨ That's It!

Super simple Rust → .so → NGINX workflow. No WASM, no complexity, just native performance! 🦀⚡
