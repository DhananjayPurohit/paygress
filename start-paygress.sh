#!/bin/bash
# Start script for Paygress Unified Service
# This script loads environment variables and starts the unified service

# Get the directory where this script is located
BASEDIR=$(dirname "$0")

# Change to the script directory
cd "$BASEDIR"

# Load environment variables from .env file if it exists
if [ -f ".env" ]; then
    echo "📋 Loading environment variables from .env file..."
    set -a  # automatically export all variables
    source .env
    set +a
    echo "✅ Environment variables loaded"
else
    echo "⚠️  No .env file found, using system environment variables"
fi

# Set default logging level if not set
export RUST_LOG="${RUST_LOG:-info}"

# Create necessary directories
echo "📁 Creating necessary directories..."
mkdir -p data
mkdir -p "$(dirname "${CASHU_DB_PATH:-./cashu.db}")"
echo "✅ Directories created"

# Check if binary exists (prefer release build for production)
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
echo "🚀 Starting Paygress Unified Service"
echo "===================================="
echo "Interfaces enabled:"
echo "  - Nostr: ${ENABLE_NOSTR:-true}"
echo "  - MCP:   ${ENABLE_MCP:-true}"
echo "  - HTTP:  ${ENABLE_HTTP:-true}"
if [ "${ENABLE_HTTP:-true}" = "true" ]; then
    echo "  - HTTP URL: http://${HTTP_BIND_ADDR:-0.0.0.0:8080}"
fi
echo ""

# Run the unified service
exec "$BINARY_PATH" "$@"
