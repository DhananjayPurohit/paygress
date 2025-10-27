#!/bin/bash
# Start script for Paygress MCP-only Service (for Context VM)
# This script loads .env.contextvm and runs only the MCP interface

# Get the directory where this script is located
BASEDIR=$(dirname "$0")

# Change to the script directory
cd "$BASEDIR"

# IMPORTANT: All echo statements must go to stderr (>2) to preserve stdin/stdout for MCP JSON-RPC protocol

# Load environment variables from .env.contextvm file
if [ -f ".env.contextvm" ]; then
    echo "Loading MCP-only environment variables from .env.contextvm..." >&2
    set -a  # automatically export all variables
    source .env.contextvm
    set +a
    echo "Environment variables loaded" >&2
else
    echo "Error: .env.contextvm file not found" >&2
    echo "Creating .env.contextvm from .env..." >&2
    if [ -f ".env" ]; then
        cp .env .env.contextvm
        # Override interface settings for MCP-only
        sed -i 's/ENABLE_HTTP=.*/ENABLE_HTTP=false/' .env.contextvm
        sed -i 's/ENABLE_NOSTR=.*/ENABLE_NOSTR=false/' .env.contextvm
        sed -i 's/ENABLE_MCP=.*/ENABLE_MCP=true/' .env.contextvm
        echo "Created .env.contextvm" >&2
    else
        echo "Error: .env file not found either" >&2
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
    echo "Using debug build. For production, run: cargo build --release" >&2
else
    echo "Error: Binary not found" >&2
    echo "Please run: cargo build --release (for production) or cargo build (for development)" >&2
    exit 1
fi

# Log startup to stderr only
echo "Starting Paygress MCP-only Service (for Context VM)" >&2
echo "Interfaces: MCP=true, HTTP=false, Nostr=false" >&2

# Run the MCP-only service
# stdin/stdout must be clean for JSON-RPC protocol
exec "$BINARY_PATH" "$@"

