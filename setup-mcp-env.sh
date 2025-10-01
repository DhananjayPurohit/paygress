#!/bin/bash

# Setup script for Paygress MCP Server
echo "🔧 Setting up Paygress MCP Server environment..."

# Check if required environment variables are set
check_env_var() {
    if [ -z "${!1}" ]; then
        echo "⚠️  Environment variable $1 is not set"
        return 1
    else
        echo "✅ $1 is set"
        return 0
    fi
}

# Set default environment variables if not already set
export CASHU_DB_PATH="${CASHU_DB_PATH:-./cashu.db}"
export POD_SPECS_FILE="${POD_SPECS_FILE:-./pod-specs.json}"
export POD_NAMESPACE="${POD_NAMESPACE:-user-workloads}"
export MINIMUM_POD_DURATION_SECONDS="${MINIMUM_POD_DURATION_SECONDS:-60}"
export SSH_HOST="${SSH_HOST:-localhost}"
export SSH_PORT_RANGE_START="${SSH_PORT_RANGE_START:-30000}"
export SSH_PORT_RANGE_END="${SSH_PORT_RANGE_END:-31000}"
export BASE_IMAGE="${BASE_IMAGE:-linuxserver/openssh-server:latest}"
export ENABLE_CLEANUP_TASK="${ENABLE_CLEANUP_TASK:-true}"

# Check for required variables
echo "🔍 Checking environment variables..."
all_set=true

if ! check_env_var "WHITELISTED_MINTS"; then
    echo "❌ WHITELISTED_MINTS is required!"
    echo "   Example: export WHITELISTED_MINTS='https://mint.cashu.space,https://mint.f7z.io'"
    all_set=false
fi

# Check optional but recommended variables
check_env_var "CASHU_DB_PATH"
check_env_var "POD_SPECS_FILE"
check_env_var "POD_NAMESPACE"
check_env_var "SSH_HOST"

if [ "$all_set" = false ]; then
    echo ""
    echo "❌ Please set the required environment variables before running the MCP server."
    echo ""
    echo "Quick setup example:"
    echo "export WHITELISTED_MINTS='https://mint.cashu.space,https://mint.f7z.io'"
    echo "export SSH_HOST='your-cluster-ip'"
    echo ""
    exit 1
fi

echo ""
echo "✅ Environment setup complete!"
echo "📋 Current configuration:"
echo "   WHITELISTED_MINTS: $WHITELISTED_MINTS"
echo "   CASHU_DB_PATH: $CASHU_DB_PATH"
echo "   POD_SPECS_FILE: $POD_SPECS_FILE"
echo "   POD_NAMESPACE: $POD_NAMESPACE"
echo "   SSH_HOST: $SSH_HOST"
echo "   SSH_PORT_RANGE: $SSH_PORT_RANGE_START-$SSH_PORT_RANGE_END"
echo ""

# Check if pod-specs.json exists
if [ -f "$POD_SPECS_FILE" ]; then
    echo "✅ Pod specifications file found: $POD_SPECS_FILE"
    spec_count=$(jq length "$POD_SPECS_FILE" 2>/dev/null || echo "unknown")
    echo "   Contains $spec_count pod specifications"
else
    echo "⚠️  Pod specifications file not found: $POD_SPECS_FILE"
    echo "   The server will use default specifications"
fi

echo ""
echo "🚀 Ready to start MCP server!"
echo "   Run: cargo run --bin paygress-mcp-server"
echo ""
echo "🔍 For debugging, you can also use:"
echo "   RUST_LOG=debug cargo run --bin paygress-mcp-server"
echo ""
echo "📖 For MCP Inspector integration:"
echo "   cargo run --bin paygress-mcp-server | npx @modelcontextprotocol/inspector"
