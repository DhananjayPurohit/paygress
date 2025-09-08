#!/bin/bash

# Paygress Sidecar Deployment Script
set -e

# Colors for output
BLUE='\033[0;34m'
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${BLUE}🚀 Paygress Sidecar Deployment${NC}"

# Check if Minikube is running
echo -e "${BLUE}Step 1: Checking Minikube status...${NC}"
if ! minikube status &> /dev/null; then
    echo -e "${RED}Minikube is not running. Starting Minikube...${NC}"
    minikube start || { echo -e "${RED}Failed to start Minikube. Exiting.${NC}"; exit 1; }
else
    echo -e "${GREEN}✅ Minikube is running${NC}"
fi

# Build the sidecar image
echo -e "${BLUE}Step 2: Building Paygress sidecar image...${NC}"
eval $(minikube docker-env)
docker build -t paygress-sidecar:latest -f Dockerfile.sidecar . || {
    echo -e "${RED}Failed to build sidecar image. Exiting.${NC}"
    exit 1
}
eval $(minikube docker-env -u)
echo -e "${GREEN}✅ Sidecar image built successfully${NC}"

# Deploy to Kubernetes
echo -e "${BLUE}Step 3: Deploying to Kubernetes...${NC}"
kubectl apply -f k8s-local.yaml || {
    echo -e "${RED}Failed to apply k8s manifests. Exiting.${NC}"
    exit 1
}

kubectl apply -f ingress.yaml || {
    echo -e "${RED}Failed to apply ingress. Exiting.${NC}"
    exit 1
}

# Wait for deployment
echo -e "${BLUE}Step 4: Waiting for deployment to be ready...${NC}"
kubectl wait --for=condition=ready pod -l app=paygress-nginx-ingress-controller -n paygress-test --timeout=120s || {
    echo -e "${RED}Deployment timed out. Checking logs...${NC}"
    kubectl logs -l app=paygress-nginx-ingress-controller -n paygress-test -c paygress-sidecar --tail=10
    exit 1
}

echo -e "${GREEN}✅ Deployment ready!${NC}"

# Show pod status
echo -e "${BLUE}Step 5: Checking pod status...${NC}"
kubectl get pods -n paygress-test

# Check sidecar logs
echo -e "${BLUE}Step 6: Checking sidecar logs...${NC}"
kubectl logs -l app=paygress-nginx-ingress-controller -n paygress-test -c paygress-sidecar --tail=10

# Start port forwarding in background
echo -e "${BLUE}Step 7: Setting up port forwarding...${NC}"
pkill -f "kubectl port-forward.*8080" 2>/dev/null || true
kubectl port-forward svc/ingress-nginx 8080:80 -n paygress-test >/dev/null 2>&1 &
PORT_FORWARD_PID=$!

# Wait a moment for port forwarding to start
sleep 3

echo -e "${GREEN}🎉 Paygress Sidecar is ready for testing!${NC}"
echo
echo -e "${YELLOW}Testing URLs:${NC}"
echo -e "${BLUE}Health Check:${NC} curl http://localhost:8080/health"
echo -e "${BLUE}No Payment:${NC}   curl -H 'Host: localhost' http://localhost:8080/premium -v"
echo -e "${BLUE}Valid Payment:${NC} curl -H 'Host: localhost' -H 'Authorization: Bearer cashu_token_1000_demo' http://localhost:8080/premium -v"
echo
echo -e "${YELLOW}Logs:${NC}"
echo -e "${BLUE}Sidecar logs:${NC} kubectl logs -l app=paygress-nginx-ingress-controller -n paygress-test -c paygress-sidecar -f"
echo -e "${BLUE}NGINX logs:${NC}   kubectl logs -l app=paygress-nginx-ingress-controller -n paygress-test -c nginx-ingress-controller -f"
echo
echo -e "${YELLOW}To stop port forwarding:${NC} kill $PORT_FORWARD_PID"
echo -e "${YELLOW}To cleanup:${NC} kubectl delete namespace paygress-test"
