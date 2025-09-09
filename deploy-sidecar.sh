#!/bin/bash

# Paygress Sidecar Service Deployment Script
set -e

echo "🚀 Deploying Paygress Sidecar Service"
echo "======================================"

# Configuration
NAMESPACE="ingress-system"
USER_NAMESPACE="user-workloads"
SERVICE_NAME="paygress-sidecar"
IMAGE_TAG="paygress:latest"

# Function to check if kubectl is available
check_kubectl() {
    if ! command -v kubectl &> /dev/null; then
        echo "❌ kubectl is not installed or not in PATH"
        exit 1
    fi
    
    echo "✅ kubectl found"
}

# Function to check if cluster is accessible
check_cluster() {
    echo "🔍 Checking cluster connectivity..."
    
    if ! kubectl cluster-info &> /dev/null; then
        echo "❌ Cannot connect to Kubernetes cluster"
        echo "Please ensure your kubectl is configured and cluster is accessible"
        exit 1
    fi
    
    echo "✅ Cluster connectivity OK"
}

# Function to create namespaces
create_namespaces() {
    echo "📁 Creating namespaces..."
    
    # Create ingress-system namespace if it doesn't exist
    if ! kubectl get namespace "$NAMESPACE" &> /dev/null; then
        kubectl create namespace "$NAMESPACE"
        echo "✅ Created namespace: $NAMESPACE"
    else
        echo "✅ Namespace already exists: $NAMESPACE"
    fi
    
    # Create user-workloads namespace if it doesn't exist
    if ! kubectl get namespace "$USER_NAMESPACE" &> /dev/null; then
        kubectl create namespace "$USER_NAMESPACE"
        echo "✅ Created namespace: $USER_NAMESPACE"
    else
        echo "✅ Namespace already exists: $USER_NAMESPACE"
    fi
}

# Function to build Docker image
build_image() {
    echo "🔨 Building Docker image..."
    
    if [ ! -f "Dockerfile" ]; then
        echo "❌ Dockerfile not found in current directory"
        exit 1
    fi
    
    echo "Building image: $IMAGE_TAG"
    docker build -t "$IMAGE_TAG" .
    
    echo "✅ Docker image built successfully"
}

# Function to load image into cluster
load_image() {
    echo "📦 Loading image into cluster..."
    
    # Detect cluster type and load image accordingly
    if command -v kind &> /dev/null && kind get clusters 2>/dev/null | grep -q .; then
        echo "Detected kind cluster, loading image..."
        kind load docker-image "$IMAGE_TAG"
        echo "✅ Image loaded into kind cluster"
    elif command -v minikube &> /dev/null && minikube status &> /dev/null; then
        echo "Detected minikube cluster, loading image..."
        minikube image load "$IMAGE_TAG"
        echo "✅ Image loaded into minikube cluster"
    else
        echo "⚠️  Could not detect kind or minikube cluster"
        echo "   If using a cloud cluster, ensure image is pushed to a registry"
        echo "   and update the image name in k8s/sidecar-service.yaml"
    fi
}

# Function to deploy manifests
deploy_manifests() {
    echo "📋 Deploying Kubernetes manifests..."
    
    if [ ! -f "k8s/sidecar-service.yaml" ]; then
        echo "❌ Manifest file not found: k8s/sidecar-service.yaml"
        exit 1
    fi
    
    kubectl apply -f k8s/sidecar-service.yaml
    echo "✅ Manifests applied successfully"
}

# Function to wait for deployment
wait_for_deployment() {
    echo "⏳ Waiting for deployment to be ready..."
    
    kubectl wait --for=condition=available --timeout=300s \
        deployment/"$SERVICE_NAME" -n "$NAMESPACE"
    
    echo "✅ Deployment is ready"
}

# Function to show service status
show_status() {
    echo "📊 Service Status"
    echo "=================="
    
    echo
    echo "🏷️  Pods:"
    kubectl get pods -n "$NAMESPACE" -l app="$SERVICE_NAME"
    
    echo
    echo "🌐 Services:"
    kubectl get svc -n "$NAMESPACE" -l app="$SERVICE_NAME"
    
    echo
    echo "🔧 ConfigMaps:"
    kubectl get configmap -n "$NAMESPACE" -l app="$SERVICE_NAME" 2>/dev/null || echo "No ConfigMaps found"
    
    echo
    echo "👤 Service Account:"
    kubectl get serviceaccount -n "$NAMESPACE" "$SERVICE_NAME" 2>/dev/null || echo "No ServiceAccount found"
    
    echo
    echo "🔐 RBAC:"
    kubectl get clusterrole "$SERVICE_NAME" 2>/dev/null || echo "No ClusterRole found"
    kubectl get clusterrolebinding "$SERVICE_NAME" 2>/dev/null || echo "No ClusterRoleBinding found"
}

