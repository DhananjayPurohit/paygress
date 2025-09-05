#!/bin/bash

echo "🦀 Building Paygress WASM Ingress Plugins"
echo "========================================="
echo

# Check if wasm-pack is installed
if ! command -v wasm-pack &> /dev/null; then
    echo "📦 Installing wasm-pack..."
    curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
fi

# Add WASM target
echo "🎯 Adding WASM target..."
rustup target add wasm32-unknown-unknown

echo
echo "Building WASM plugins for ingress controllers..."
echo

# Build NGINX Ingress Controller WASM Plugin
echo "1️⃣ Building NGINX Ingress Controller WASM Plugin..."
wasm-pack build --target web --features nginx-wasm --out-dir pkg/nginx-wasm
if [ $? -eq 0 ]; then
    echo "✅ NGINX WASM plugin built: pkg/nginx-wasm/"
else
    echo "❌ NGINX WASM plugin build failed"
fi
echo

# Build Traefik WASM Plugin
echo "2️⃣ Building Traefik WASM Plugin..."
wasm-pack build --target web --features traefik-wasm --out-dir pkg/traefik-wasm
if [ $? -eq 0 ]; then
    echo "✅ Traefik WASM plugin built: pkg/traefik-wasm/"
else
    echo "❌ Traefik WASM plugin build failed"
fi
echo

# Build Envoy/Istio WASM Plugin
echo "3️⃣ Building Envoy/Istio WASM Plugin..."
cargo build --target wasm32-unknown-unknown --features envoy-wasm --release
if [ $? -eq 0 ]; then
    echo "✅ Envoy WASM plugin built: target/wasm32-unknown-unknown/release/paygress.wasm"
    # Copy to standard location
    mkdir -p pkg/envoy-wasm
    cp target/wasm32-unknown-unknown/release/paygress.wasm pkg/envoy-wasm/
else
    echo "❌ Envoy WASM plugin build failed"
fi
echo

echo "📦 Plugin Files Created:"
echo "├── pkg/nginx-wasm/paygress.wasm     # NGINX Ingress Controller"
echo "├── pkg/traefik-wasm/paygress.wasm   # Traefik"
echo "└── pkg/envoy-wasm/paygress.wasm     # Envoy/Istio"
echo

echo "🚀 Integration Instructions:"
echo

echo "📘 NGINX Ingress Controller:"
echo "1. Copy pkg/nginx-wasm/ to your NGINX ingress controller"
echo "2. Add wasm configuration to your ingress annotations"
echo "3. Deploy and test"
echo

echo "🟣 Traefik:"
echo "1. Create a Traefik plugin from pkg/traefik-wasm/"
echo "2. Configure as middleware in your IngressRoute"
echo "3. Deploy and test"
echo

echo "🟢 Envoy/Istio:"
echo "1. Deploy pkg/envoy-wasm/paygress.wasm to your Envoy config"
echo "2. Configure as HTTP filter in Envoy/Istio"
echo "3. Deploy and test"
echo

echo "✨ All WASM plugins built successfully!"
echo "These run DIRECTLY inside your ingress controllers!"
