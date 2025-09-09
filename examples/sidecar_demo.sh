#!/bin/bash

# Paygress Sidecar Service Demo
# This script demonstrates how to interact with the sidecar service

set -e

SIDECAR_URL="http://localhost:8080"
EXAMPLE_CASHU_TOKEN="cashuAeyJ0b2tlbiI6W3sibWludCI6Imh0dHA6Ly9sb2NhbGhvc3Q6MzMzOCIsInByb29mcyI6W3siYW1vdW50IjoxLCJpZCI6IkkyeU4raVJZZmt6VFEiLCJzZWNyZXQiOiI4d3NXc3luNkt6N3N2L1QycFgvQlY4MFNzOVR0YW11MmNoQ1Azd1duNW1vPSJ9LHsiYW1vdW50IjoyLCJpZCI6Ikkyesk4aWRkY1R0YW11MmNoQ1Azd1duNW1vPSJ9XX1dfQ"

echo "🚀 Paygress Sidecar Service Demo"
echo "================================="

# Function to check if service is running
check_service() {
    echo "📊 Checking service health..."
    response=$(curl -s "${SIDECAR_URL}/healthz" || echo "ERROR")
    
    if [[ "$response" == "ERROR" ]]; then
        echo "❌ Service not running. Please start the sidecar service first:"
        echo "   kubectl port-forward -n ingress-system svc/paygress-sidecar 8080:8080"
        exit 1
    fi
    
    echo "✅ Service is healthy"
    echo "$response" | jq .
    echo
}

# Function to spawn an SSH pod
spawn_pod() {
    local duration=${1:-60}
    local username=${2:-"demo-user"}
    
    echo "🚀 Spawning SSH pod for ${duration} minutes..."
    echo "💰 Payment calculation: $(echo "scale=2; ${duration}/60*100" | bc) sats"
    
    response=$(curl -s -X POST "${SIDECAR_URL}/spawn-pod" \
        -H "Content-Type: application/json" \
        -d "{
            \"cashu_token\": \"${EXAMPLE_CASHU_TOKEN}\",
            \"duration_minutes\": ${duration},
            \"ssh_username\": \"${username}\"
        }")
    
    if echo "$response" | jq -e '.success == true' > /dev/null; then
        echo "✅ Pod spawned successfully!"
        echo "$response" | jq .
        
        # Extract SSH details
        pod_name=$(echo "$response" | jq -r '.pod_info.pod_name')
        ssh_username=$(echo "$response" | jq -r '.pod_info.ssh_username')
        ssh_password=$(echo "$response" | jq -r '.pod_info.ssh_password')
        expires_at=$(echo "$response" | jq -r '.pod_info.expires_at')
        
        echo
        echo "🔑 SSH Access Details:"
        echo "   Pod Name: ${pod_name}"
        echo "   Username: ${ssh_username}"
        echo "   Password: ${ssh_password}"
        echo "   Expires:  ${expires_at}"
        echo
        echo "🌐 To connect via SSH:"
        echo "   1. Get the NodePort: kubectl get svc -n user-workloads ${pod_name}-ssh"
        echo "   2. Connect: ssh ${ssh_username}@<node-ip> -p <nodeport>"
        echo "   3. Use password: ${ssh_password}"
        echo
        
        return 0
    else
        echo "❌ Failed to spawn pod:"
        echo "$response" | jq .
        return 1
    fi
}

# Function to list active pods
list_pods() {
    echo "📋 Listing active pods..."
    response=$(curl -s "${SIDECAR_URL}/pods")
    
    pod_count=$(echo "$response" | jq '. | length')
    echo "Found ${pod_count} active pod(s)"
    
    if [[ "$pod_count" -gt 0 ]]; then
        echo "$response" | jq .
    else
        echo "No active pods found."
    fi
    echo
}

# Function to get specific pod info
get_pod_info() {
    local pod_name="$1"
    
    if [[ -z "$pod_name" ]]; then
        echo "❌ Pod name required"
        return 1
    fi
    
    echo "🔍 Getting info for pod: ${pod_name}"
    response=$(curl -s "${SIDECAR_URL}/pods/${pod_name}")
    
    if echo "$response" | jq -e '.pod_name' > /dev/null 2>&1; then
        echo "✅ Pod found:"
        echo "$response" | jq .
    else
        echo "❌ Pod not found"
    fi
    echo
}

