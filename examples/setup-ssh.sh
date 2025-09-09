#!/bin/bash

# Automatic SSH setup for Paygress pods
# This script gets pod info and sets up port-forwarding automatically

if [ $# -ne 1 ]; then
    echo "Usage: $0 <pod_name>"
    echo "Example: $0 ssh-pod-a1b2c3d4"
    exit 1
fi

POD_NAME="$1"
SIDECAR_URL="${SIDECAR_URL:-http://localhost:8080}"

echo "🚀 Setting up SSH access for pod: $POD_NAME"
echo "=========================================="

# Get port-forward command
echo "📡 Getting port-forward instructions..."
RESPONSE=$(curl -s "${SIDECAR_URL}/pods/${POD_NAME}/port-forward")

if echo "$RESPONSE" | jq -e '.pod_name' > /dev/null 2>&1; then
    # Extract information
    SSH_PORT=$(echo "$RESPONSE" | jq -r '.ssh_port')
    PORT_FORWARD_CMD=$(echo "$RESPONSE" | jq -r '.port_forward_command')
    SSH_CMD=$(echo "$RESPONSE" | jq -r '.ssh_command')
    PASSWORD=$(echo "$RESPONSE" | jq -r '.instructions[2]' | sed 's/Password: //')

    echo "✅ Pod found!"
    echo "📋 SSH Port: $SSH_PORT"
    echo "🔐 Password: $PASSWORD"
    echo

    echo "📝 Instructions:"
    echo "$RESPONSE" | jq -r '.instructions[]'
    echo

    echo "🔄 Starting port-forward..."
    echo "Command: $PORT_FORWARD_CMD"
    echo "Press Ctrl+C to stop port-forwarding"
    echo

    # Run port-forward in background
    eval "$PORT_FORWARD_CMD" &
    PF_PID=$!

    # Wait a moment for port-forward to establish
    sleep 3

    echo "✅ Port-forward established!"
    echo "🌐 SSH Command: $SSH_CMD"
    echo "🔑 Password: $PASSWORD"
    echo

    # Test SSH connection
    echo "🧪 Testing SSH connection..."
    if ssh -o ConnectTimeout=5 -o PreferredAuthentications=password -o PubkeyAuthentication=no -o StrictHostKeyChecking=no \
           -p "$SSH_PORT" "testuser@localhost" "echo 'SSH connection successful!'" 2>/dev/null; then
        echo "✅ SSH connection test passed!"
    else
        echo "⚠️  SSH connection test failed. Password authentication might not be enabled yet."
        echo "   The pod might still be starting up. Try the SSH command manually in a few seconds."
    fi

    echo
    echo "🔄 Port-forward is running in background (PID: $PF_PID)"
    echo "💡 To stop: kill $PF_PID"
    echo "💡 To reconnect: $SSH_CMD"
    echo

    # Keep running until user interrupts
    wait $PF_PID

else
    echo "❌ Pod not found or API error"
    echo "Response: $RESPONSE"
    exit 1
fi
