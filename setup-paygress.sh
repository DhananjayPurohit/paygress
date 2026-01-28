#!/bin/bash
# Paygress - Single Script Deployment
# Complete deployment, testing, and management in one script

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

echo -e "${BLUE}╔════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║                    PAYGRESS                                ║${NC}"
echo -e "${BLUE}║  Cashu Payment Gateway for Kubernetes Pod Provisioning    ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Show usage
show_usage() {
    cat <<EOF
${CYAN}USAGE:${NC}
  $0 [COMMAND]

${CYAN}COMMANDS:${NC}
  deploy          Deploy Paygress to server (runs ansible)
  status          Check service status
  logs            View service logs
  test            Test API endpoints
  restart         Restart paygress service
  fix-k8s         Fix Kubernetes issues
  fix-pods        Fix stuck containers
  help            Show this help

${CYAN}QUICK START:${NC}
  1. Edit inventory.ini with your server details
  2. Run: ./setup-paygress.sh deploy
  3. Test: ./setup-paygress.sh test

${CYAN}PRICING:${NC}
  Basic:    100 msats/sec (1 CPU, 1GB)  - 60k msats = 10 min
  Standard: 200 msats/sec (2 CPU, 2GB)  - 60k msats = 5 min
  Premium:  400 msats/sec (4 CPU, 4GB)  - 60k msats = 2.5 min

  Formula: duration = payment_msats / tier_rate

${CYAN}EXAMPLE USAGE:${NC}
  curl -X POST http://YOUR_SERVER:8080/pods/spawn \\
    -H "Content-Type: application/json" \\
    -d '{
      "cashu_token": "cashuAeyJ0b2tlbiI6...",
      "pod_spec_id": "basic",
      "pod_image": "linuxserver/openssh-server:latest",
      "ssh_username": "user",
      "ssh_password": "password"
    }'

EOF
    exit 0
}

# Get server info from inventory
get_server_info() {
    if [ ! -f "inventory.ini" ]; then
        echo -e "${RED}❌ Error: inventory.ini not found${NC}"
        echo "Create it: cp inventory.ini.template inventory.ini"
        exit 1
    fi
    
    SERVER=$(grep "ansible_host=" inventory.ini | head -1 | sed -E 's/.*ansible_host=([^ ]+).*/\1/')
    USER=$(grep "ansible_user=" inventory.ini | head -1 | sed -E 's/.*ansible_user=([^ ]+).*/\1/')
    PUBLIC_IP=$(grep "^public_ip=" inventory.ini | cut -d'=' -f2)
    HTTP_PORT=$(grep "^http_port=" inventory.ini | cut -d'=' -f2 || echo "8080")
    
    if [ -z "$SERVER" ] || [ -z "$USER" ]; then
        echo -e "${RED}❌ Error: Could not extract server info from inventory.ini${NC}"
        exit 1
    fi
}

