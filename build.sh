#!/bin/bash

echo "🔧 Building NGINX Ingress with Paygress Plugin"
echo "=============================================="

# Build the Docker image
echo "Building Docker image..."
docker build -f Dockerfile.nginx-ingress -t paygress-nginx-ingress:latest .

if [ $? -eq 0 ]; then
    echo "✅ Docker image built: paygress-nginx-ingress:latest"
    echo
    echo "🚀 Deploy to Kubernetes:"
    echo "kubectl apply -f ingress.yaml"
    echo
    echo "🧪 Test:"
    echo "curl -H 'Authorization: Bearer 1000sat-token' http://api.example.com/premium"
else
    echo "❌ Build failed"
    exit 1
fi
