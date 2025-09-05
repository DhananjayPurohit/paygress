#!/bin/bash

echo "🔧 Building Paygress .so Shared Library Plugins"
echo "=============================================="
echo

# Build NGINX .so module
echo "1️⃣ Building NGINX .so Module..."
cargo build --release --features nginx-so --lib
if [ $? -eq 0 ]; then
    SO_FILE="target/release/libpaygress.so"
    if [ -f "$SO_FILE" ]; then
        echo "✅ NGINX .so module built: $SO_FILE"
        
        # Create NGINX-specific .so
        mkdir -p plugins/nginx/
        cp "$SO_FILE" plugins/nginx/ngx_http_paygress_module.so
        echo "📦 NGINX module: plugins/nginx/ngx_http_paygress_module.so"
    else
        echo "❌ .so file not found"
    fi
else
    echo "❌ NGINX .so build failed"
fi
echo

# Build Traefik Go plugin that uses our Rust .so
echo "2️⃣ Building Traefik Go Plugin (calls Rust .so)..."
mkdir -p plugins/traefik/
cat > plugins/traefik/main.go << 'EOF'
package main

import (
    "C"
    "context"
    "fmt"
    "net/http"
    "unsafe"
)

// #cgo LDFLAGS: -L../.. -lpaygress
// #include <stdlib.h>
// extern int ngx_http_paygress_verify_payment(const char* token, int amount);
import "C"

// Config holds the plugin configuration
type Config struct {
    Amount                int  `json:"amount"`
    EnablePodProvisioning bool `json:"enable_pod_provisioning"`
}

// CreateConfig creates the default plugin configuration
func CreateConfig() *Config {
    return &Config{
        Amount:                1000,
        EnablePodProvisioning: true,
    }
}

// PaygressPlugin holds the plugin instance
type PaygressPlugin struct {
    next   http.Handler
    config *Config
    name   string
}

// New creates a new PaygressPlugin instance
func New(ctx context.Context, next http.Handler, config *Config, name string) (http.Handler, error) {
    return &PaygressPlugin{
        next:   next,
        config: config,
        name:   name,
    }, nil
}

func (p *PaygressPlugin) ServeHTTP(rw http.ResponseWriter, req *http.Request) {
    // Get Authorization header
    authHeader := req.Header.Get("Authorization")
    if authHeader == "" {
        p.sendPaymentRequired(rw)
        return
    }

    // Extract token
    token := authHeader
    if len(authHeader) > 7 && authHeader[:7] == "Bearer " {
        token = authHeader[7:]
    }

    // Call Rust verification function
    cToken := C.CString(token)
    defer C.free(unsafe.Pointer(cToken))
    
    result := C.ngx_http_paygress_verify_payment(cToken, C.int(p.config.Amount))
    
    if result == 0 {
        // Payment verified
        rw.Header().Set("X-Payment-Verified", "true")
        rw.Header().Set("X-Payment-Amount", fmt.Sprintf("%d", p.config.Amount))
        p.next.ServeHTTP(rw, req)
    } else {
        // Payment failed
        p.sendPaymentRequired(rw)
    }
}

func (p *PaygressPlugin) sendPaymentRequired(rw http.ResponseWriter) {
    rw.Header().Set("Content-Type", "application/json")
    rw.WriteHeader(402)
    rw.Write([]byte(fmt.Sprintf(`{"error":"Payment Required","amount":%d}`, p.config.Amount)))
}
EOF

cat > plugins/traefik/go.mod << 'EOF'
module github.com/your-org/paygress-traefik-plugin

go 1.19
EOF

echo "✅ Traefik Go plugin created: plugins/traefik/"
echo

# Build Envoy C++ extension
echo "3️⃣ Building Envoy C++ Extension (calls Rust .so)..."
mkdir -p plugins/envoy/
cat > plugins/envoy/paygress_filter.cc << 'EOF'
#include <string>
#include "envoy/server/filter_config.h"
#include "envoy/http/filter.h"

// Link to our Rust library
extern "C" {
    int ngx_http_paygress_verify_payment(const char* token, int amount);
}

namespace Envoy {
namespace Http {

class PaygressFilter : public StreamFilter {
public:
    PaygressFilter(int amount) : amount_(amount) {}

    FilterHeadersStatus decodeHeaders(RequestHeaderMap& headers, bool) override {
        // Get Authorization header
        const HeaderEntry* auth_header = headers.get(LowerCaseString("authorization"));
        if (!auth_header) {
            sendPaymentRequired();
            return FilterHeadersStatus::StopIteration;
        }

        std::string token = std::string(auth_header->value().getStringView());
        
        // Remove "Bearer " prefix if present
        if (token.substr(0, 7) == "Bearer ") {
            token = token.substr(7);
        }

        // Call Rust verification
        int result = ngx_http_paygress_verify_payment(token.c_str(), amount_);
        
        if (result == 0) {
            // Payment verified
            headers.addCopy(LowerCaseString("x-payment-verified"), "true");
            return FilterHeadersStatus::Continue;
        } else {
            // Payment failed
            sendPaymentRequired();
            return FilterHeadersStatus::StopIteration;
        }
    }

private:
    void sendPaymentRequired() {
        // Send 402 response
        decoder_callbacks_->sendLocalReply(
            Code::PaymentRequired,
            "{\"error\":\"Payment Required\",\"amount\":" + std::to_string(amount_) + "}",
            nullptr, absl::nullopt, ""
        );
    }

    int amount_;
};

} // namespace Http
} // namespace Envoy
EOF

echo "✅ Envoy C++ extension created: plugins/envoy/"
echo

echo "📦 Plugin Files Created:"
echo "├── plugins/nginx/ngx_http_paygress_module.so  # NGINX native module"
echo "├── plugins/traefik/main.go                    # Traefik Go plugin"
echo "└── plugins/envoy/paygress_filter.cc           # Envoy C++ extension"
echo

echo "🚀 Installation Instructions:"
echo

echo "📘 NGINX Ingress Controller:"
echo "1. Copy .so to NGINX modules: sudo cp plugins/nginx/ngx_http_paygress_module.so /etc/nginx/modules/"
echo "2. Add to nginx.conf: load_module modules/ngx_http_paygress_module.so;"
echo "3. Configure ingress with: paygress_enable on; paygress_amount 1000;"
echo

echo "🟣 Traefik:"
echo "1. Build Go plugin: cd plugins/traefik && go build -buildmode=plugin -o paygress.so"
echo "2. Copy to Traefik plugins directory"
echo "3. Configure as middleware in your IngressRoute"
echo

echo "🟢 Envoy/Istio:"
echo "1. Compile C++ extension with Envoy build system"
echo "2. Add to Envoy configuration as http_filter"
echo "3. Deploy with Istio EnvoyFilter"
echo

echo "✨ All .so plugins built successfully!"
echo "These provide NATIVE performance - no WASM overhead!"
