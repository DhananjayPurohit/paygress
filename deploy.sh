#!/bin/bash
set -e

echo "🚀 Deploying Paygress Ingress Plugin"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if kubectl is available
if ! command -v kubectl &> /dev/null; then
    print_error "kubectl is not installed or not in PATH"
    exit 1
fi

# Check if docker is available
if ! command -v docker &> /dev/null; then
    print_error "docker is not installed or not in PATH"
    exit 1
fi

# Build Docker image
print_status "Building Docker image..."
if docker build -t paygress:latest .; then
    print_success "Docker image built successfully"
else
    print_error "Failed to build Docker image"
    exit 1
fi

# Load image into kind/minikube if available
if command -v kind &> /dev/null; then
    print_status "Loading image into kind cluster..."
    kind load docker-image paygress:latest || print_warning "Failed to load into kind (cluster might not exist)"
elif command -v minikube &> /dev/null; then
    print_status "Loading image into minikube..."
    minikube image load paygress:latest || print_warning "Failed to load into minikube"
fi

# Apply Kubernetes manifests
print_status "Applying Kubernetes manifests..."
if kubectl apply -f k8s/ingress-plugin.yaml; then
    print_success "Kubernetes manifests applied"
else
    print_error "Failed to apply Kubernetes manifests"
    exit 1
fi

# Wait for deployment to be ready
print_status "Waiting for deployment to be ready..."
kubectl rollout status deployment/paygress-plugin -n paygress-system --timeout=300s

# Get service information
print_success "Deployment completed successfully!"
echo ""
echo "📋 Service Information:"
kubectl get pods,svc -n paygress-system
echo ""
echo "🔍 To check logs:"
echo "kubectl logs -f deployment/paygress-plugin -n paygress-system"
echo ""
echo "🧪 To test the plugin:"
echo "kubectl port-forward -n paygress-system svc/paygress-plugin 8080:8080"
echo "Then visit: http://localhost:8080/healthz"
echo ""
echo "🌐 Your ingress is configured for: api.example.com"
echo "Update your /etc/hosts or DNS to point api.example.com to your ingress IP"

# Show ingress status
echo ""
echo "📡 Ingress Status:"
kubectl get ingress paygress-example -n default