# Function to show access information
show_access_info() {
    echo
    echo "🌐 Access Information"
    echo "===================="
    
    echo "To access the service locally:"
    echo "kubectl port-forward -n $NAMESPACE svc/$SERVICE_NAME 8080:8080"
    echo
    echo "Then visit:"
    echo "- Health check: http://localhost:8080/healthz"
    echo "- Spawn pod: POST http://localhost:8080/spawn-pod"
    echo "- List pods: http://localhost:8080/pods"
    echo
    echo "📖 For usage examples, see README-SIDECAR.md"
    echo "🎬 Run the demo: ./examples/sidecar_demo.sh"
}

# Function to test service
test_service() {
    echo "🧪 Testing service..."
    
    # Port forward in background
    kubectl port-forward -n "$NAMESPACE" svc/"$SERVICE_NAME" 8080:8080 &> /dev/null &
    PF_PID=$!
    
    # Wait a moment for port forward to establish
    sleep 3
    
    # Test health endpoint
    if curl -s http://localhost:8080/healthz &> /dev/null; then
        echo "✅ Service is responding"
        
        # Get health status
        echo "📊 Health Status:"
        curl -s http://localhost:8080/healthz | jq . 2>/dev/null || echo "  (Could not parse JSON response)"
    else
        echo "❌ Service is not responding"
        echo "   Check the logs: kubectl logs -n $NAMESPACE -l app=$SERVICE_NAME"
    fi
    
    # Kill port forward
    kill $PF_PID 2>/dev/null || true
}

# Function to show logs
show_logs() {
    echo "📋 Recent logs:"
    kubectl logs -n "$NAMESPACE" -l app="$SERVICE_NAME" --tail=20 2>/dev/null || echo "No logs available yet"
}

# Function to cleanup
cleanup() {
    echo "🧹 Cleaning up Paygress Sidecar Service..."
    
    kubectl delete -f k8s/sidecar-service.yaml 2>/dev/null || echo "Some resources may not exist"
    
    # Optionally remove namespaces (commented out for safety)
    # kubectl delete namespace "$USER_NAMESPACE" 2>/dev/null || true
    
    echo "✅ Cleanup completed"
}

# Main deployment function
deploy() {
    echo "Starting deployment..."
    
    check_kubectl
    check_cluster
    create_namespaces
    
    if [[ "$SKIP_BUILD" != "true" ]]; then
        build_image
        load_image
    else
        echo "⏭️  Skipping image build (SKIP_BUILD=true)"
    fi
    
    deploy_manifests
    wait_for_deployment
    show_status
    
    echo
    echo "✅ Deployment completed successfully!"
    
    if [[ "$RUN_TESTS" == "true" ]]; then
        echo
        test_service
    fi
    
    show_access_info
}

# Help function
show_help() {
    echo "Paygress Sidecar Service Deployment Script"
    echo
    echo "Usage: $0 [OPTION]"
    echo
    echo "Options:"
    echo "  deploy      Deploy the sidecar service (default)"
    echo "  status      Show current deployment status"
    echo "  test        Test the deployed service"
    echo "  logs        Show recent logs"
    echo "  cleanup     Remove the deployment"
    echo "  help        Show this help message"
    echo
    echo "Environment Variables:"
    echo "  SKIP_BUILD=true    Skip Docker image build and load"
    echo "  RUN_TESTS=true     Run tests after deployment"
    echo
    echo "Examples:"
    echo "  $0                    # Deploy with defaults"
    echo "  $0 status             # Show status"
    echo "  SKIP_BUILD=true $0    # Deploy without building"
    echo "  RUN_TESTS=true $0     # Deploy and test"
}

# Main script
case "${1:-deploy}" in
    "deploy")
        deploy
        ;;
    "status")
        show_status
        ;;
    "test")
        test_service
        ;;
    "logs")
        show_logs
        ;;
    "cleanup")
        cleanup
        ;;
    "help"|"-h"|"--help")
        show_help
        ;;
    *)
        echo "❌ Unknown option: $1"
        echo "Use '$0 help' for usage information"
        exit 1
        ;;
esac