# Deploy with Ansible
deploy() {
    echo -e "${YELLOW}🚀 Deploying Paygress...${NC}"
    echo ""
    
    # Check Ansible (local installation - detect OS)
    if ! command -v ansible-playbook &> /dev/null; then
        echo -e "${YELLOW}📦 Installing Ansible locally...${NC}"
        if [[ "$OSTYPE" == "darwin"* ]]; then
            # macOS - use Homebrew
            if ! command -v brew &> /dev/null; then
                echo -e "${RED}❌ Homebrew not found. Please install it first:${NC}"
                echo '  /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"'
                exit 1
            fi
            brew install ansible
        else
            # Linux - use apt
            sudo apt update && sudo apt install -y ansible
        fi
    fi
    
    # Check files
    if [ ! -f "inventory.ini" ]; then
        echo -e "${RED}❌ Error: inventory.ini not found${NC}"
        echo "Create it: cp inventory.ini.template inventory.ini"
        exit 1
    fi
    
    if [ ! -f "ansible-setup.yml" ]; then
        echo -e "${RED}❌ Error: ansible-setup.yml not found${NC}"
        exit 1
    fi
    
    echo -e "${GREEN}✅ Running deployment...${NC}"
    
    # Run ansible, capture result
    if ansible-playbook -i inventory.ini ansible-setup.yml -v; then
        DEPLOYMENT_SUCCESS=true
    else
        DEPLOYMENT_SUCCESS=false
    fi
    
    echo ""
    if [ "$DEPLOYMENT_SUCCESS" = true ]; then
        echo -e "${GREEN}╔════════════════════════════════════════════════════════════╗${NC}"
        echo -e "${GREEN}║              🎉 DEPLOYMENT COMPLETE! 🎉                   ║${NC}"
        echo -e "${GREEN}╚════════════════════════════════════════════════════════════╝${NC}"
    else
        echo -e "${YELLOW}╔════════════════════════════════════════════════════════════╗${NC}"
        echo -e "${YELLOW}║           Deployment completed with warnings              ║${NC}"
        echo -e "${YELLOW}╚════════════════════════════════════════════════════════════╝${NC}"
    fi
    echo ""
    
    # Always try to fix Kubernetes issues after deployment
    echo -e "${YELLOW}Checking Kubernetes status...${NC}"
    get_server_info
    
    if ssh ${USER}@${SERVER} 'kubectl cluster-info --request-timeout=5s' &> /dev/null; then
        echo -e "${GREEN}✅ Kubernetes is working${NC}"
    else
        echo -e "${YELLOW}⚠️  Kubernetes needs fixing${NC}"
        echo ""
        echo -e "${CYAN}Running automatic fix...${NC}"
        fix_kubernetes
    fi
    
    echo ""
    echo -e "${CYAN}Next steps:${NC}"
    echo "  1. Check status: ./setup-paygress.sh status"
    echo "  2. View logs: ./setup-paygress.sh logs"
    echo "  3. Test API: ./setup-paygress.sh test"
    echo ""
}

# Check service status
check_status() {
    echo -e "${YELLOW}📊 Checking Status...${NC}"
    echo ""
    
    get_server_info
    
    echo -e "${CYAN}Server: ${SERVER}${NC}"
    echo ""
    echo -e "${BLUE}━━━ Paygress Service ━━━${NC}"
    ssh ${USER}@${SERVER} 'sudo systemctl status paygress --no-pager || true'
    echo ""
    echo -e "${BLUE}━━━ Kubernetes Status ━━━${NC}"
    ssh ${USER}@${SERVER} 'kubectl get nodes 2>/dev/null || echo "Kubernetes not accessible"'
    echo ""
    ssh ${USER}@${SERVER} 'kubectl get pods -n user-workloads 2>/dev/null || echo "No pods running"'
}

# View logs
view_logs() {
    echo -e "${YELLOW}📜 Viewing Logs...${NC}"
    echo ""
    
    get_server_info
    
    echo -e "${CYAN}Server: ${SERVER}${NC}"
    echo -e "${YELLOW}Press Ctrl+C to stop${NC}"
    echo ""
    
    ssh ${USER}@${SERVER} 'sudo journalctl -u paygress -f'
}

