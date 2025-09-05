#!/bin/bash

echo "🚀 Deploying Paygress to Local Kubernetes"
echo "========================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check if kubectl is available
if ! command -v kubectl &> /dev/null; then
    echo -e "${RED}❌ kubectl is not installed. Please install kubectl first.${NC}"
    exit 1
fi

# Check if Docker is running
if ! docker info &> /dev/null; then
    echo -e "${RED}❌ Docker is not running. Please start Docker first.${NC}"
    exit 1
fi

echo -e "${BLUE}Step 1: Building Paygress NGINX Ingress Controller image...${NC}"
docker build -t paygress-nginx-ingress:latest -f Dockerfile.nginx-ingress .
if [ $? -ne 0 ]; then
    echo -e "${RED}❌ Failed to build Docker image${NC}"
    exit 1
fi
echo -e "${GREEN}✅ Docker image built successfully${NC}"

echo -e "${BLUE}Step 2: Deploying to Kubernetes...${NC}"
kubectl apply -f k8s-local.yaml
if [ $? -ne 0 ]; then
    echo -e "${RED}❌ Failed to deploy to Kubernetes${NC}"
    exit 1
fi
echo -e "${GREEN}✅ Kubernetes resources deployed${NC}"

echo -e "${BLUE}Step 3: Deploying Paygress Ingress...${NC}"
kubectl apply -f ingress.yaml
if [ $? -ne 0 ]; then
    echo -e "${RED}❌ Failed to deploy ingress${NC}"
    exit 1
fi
echo -e "${GREEN}✅ Ingress deployed${NC}"

echo -e "${BLUE}Step 4: Waiting for pods to be ready...${NC}"
kubectl wait --for=condition=ready pod -l app=paygress-nginx-ingress-controller -n paygress-test --timeout=120s
kubectl wait --for=condition=ready pod -l app=test-backend -n paygress-test --timeout=60s

echo -e "${GREEN}🎉 Deployment completed!${NC}"
echo ""
echo -e "${YELLOW}📋 Access Information:${NC}"
echo -e "• Ingress Controller: http://localhost:30080"
echo -e "• HTTPS: https://localhost:30443"
echo -e "• Premium Endpoint: http://localhost:30080/premium"
echo ""
echo -e "${YELLOW}🧪 Testing Commands:${NC}"
echo -e "• Test without payment: ${BLUE}curl http://localhost:30080/premium${NC}"
echo -e "• Test with invalid token: ${BLUE}curl -H 'Authorization: Bearer invalid' http://localhost:30080/premium${NC}"
echo -e "• Test with valid token: ${BLUE}curl -H 'Authorization: Bearer cashu_token_1000_sats_demo' http://localhost:30080/premium${NC}"
echo ""
echo -e "${YELLOW}📊 Monitoring Commands:${NC}"
echo -e "• Check pods: ${BLUE}kubectl get pods -n paygress-test${NC}"
echo -e "• Check logs: ${BLUE}kubectl logs -f deployment/paygress-nginx-ingress-controller -n paygress-test${NC}"
echo -e "• Check ingress: ${BLUE}kubectl get ingress -n paygress-test${NC}"
echo ""
echo -e "${YELLOW}🗑️  Cleanup:${NC}"
echo -e "• Remove everything: ${BLUE}kubectl delete namespace paygress-test${NC}"
