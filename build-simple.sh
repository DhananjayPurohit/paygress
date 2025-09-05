#!/bin/bash

# Simple Paygress Plugin Build Script
# This shows you how to build the native plugins

set -e

echo "🔧 Building Paygress Native Plugins"
echo ""

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${BLUE}📋 Available build targets:${NC}"
echo "1. cargo build --features service             # External service (your current setup)"
echo "2. cargo build --features nginx --bin nginx-plugin     # NGINX native plugin"
echo "3. cargo build --features traefik --bin traefik-plugin # Traefik WebAssembly plugin"
echo "4. cargo build --features envoy --bin envoy-plugin     # Envoy Proxy-WASM plugin"
echo ""

echo -e "${BLUE}🚀 Testing builds...${NC}"

# Test external service build (your current working setup)
echo -e "${BLUE}Building external service...${NC}"
if cargo build --features service; then
    echo -e "${GREEN}✅ External service build successful${NC}"
else
    echo -e "${RED}❌ External service build failed${NC}"
fi
echo ""

# Test nginx plugin build
echo -e "${BLUE}Building NGINX plugin...${NC}"
if cargo build --features nginx --bin nginx-plugin; then
    echo -e "${GREEN}✅ NGINX plugin build successful${NC}"
    echo "   Output: target/debug/nginx-plugin"
else
    echo -e "${RED}❌ NGINX plugin build failed${NC}"
fi
echo ""

# Test traefik plugin build
echo -e "${BLUE}Building Traefik plugin...${NC}"
if cargo build --features traefik --bin traefik-plugin; then
    echo -e "${GREEN}✅ Traefik plugin build successful${NC}"
    echo "   Output: target/debug/traefik-plugin"
else
    echo -e "${RED}❌ Traefik plugin build failed${NC}"
fi
echo ""

# Test envoy plugin build
echo -e "${BLUE}Building Envoy plugin...${NC}"
if cargo build --features envoy --bin envoy-plugin; then
    echo -e "${GREEN}✅ Envoy plugin build successful${NC}"
    echo "   Output: target/debug/envoy-plugin"
else
    echo -e "${RED}❌ Envoy plugin build failed${NC}"
fi
echo ""

echo -e "${GREEN}🎉 Build tests completed!${NC}"
echo ""
echo -e "${BLUE}📖 How to use:${NC}"
echo ""
echo "🔵 NGINX Plugin:"
echo "   1. The nginx-plugin binary shows how to integrate with NGINX"
echo "   2. In production, this would be compiled as a .so dynamic module"
echo "   3. NGINX would load it with: load_module modules/ngx_http_paygress_module.so;"
echo ""
echo "🟣 Traefik Plugin:"
echo "   1. The traefik-plugin shows the WebAssembly approach"
echo "   2. Would be compiled to WASM and loaded via Traefik's plugin system"
echo "   3. Configured in traefik.yml under experimental.plugins"
echo ""
echo "🟢 Envoy Plugin:"
echo "   1. The envoy-plugin shows Proxy-WASM integration"
echo "   2. Would be compiled to WASM and loaded via EnvoyFilter"
echo "   3. Used in Istio service mesh environments"
echo ""
echo "🔧 External Service (Current):"
echo "   1. Your current working setup - runs as separate service"
echo "   2. Uses external auth with ingress controllers"
echo "   3. Run with: cargo run --features service"
echo ""

# Show current state
echo -e "${BLUE}📊 Current project state:${NC}"
echo "✅ External service: Working (your current setup)"
echo "🔧 NGINX plugin: Framework ready (needs full C integration)"
echo "🔧 Traefik plugin: Framework ready (needs WASM compilation)"
echo "🔧 Envoy plugin: Framework ready (needs Proxy-WASM integration)"