# Test API
test_api() {
    echo -e "${YELLOW}🧪 Testing API...${NC}"
    echo ""
    
    get_server_info
    
    API_URL="http://${PUBLIC_IP:-$SERVER}:${HTTP_PORT}"
    
    echo -e "${CYAN}Testing: ${API_URL}${NC}"
    echo ""
    
    # Test health
    echo -e "${BLUE}1. Health Check${NC}"
    if curl -s --max-time 5 "${API_URL}/health" | jq '.' 2>/dev/null; then
        echo -e "${GREEN}✅ Health OK${NC}"
    else
        echo -e "${RED}❌ Health check failed${NC}"
    fi
    echo ""
    
    # Test offers
    echo -e "${BLUE}2. Get Offers${NC}"
    if OFFERS=$(curl -s --max-time 5 "${API_URL}/offers" 2>/dev/null); then
        echo "$OFFERS" | jq '.' 2>/dev/null || echo "$OFFERS"
        echo -e "${GREEN}✅ Offers OK${NC}"
    else
        echo -e "${RED}❌ Offers failed${NC}"
    fi
    echo ""
    
    echo -e "${CYAN}API Endpoints:${NC}"
    echo "  GET  ${API_URL}/health"
    echo "  GET  ${API_URL}/offers"
    echo "  POST ${API_URL}/pods/spawn"
    echo "  POST ${API_URL}/pods/topup"
    echo "  POST ${API_URL}/pods/status"
    echo ""
    
    echo -e "${YELLOW}Example spawn request:${NC}"
    cat <<'EOF'
curl -X POST http://YOUR_SERVER:8080/pods/spawn \
  -H "Content-Type: application/json" \
  -d '{
    "cashu_token": "cashuAeyJ0b2tlbiI6...",
    "pod_spec_id": "basic",
    "pod_image": "linuxserver/openssh-server:latest",
    "ssh_username": "user",
    "ssh_password": "password"
  }'
EOF
    echo ""
}

# Restart service
restart_service() {
    echo -e "${YELLOW}🔄 Restarting Service...${NC}"
    echo ""
    
    get_server_info
    
    ssh ${USER}@${SERVER} 'sudo systemctl restart paygress'
    sleep 2
    ssh ${USER}@${SERVER} 'sudo systemctl status paygress --no-pager'
    
    echo ""
    echo -e "${GREEN}✅ Service restarted${NC}"
}

# Fix Kubernetes issues
fix_kubernetes() {
    echo -e "${YELLOW}🔧 Fixing Kubernetes...${NC}"
    echo ""
    
    get_server_info
    
    echo -e "${CYAN}Server: ${SERVER}${NC}"
    echo ""
    
    # Create inline fix script
    cat > /tmp/fix-k8s.sh <<'FIXSCRIPT'
#!/bin/bash
set -e

echo "Checking Kubernetes..."

# First, fix containerd configuration (common cause of CRI errors)
echo "Ensuring containerd is properly configured..."

# Check if containerd is installed
if ! command -v containerd &> /dev/null; then
    echo "✗ containerd not found, installing..."
    apt-get update
    apt-get install -y containerd
fi

# Create proper containerd config
mkdir -p /etc/containerd
containerd config default > /etc/containerd/config.toml

# Enable SystemdCgroup (required for Kubernetes)
sed -i 's/SystemdCgroup = false/SystemdCgroup = true/' /etc/containerd/config.toml

# Restart containerd
echo "Restarting containerd..."
systemctl restart containerd
systemctl enable containerd
sleep 5

# Verify containerd is running
if ! systemctl is-active --quiet containerd; then
    echo "✗ containerd failed to start"
    systemctl status containerd
    exit 1
fi
echo "✓ containerd is running"

# Load required kernel modules
echo "Loading kernel modules..."
modprobe overlay 2>/dev/null || true
modprobe br_netfilter 2>/dev/null || true

# Set sysctl params
cat > /etc/sysctl.d/k8s.conf <<EOF
net.bridge.bridge-nf-call-iptables = 1
net.bridge.bridge-nf-call-ip6tables = 1
net.ipv4.ip_forward = 1
EOF
sysctl --system > /dev/null 2>&1 || true

# Check if kubeadm is installed
if ! command -v kubeadm &> /dev/null; then
    echo "✗ kubeadm not found, installing Kubernetes packages..."
    
    apt-get update
    apt-get install -y apt-transport-https ca-certificates curl gpg
    
    mkdir -p /etc/apt/keyrings
    curl -fsSL https://pkgs.k8s.io/core:/stable:/v1.28/deb/Release.key | gpg --dearmor -o /etc/apt/keyrings/kubernetes-apt-keyring.gpg 2>/dev/null || true
    echo 'deb [signed-by=/etc/apt/keyrings/kubernetes-apt-keyring.gpg] https://pkgs.k8s.io/core:/stable:/v1.28/deb/ /' | tee /etc/apt/sources.list.d/kubernetes.list
    
    apt-get update
    apt-get install -y kubelet kubeadm kubectl
    apt-mark hold kubelet kubeadm kubectl
    
    echo "✓ Kubernetes packages installed"
fi

# Check if API server is accessible
if kubectl cluster-info --request-timeout=5s &> /dev/null; then
    echo "✓ Kubernetes is working"
    exit 0
fi

echo "✗ Kubernetes API not accessible, reinitializing..."

# Reset and reinitialize
kubeadm reset --force 2>/dev/null || true
rm -rf /etc/cni/net.d ~/.kube
iptables -F && iptables -t nat -F && iptables -t mangle -F && iptables -X 2>/dev/null || true

# Make sure containerd is still running after reset
systemctl restart containerd
sleep 3

echo "Initializing Kubernetes..."
kubeadm init --pod-network-cidr=10.244.0.0/16 --ignore-preflight-errors=all

# Setup kubeconfig
mkdir -p ~/.kube
cp /etc/kubernetes/admin.conf ~/.kube/config
chown $(id -u):$(id -g) ~/.kube/config

# Install Flannel CNI
echo "Installing Flannel CNI..."
kubectl apply -f https://github.com/flannel-io/flannel/releases/latest/download/kube-flannel.yml

# Remove taints
sleep 10
kubectl taint nodes --all node-role.kubernetes.io/control-plane- 2>/dev/null || true

# Create namespace
kubectl create namespace user-workloads 2>/dev/null || true

echo "✓ Kubernetes fixed!"
kubectl get nodes
FIXSCRIPT
    
    # Upload and run
    scp /tmp/fix-k8s.sh ${USER}@${SERVER}:/tmp/
    ssh ${USER}@${SERVER} 'sudo bash /tmp/fix-k8s.sh'
    
    echo ""
    echo -e "${GREEN}✅ Kubernetes fixed${NC}"
    echo ""
    echo -e "${CYAN}Now restart paygress:${NC}"
    echo "  ./setup-paygress.sh restart"
}

