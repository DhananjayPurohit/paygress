#!/bin/bash
set -e

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}Starting Paygress Stack...${NC}"

# Check for Docker
if ! command -v docker &> /dev/null; then
    echo "Error: docker is not installed."
    exit 1
fi

# Build and Start
echo "Building and starting containers..."
docker compose up -d --build --remove-orphans

echo -e "${GREEN}Stack is starting!${NC}"
echo "Waiting for Kubernetes (k3s) to define kubeconfig..."
sleep 5

# Optional: Wait loop for kubeconfig
while [ ! -f "$(docker compose run --rm --entrypoint ls paygress /kubeconfig/kubeconfig.yaml 2>/dev/null)" ] && [ -z "$(docker compose ps -q k3s)" ]; do
    echo "Waiting for k3s initialization..."
    sleep 3
done

echo -e "${GREEN}Paygress is running!${NC}"
echo -e "${BLUE}Logs:${NC} docker compose logs -f paygress"
