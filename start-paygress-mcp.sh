#!/bin/bash
# Start script for Paygress MCP-only Service (for Context VM)
# This script loads .env.contextvm and runs only the MCP interface

# Get the directory where this script is located
BASEDIR=$(dirname "$0")

# Change to the script directory
cd "$BASEDIR"

# Load environment variables from .env.contextvm file
if [ -f ".env.contextvm" ]; then
    echo "📋 Loading MCP-only environment variables from .env.contextvm..."
    set -a  # automatically export all variables
    source .env.contextvm
    set +a
    echo "✅ Environment variables loaded"
else
    echo "❌ Error: .env.contextvm file not found"
    echo "   Creating .env.contextvm from .env..."
    if [ -f ".env" ]; then
        cp .env .env.contextvm
        # Override interface settings for MCP-only
        sed -i 's/ENABLE_HTTP=.*/ENABLE_HTTP=false/' .env.contextvm
        sed -i 's/ENABLE_NOSTR=.*/ENABLE_NOSTR=false/' .env.contextvm
        sed -i 's/ENABLE_MCP=.*/ENABLE_MCP=true/' .env.contextvm
        echo "✅ Created .env.contextvm"
    else
        echo "❌ Error: .env file not found either"
        exit 1
    fi
fi

# Set default logging level if not set
export RUST_LOG="${RUST_LOG:-info}"

# Create necessary directories
mkdir -p data
mkdir -p "$(dirname "${CASHU_DB_PATH:-./cashu.db}")"

# Check if binary exists (prefer release build)
if [ -f "./target/release/paygress" ]; then
    BINARY_PATH="./target/release/paygress"
elif [ -f "./target/debug/paygress" ]; then
    BINARY_PATH="./target/debug/paygress"
    echo "⚠️  Using debug build. For production, run: cargo build --release"
else
    echo "❌ Error: Binary not found"
    echo "   Please run: cargo build --release (for production) or cargo build (for development)"
    exit 1
fi

# Display configuration
echo ""
echo "🤖 Starting Paygress MCP-only Service (for Context VM)"
echo "===================================="
echo "Interfaces enabled:"
echo "  - MCP:   true (stdio for gateway-cli)"
echo "  - Nostr: false"
echo "  - HTTP:  false"
echo ""

# Run the MCP-only service
exec "$BINARY_PATH" "$@"