# Fix stuck pods
fix_pods() {
    echo -e "${YELLOW}🔧 Fixing Stuck Pods...${NC}"
    echo ""
    
    get_server_info
    
    echo -e "${CYAN}Server: ${SERVER}${NC}"
    echo ""
    
    cat > /tmp/fix-pods.sh <<'FIXPODS'
#!/bin/bash
set -e

echo "Current pod status:"
kubectl get pods -n user-workloads

echo ""
echo "Restarting container runtime..."
systemctl restart containerd
sleep 5
systemctl restart kubelet
sleep 10

echo ""
echo "Deleting stuck pods..."
kubectl delete pod --all -n user-workloads --force --grace-period=0 2>/dev/null || true

echo ""
echo "Waiting for cleanup..."
sleep 15

echo ""
echo "Final status:"
kubectl get pods -n user-workloads
kubectl get nodes

echo ""
echo "✓ Pods cleaned up!"
echo "Try creating a new pod now."
FIXPODS
    
    scp /tmp/fix-pods.sh ${USER}@${SERVER}:/tmp/
    ssh ${USER}@${SERVER} 'sudo bash /tmp/fix-pods.sh'
    
    echo ""
    echo -e "${GREEN}✅ Pods fixed${NC}"
    echo ""
    echo -e "${CYAN}Restart paygress and try again:${NC}"
    echo "  ./setup-paygress.sh restart"
}

# Parse command
ACTION="${1:-help}"

case "$ACTION" in
    deploy)
        deploy
        ;;
    status)
        check_status
        ;;
    logs|log)
        view_logs
        ;;
    test)
        test_api
        ;;
    restart)
        restart_service
        ;;
    fix-k8s|fix)
        fix_kubernetes
        ;;
    fix-pods|fixpods)
        fix_pods
        ;;
    help|-h|--help)
        show_usage
        ;;
    *)
        echo -e "${RED}❌ Unknown command: $ACTION${NC}"
        echo ""
        show_usage
        ;;
esac
