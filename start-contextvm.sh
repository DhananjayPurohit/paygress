#!/bin/bash
# Start Context VM Gateway for Paygress
# This script runs gateway-cli which spawns paygress in MCP-only mode

# Get the directory where this script is located
BASEDIR=$(dirname "$0")
cd "$BASEDIR"

# Check if gateway-cli is available
if ! command -v gateway-cli &> /dev/null; then
    echo "❌ gateway-cli not found"
    echo "📦 Installing gateway-cli..."
    npm install -g @contextvm/gateway-cli
    
    if ! command -v gateway-cli &> /dev/null; then
        echo "❌ Failed to install gateway-cli"
        echo "   Please install Node.js and npm first:"
        echo "   sudo apt install nodejs npm"
        exit 1
    fi
fi

echo "🤖 Starting Context VM Gateway for Paygress"
echo "=========================================="

# Check for Context VM private key in environment or .env
if [ -z "$CONTEXTVM_PRIVATE_KEY" ]; then
    if [ -f ".env" ]; then
        source .env
    fi
fi

if [ -z "$CONTEXTVM_PRIVATE_KEY" ]; then
    echo "⚠️  Warning: CONTEXTVM_PRIVATE_KEY not set"
    echo "   Using default key (NOT SECURE FOR PRODUCTION)"
    CONTEXTVM_PRIVATE_KEY="5747e39e96339baf8484d9a503286c302aff8ee88c796812839e2dccf9b75dfe"
fi

# Start Context VM with Paygress MCP-only mode
exec gateway-cli \
    --private-key "$CONTEXTVM_PRIVATE_KEY" \
    --relays "wss://relay.contextvm.org" \
    --server ./start-paygress-mcp.sh

