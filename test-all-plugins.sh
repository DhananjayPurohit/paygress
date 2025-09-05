#!/bin/bash

echo "🚀 Testing Paygress Native Ingress Plugins"
echo "=========================================="
echo

# Test Service Mode
echo "1️⃣ Testing External Service Mode..."
echo "Building paygress service..."
cargo build --features service --bin paygress-service
if [ $? -eq 0 ]; then
    echo "✅ Service mode builds successfully"
    echo "   Run with: ./target/debug/paygress-service"
else
    echo "❌ Service mode build failed"
fi
echo

# Test NGINX Plugin
echo "2️⃣ Testing NGINX Native Plugin..."
echo "Building nginx plugin..."
cargo build --features nginx --bin nginx-plugin
if [ $? -eq 0 ]; then
    echo "✅ NGINX plugin builds successfully"
    echo "   Run with: ./target/debug/nginx-plugin"
    echo "   🔗 For production: cargo build --release --features nginx --bin nginx-plugin"
else
    echo "❌ NGINX plugin build failed"
fi
echo

# Test Traefik Plugin
echo "3️⃣ Testing Traefik Native Plugin..."
echo "Building traefik plugin..."
cargo build --features traefik --bin traefik-plugin
if [ $? -eq 0 ]; then
    echo "✅ Traefik plugin builds successfully"
    echo "   Run with: ./target/debug/traefik-plugin"
    echo "   🔗 For WASM: cargo build --target wasm32-unknown-unknown --features traefik --bin traefik-plugin"
else
    echo "❌ Traefik plugin build failed"
fi
echo

# Test Envoy Plugin
echo "4️⃣ Testing Envoy Native Plugin..."
echo "Building envoy plugin..."
cargo build --features envoy --bin envoy-plugin
if [ $? -eq 0 ]; then
    echo "✅ Envoy plugin builds successfully"
    echo "   Run with: ./target/debug/envoy-plugin"
    echo "   🔗 For WASM: cargo build --target wasm32-unknown-unknown --features envoy --bin envoy-plugin"
else
    echo "❌ Envoy plugin build failed"
fi
echo

# Test All Plugins Feature
echo "5️⃣ Testing All Plugins Feature..."
echo "Building with all-plugins feature..."
cargo build --features all-plugins
if [ $? -eq 0 ]; then
    echo "✅ All plugins feature builds successfully"
else
    echo "❌ All plugins feature build failed"
fi
echo

echo "🎉 Plugin Testing Complete!"
echo
echo "📋 Available Build Commands:"
echo "   Service:  cargo build --features service --bin paygress-service"
echo "   NGINX:    cargo build --features nginx --bin nginx-plugin"
echo "   Traefik:  cargo build --features traefik --bin traefik-plugin"
echo "   Envoy:    cargo build --features envoy --bin envoy-plugin"
echo "   All:      cargo build --features all-plugins"
echo
echo "🚀 Next Steps:"
echo "   1. Run any plugin binary to see integration details"
echo "   2. For production: use --release flag"
echo "   3. For WASM: use --target wasm32-unknown-unknown"
echo "   4. Deploy using docker-compose.yml or k8s manifests"
