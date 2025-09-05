#!/bin/bash

echo "🔧 Building NGINX Plugin (.so)"
echo "=============================="
echo

# Build the Rust library as .so
echo "Building Rust → .so..."
cargo build --release --features nginx-plugin --lib

if [ $? -eq 0 ]; then
    SO_FILE="target/release/libpaygress.so"
    
    if [ -f "$SO_FILE" ]; then
        echo "✅ Built successfully: $SO_FILE"
        
        # Copy to nginx-ready location
        mkdir -p nginx-plugin/
        cp "$SO_FILE" nginx-plugin/paygress.so
        
        echo "📦 NGINX plugin ready: nginx-plugin/paygress.so"
        echo
        echo "🚀 Installation:"
        echo "1. Copy to NGINX: sudo cp nginx-plugin/paygress.so /etc/nginx/modules/"
        echo "2. Add to nginx.conf: load_module modules/paygress.so;"
        echo "3. Use in locations with FFI calls"
        echo
        echo "🧪 Test functions:"
        echo "   paygress_verify_payment(token, amount) → 0=success, 1=fail"
        echo "   paygress_provision_pod(namespace, name) → 0=success, 1=fail"
        echo "   paygress_version() → version string"
    else
        echo "❌ .so file not found"
        exit 1
    fi
else
    echo "❌ Build failed"
    exit 1
fi
