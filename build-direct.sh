#!/bin/bash

echo "🚀 Building Direct NGINX Module (No Lua)"
echo "======================================="
echo

# Build the direct module
echo "Building Rust → Direct NGINX Module..."
cargo build --release --features nginx-direct --lib

if [ $? -eq 0 ]; then
    SO_FILE="target/release/libpaygress.so"
    
    if [ -f "$SO_FILE" ]; then
        echo "✅ Built successfully: $SO_FILE"
        
        # Copy to nginx-ready location
        mkdir -p nginx-direct/
        cp "$SO_FILE" nginx-direct/paygress_direct.so
        
        echo "📦 Direct NGINX module ready: nginx-direct/paygress_direct.so"
        echo
        echo "🔧 Installation:"
        echo "1. Copy to NGINX: sudo cp nginx-direct/paygress_direct.so /etc/nginx/modules/"
        echo "2. Add to nginx.conf: load_module modules/paygress_direct.so;"
        echo "3. Use directives: paygress on; paygress_amount 1000;"
        echo
        echo "📋 Available Directives:"
        echo "   paygress on|off          # Enable/disable payment checking"
        echo "   paygress_amount <number> # Set required payment amount"
        echo
        echo "🎯 How it works:"
        echo "   → Request comes to location with 'paygress on'"
        echo "   → Your Rust handler runs automatically"
        echo "   → Checks Authorization header for Cashu token"
        echo "   → Returns 402 if invalid, continues if valid"
        echo "   → NO LUA SCRIPTS NEEDED!"
        echo
        echo "🧪 Test:"
        echo "   curl -H 'Authorization: Bearer 1000sat-token' http://localhost/premium"
    else
        echo "❌ .so file not found"
        exit 1
    fi
else
    echo "❌ Build failed"
    exit 1
fi
