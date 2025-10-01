#!/bin/bash
# Simple test script for Paygress Unified Service

set -e

echo "🧪 Quick Paygress Test"
echo "======================"

# Test 1: Build
echo "Building service..."
cargo build --bin paygress
echo "✅ Build successful"

# Test 2: Check binary
if [ -f "./target/debug/paygress" ]; then
    echo "✅ Binary exists"
else
    echo "❌ Binary not found"
    exit 1
fi

# Test 3: Quick HTTP test
echo "Testing HTTP interface..."
export ENABLE_NOSTR=false
export ENABLE_MCP=false
export ENABLE_HTTP=true
export HTTP_PORT=8080
export WHITELISTED_MINTS=https://testnut.cashu.space
export POD_SPECS_FILE=./pod-specs.json
export CASHU_DB_PATH=./test_cashu.db

# Start service in background
./target/debug/paygress &
SERVICE_PID=$!
sleep 3

# Test health endpoint
HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" http://localhost:8080/health)
if [ "$HTTP_CODE" == "200" ]; then
    echo "✅ HTTP service working"
else
    echo "❌ HTTP service failed (HTTP $HTTP_CODE)"
fi

# Cleanup
kill $SERVICE_PID 2>/dev/null || true
rm -f ./test_cashu.db

echo "🎉 Test completed!"
echo "To start the full service: ./start-paygress.sh"

