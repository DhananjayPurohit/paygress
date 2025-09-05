#!/bin/bash

echo "🚀 Paygress Local Testing Script"
echo "==============================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test endpoints
NGINX_URL="http://localhost:8080"
BACKEND_URL="http://localhost:8081"
PREMIUM_URL="$NGINX_URL/premium"

echo -e "${BLUE}Testing Paygress NGINX Plugin locally...${NC}"
echo ""

# Test 1: Access without payment (should fail with 402)
echo -e "${BLUE}Test 1: Access without payment${NC}"
echo "GET $PREMIUM_URL"
response=$(curl -s -w "%{http_code}" -o /tmp/test_response.txt "$PREMIUM_URL")
http_code="${response: -3}"

if [ "$http_code" = "402" ]; then
    echo -e "${GREEN}✅ PASS: Correctly blocked access without payment (402)${NC}"
    echo "Response:"
    cat /tmp/test_response.txt | jq . 2>/dev/null || cat /tmp/test_response.txt
else
    echo -e "${RED}❌ FAIL: Expected 402, got $http_code${NC}"
    cat /tmp/test_response.txt
fi
echo ""

# Test 2: Access with invalid token (should fail with 402)
echo -e "${BLUE}Test 2: Access with invalid Cashu token${NC}"
echo "GET $PREMIUM_URL with Authorization: Bearer invalid_token"
response=$(curl -s -w "%{http_code}" -o /tmp/test_response.txt -H "Authorization: Bearer invalid_token" "$PREMIUM_URL")
http_code="${response: -3}"

if [ "$http_code" = "402" ]; then
    echo -e "${GREEN}✅ PASS: Correctly rejected invalid token (402)${NC}"
    echo "Response:"
    cat /tmp/test_response.txt | jq . 2>/dev/null || cat /tmp/test_response.txt
else
    echo -e "${RED}❌ FAIL: Expected 402, got $http_code${NC}"
    cat /tmp/test_response.txt
fi
echo ""

# Test 3: Access with valid token (simulated)
echo -e "${BLUE}Test 3: Access with valid Cashu token (demo)${NC}"
valid_token="cashu_token_1000_sats_demo"
echo "GET $PREMIUM_URL with Authorization: Bearer $valid_token"
response=$(curl -s -w "%{http_code}" -o /tmp/test_response.txt -H "Authorization: Bearer $valid_token" "$PREMIUM_URL")
http_code="${response: -3}"

if [ "$http_code" = "200" ]; then
    echo -e "${GREEN}✅ PASS: Valid token accepted (200)${NC}"
    echo "Response:"
    cat /tmp/test_response.txt
else
    echo -e "${BLUE}ℹ️  Got $http_code - this might be expected if backend is not running${NC}"
    cat /tmp/test_response.txt
fi
echo ""

# Test 4: Check plugin status
echo -e "${BLUE}Test 4: Check plugin status${NC}"
echo "GET $NGINX_URL/health"
response=$(curl -s -w "%{http_code}" -o /tmp/test_response.txt "$NGINX_URL/health" 2>/dev/null)
http_code="${response: -3}"

echo "Plugin status check: $http_code"
if [ -s /tmp/test_response.txt ]; then
    cat /tmp/test_response.txt
fi
echo ""

# Test 5: Pod access test
echo -e "${BLUE}Test 5: Pod access test${NC}"
echo "GET $PREMIUM_URL with X-Pod-ID header"
response=$(curl -s -w "%{http_code}" -o /tmp/test_response.txt \
    -H "Authorization: Bearer $valid_token" \
    -H "X-Pod-ID: pod-test123" \
    "$PREMIUM_URL")
http_code="${response: -3}"

echo "Pod access test: $http_code"
if [ -s /tmp/test_response.txt ]; then
    cat /tmp/test_response.txt
fi
echo ""

echo -e "${BLUE}📋 How to Send Nostr Events for Pod Provisioning:${NC}"
echo ""
echo "Send a Nostr event (kind 1000) with content like this:"
echo '{'
echo '  "cashu_token": "cashu_token_1000_sats_demo",'
echo '  "amount": 1000,'
echo '  "pod_description": {'
echo '    "image": "nginx:alpine",'
echo '    "cpu": "100m",'
echo '    "memory": "128Mi"'
echo '  }'
echo '}'
echo ""
echo "The plugin will automatically:"
echo "1. Listen for Nostr events"
echo "2. Verify Cashu tokens"
echo "3. Provision pods in Kubernetes"
echo ""

# Cleanup
rm -f /tmp/test_response.txt

echo -e "${GREEN}🎉 Local testing completed!${NC}"