# Function to test auth endpoint
test_auth() {
    local duration=${1:-60}
    
    echo "🔐 Testing auth endpoint for ${duration} minutes..."
    
    response=$(curl -s -w "HTTP_CODE:%{http_code}" \
        "${SIDECAR_URL}/auth?token=${EXAMPLE_CASHU_TOKEN}&duration_minutes=${duration}")
    
    http_code=$(echo "$response" | grep -o "HTTP_CODE:[0-9]*" | cut -d: -f2)
    body=$(echo "$response" | sed 's/HTTP_CODE:[0-9]*$//')
    
    if [[ "$http_code" == "200" ]]; then
        echo "✅ Auth successful (HTTP $http_code)"
    elif [[ "$http_code" == "402" ]]; then
        echo "💳 Payment required (HTTP $http_code)"
    else
        echo "❌ Auth failed (HTTP $http_code)"
    fi
    
    if [[ -n "$body" ]]; then
        echo "Response: $body"
    fi
    echo
}

# Function to monitor pods
monitor_pods() {
    echo "👀 Monitoring pods (Ctrl+C to stop)..."
    echo
    
    while true; do
        clear
        echo "📊 Pod Monitor - $(date)"
        echo "===================="
        
        # Service health
        health=$(curl -s "${SIDECAR_URL}/healthz" | jq -r '.active_pods // "unknown"')
        echo "Active pods: $health"
        echo
        
        # List pods
        pods=$(curl -s "${SIDECAR_URL}/pods")
        pod_count=$(echo "$pods" | jq '. | length')
        
        if [[ "$pod_count" -gt 0 ]]; then
            echo "Pod Details:"
            echo "$pods" | jq -r '.[] | "  \(.pod_name) - \(.ssh_username) - expires: \(.expires_at)"'
        else
            echo "No active pods"
        fi
        
        echo
        echo "Kubernetes Status:"
        kubectl get pods -n user-workloads -l app=paygress-ssh-pod --no-headers 2>/dev/null | head -5 || echo "  No Kubernetes pods found"
        
        sleep 10
    done
}

# Main menu
show_menu() {
    echo "Choose an option:"
    echo "1. Check service health"
    echo "2. Spawn SSH pod (1 hour)"
    echo "3. Spawn SSH pod (custom duration)"
    echo "4. List active pods"
    echo "5. Get pod info"
    echo "6. Test auth endpoint"
    echo "7. Monitor pods (live)"
    echo "8. Full demo sequence"
    echo "9. Exit"
    echo
    read -p "Enter choice [1-9]: " choice
    echo
}

# Full demo sequence
full_demo() {
    echo "🎬 Running full demo sequence..."
    echo
    
    check_service
    
    echo "Step 1: Testing auth endpoint..."
    test_auth 60
    
    echo "Step 2: Spawning a 1-hour SSH pod..."
    if spawn_pod 60 "demo-user-1h"; then
        
        echo "Step 3: Spawning a 30-minute SSH pod..."
        spawn_pod 30 "demo-user-30m"
        
        echo "Step 4: Listing all active pods..."
        list_pods
        
        echo "Step 5: Getting specific pod info..."
        # Get the first pod name from the list
        first_pod=$(curl -s "${SIDECAR_URL}/pods" | jq -r '.[0].pod_name // empty')
        if [[ -n "$first_pod" ]]; then
            get_pod_info "$first_pod"
        fi
        
        echo "✅ Demo sequence completed!"
        echo "🔗 Pods will automatically be cleaned up when they expire."
    fi
}

# Main script
main() {
    if [[ "$1" == "--auto" ]]; then
        check_service
        full_demo
        exit 0
    fi
    
    if [[ "$1" == "--spawn" ]]; then
        check_service
        spawn_pod "${2:-60}" "${3:-demo-user}"
        exit 0
    fi
    
    if [[ "$1" == "--monitor" ]]; then
        monitor_pods
        exit 0
    fi
    
    while true; do
        show_menu
        
        case $choice in
            1)
                check_service
                ;;
            2)
                spawn_pod 60 "demo-user"
                ;;
            3)
                read -p "Enter duration in minutes: " duration
                read -p "Enter SSH username [demo-user]: " username
                username=${username:-demo-user}
                spawn_pod "$duration" "$username"
                ;;
            4)
                list_pods
                ;;
            5)
                read -p "Enter pod name: " pod_name
                get_pod_info "$pod_name"
                ;;
            6)
                read -p "Enter duration in minutes [60]: " duration
                duration=${duration:-60}
                test_auth "$duration"
                ;;
            7)
                monitor_pods
                ;;
            8)
                full_demo
                ;;
            9)
                echo "👋 Goodbye!"
                exit 0
                ;;
            *)
                echo "❌ Invalid choice. Please try again."
                ;;
        esac
        
        read -p "Press Enter to continue..."
        echo
    done
}

# Run main function
main "$@"
