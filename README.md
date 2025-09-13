# Paygress - Nostr-Based Pod Provisioning

🔧 **Nostr Events → Kubernetes Pod Provisioning with Cashu Payments**

## Architecture

**Nostr-Driven Pod Provisioning:**
- Service listens for Nostr events (kind 1000) with Cashu tokens
- Automatically provisions SSH pods in Kubernetes
- Replies with access details via Nostr events (kind 1001)
- Fully decentralized - no HTTP endpoints needed

## 🚀 Complete Setup Guide

### Prerequisites

#### 1. Install Minikube
```bash
# Install Minikube
curl -LO https://storage.googleapis.com/minikube/releases/latest/minikube-linux-amd64
sudo install minikube-linux-amd64 /usr/local/bin/minikube

# Install kubectl
curl -LO "https://dl.k8s.io/release/$(curl -L -s https://dl.k8s.io/release/stable.txt)/bin/linux/amd64/kubectl"
sudo install -o root -g root -m 0755 kubectl /usr/local/bin/kubectl

# Install Docker
sudo apt update && sudo apt install -y docker.io
sudo systemctl start docker
sudo usermod -aG docker $USER
# Log out and back in for group changes to take effect
```

#### 2. Install Required Tools
```bash
# Install jq for JSON parsing
sudo apt install -y jq

# Install Go and Nak (Nostr CLI)
sudo apt install -y golang
go install github.com/fiatjaf/nak@latest
echo 'export PATH="$HOME/go/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc

# Install Cashu CLI for payments
pip install cashu
```

### Step 1: Start Minikube
```bash
# Start Minikube with sufficient resources
minikube start --memory=4096 --cpus=2

# Verify cluster is running
kubectl cluster-info
kubectl get nodes
```

### Step 2: Deploy Paygress
```bash
# Clone and navigate to project
git clone <your-repo-url>
cd paygress

# Build Docker image
docker build -t paygress:latest .

# Load image into Minikube
minikube image load paygress:latest

# Deploy to Kubernetes
kubectl apply -f k8s/sidecar-service.yaml

# Wait for deployment to be ready
kubectl wait --for=condition=available --timeout=300s \
    deployment/paygress-sidecar -n ingress-system
```

### Step 3: Verify Deployment
```bash
# Check if everything is running
kubectl get pods -n ingress-system
kubectl get svc -n ingress-system

# Check logs
kubectl logs -n ingress-system -l app=paygress-sidecar

# Test the service
kubectl port-forward -n ingress-system svc/paygress-sidecar 8080:8080 &
curl http://localhost:8080/healthz
```

## 🎛️ **Deployment Modes**

The service supports two modes via `RUN_MODE` environment variable:

### **Nostr Mode** (Default: `RUN_MODE=nostr`)
- ✅ **Fully decentralized** - No HTTP endpoints
- ✅ **Nostr events only** - All communication via relays
- ✅ **Configurable relays** - Choose your preferred Nostr relays

### **HTTP Mode** (`RUN_MODE=http`)
- ✅ **Traditional REST API** - Standard HTTP endpoints
- ✅ **Health checks enabled** - Kubernetes health monitoring
- ✅ **Port forwarding** - SSH via kubectl port-forward
- ✅ **Ingress integration** - Works with existing ingress controllers

## 💰 Step 4: Get Cashu Tokens

### Get Test Tokens
```bash
# Get tokens from test mint (1000 sats = 1000 minutes = ~16 hours)
cashu mint 1000 --url https://mint.cashu.space

# This will output a Cashu token like:
# cashuAeyJ0b2tlbiI6W3sibWludCI6Imh0dHA...
```

## 📡 Step 5: Send Nostr Request

### Generate Nostr Keys
```bash
# Generate a private key
export NSEC_HEX=$(openssl rand -hex 32)
echo "Your private key: $NSEC_HEX"

# Get the public key (optional, for reference)
export NPUB_HEX=$(echo $NSEC_HEX | xxd -r -p | sha256sum | cut -c1-64)
echo "Your public key: $NPUB_HEX"
```

### Send Pod Request
```bash
# Send provisioning request (kind 1000)
# Replace <YOUR_CASHU_TOKEN> with the token from Step 4
nak event \
  --kind 1000 \
  --content '{"cashu_token":"<YOUR_CASHU_TOKEN>","ssh_username":"alice","pod_image":"linuxserver/openssh-server:latest"}' \
  --sec $NSEC_HEX \
  wss://relay.damus.io wss://nos.lol wss://relay.nostr.band

# Save the event ID from the response!
# Example response: {"id":"abc123...","event":{...}}
```

## 🎯 Step 6: Listen for Response

```bash
# Listen for response (kind 1001) - use the event ID from above
REQ_ID="<event_id_from_above>"
nak req -k 1001 -e $REQ_ID --stream wss://relay.damus.io wss://nos.lol wss://relay.nostr.band

# You should see a response with SSH access details including:
# - pod_name: ssh-pod-xxxxx
# - ssh_username: alice
# - ssh_password: xxxxxxxx
# - node_port: 3xxxx
```

## 🔑 Step 7: Access Your Pod

You'll receive SSH access details with two connection options:

### **Option 1: Direct SSH Access (Recommended)**
```bash
# Connect directly via NodePort (no kubectl needed)
ssh -o PreferredAuthentications=password -o PubkeyAuthentication=no alice@$(minikube ip) -p <node_port>
# Password: <from_nostr_response>
```

### **Option 2: Port Forward**
```bash
# Port forward to SSH service
kubectl -n user-workloads port-forward svc/ssh-pod-<pod-id>-ssh 2222:2222

# SSH to the pod
ssh -o PreferredAuthentications=password -o PubkeyAuthentication=no alice@localhost -p 2222
# Password: <from_nostr_response>
```

## 🔧 Step 8: Monitor and Manage

### Check Active Pods
```bash
# List all active pods
kubectl get pods -n user-workloads -l app=paygress-ssh-pod

# Check specific pod logs
kubectl logs -n user-workloads ssh-pod-<pod-id>

# Check service logs
kubectl logs -n ingress-system -l app=paygress-sidecar
```

### Check Service Status
```bash
# Check sidecar service logs
kubectl logs -n ingress-system -l app=paygress-sidecar

# Check service health (HTTP mode only)
kubectl port-forward -n ingress-system svc/paygress-sidecar 8080:8080 &
curl http://localhost:8080/healthz
```

## 🔄 Updating the Deployment

When you make changes to the code, update the deployment:

```bash
# Rebuild the Docker image
docker build -t paygress:latest .

# Load the new image into Minikube
minikube image load paygress:latest

# Restart the deployment to use the new image
kubectl -n ingress-system rollout restart deploy/paygress-sidecar

# Wait for the rollout to complete
kubectl -n ingress-system rollout status deploy/paygress-sidecar

# Check the new pod is running
kubectl get pods -n ingress-system -l app=paygress-sidecar
```

## 🧹 Cleanup

To remove the Paygress deployment:

```bash
# Delete the deployment and related resources
kubectl delete -f k8s/sidecar-service.yaml

# Delete any remaining SSH pods
kubectl delete pods -n user-workloads -l app=paygress-ssh-pod

# Stop Minikube (optional)
minikube stop
```

## Files

- `src/main.rs` - Main service with Nostr mode
- `src/nostr.rs` - Nostr client for publishing/listening
- `src/sidecar_service.rs` - Kubernetes pod provisioning
- `src/cashu.rs` - Cashu payment verification
- `k8s/sidecar-service.yaml` - Kubernetes deployment
- `Dockerfile` - Container image

## How it works

1. **Service starts** → Connects to Nostr relays, publishes offer event
2. **User sends Nostr event** → Kind 1000 with Cashu token and pod requirements
3. **Service processes** → Verifies payment, creates SSH pod in Kubernetes
4. **Service replies** → Kind 1001 event with SSH access details
5. **User accesses pod** → Uses provided SSH credentials via NodePort or port-forward

**Complete Nostr-based workflow - no HTTP endpoints needed!**

## Decentralized Architecture

- **Nostr Events**: All communication via decentralized relay network
- **Cashu Payments**: Bitcoin-based e-cash for payments
- **Kubernetes**: Container orchestration for pod management
- **Ready for Iroh**: Prepared for peer-to-peer networking integration

**No centralized dependencies - fully decentralized pod provisioning!**

## 🔧 Troubleshooting

### **Pod Creation Fails**
```bash
# Check service account permissions
kubectl auth can-i create pods --as=system:serviceaccount:ingress-system:paygress-sidecar -n user-workloads

# Check logs
kubectl logs -n ingress-system -l app=paygress-sidecar
kubectl describe pod -n ingress-system -l app=paygress-sidecar
```

### **SSH Connection Fails**
```bash
# Check if pod is running
kubectl get pods -n user-workloads
kubectl get svc -n user-workloads

# Check pod logs
kubectl logs -n user-workloads ssh-pod-<pod-id>
```

### **Payment Verification Fails**
```bash
# Check if mint is accessible
curl https://mint.cashu.space/info

# Check Cashu database
kubectl exec -n ingress-system deployment/paygress-sidecar -- ls -la /app/data/

# Verify token manually (HTTP mode only)
curl -X GET "http://localhost:8080/auth?token=YOUR_TOKEN&duration_minutes=60"
```

## 🎬 Quick Example

```bash
# Generate keys and get payment
export NSEC_HEX=$(openssl rand -hex 32)
cashu mint 1000 --url https://mint.cashu.space

# Send request
nak event \
  --kind 1000 \
  --content '{"cashu_token":"YOUR_TOKEN","ssh_username":"alice"}' \
  --sec $NSEC_HEX \
  wss://relay.damus.io wss://nos.lol wss://relay.nostr.band

# Listen for response
nak req -k 1001 -e $EVENT_ID --stream wss://relay.damus.io wss://nos.lol wss://relay.nostr.band

# Connect via SSH
ssh -o PreferredAuthentications=password -o PubkeyAuthentication=no alice@$(minikube ip) -p $NODE_PORT
```

## ⚙️ **Configuration**

### **Environment Variables**

| Variable | Default | Description |
|----------|---------|-------------|
| `RUN_MODE` | `nostr` | Service mode: `nostr` or `http` |
| `NOSTR_RELAYS` | `wss://relay.damus.io,wss://nos.lol,wss://relay.nostr.band` | Comma-separated list of Nostr relays |
| `WHITELISTED_MINTS` | `https://mint.cashu.space,https://mint.f7z.io,https://legend.lnbits.com/cashu/api/v1` | Comma-separated list of allowed Cashu mint URLs |
| `POD_NAMESPACE` | `user-workloads` | Kubernetes namespace for SSH pods |
| `PAYMENT_RATE_SATS_PER_HOUR` | `100` | Payment rate in satoshis per hour |
| `SSH_BASE_IMAGE` | `linuxserver/openssh-server:latest` | SSH server container image |

### **Custom Configuration**

```bash
# Use custom relays
kubectl -n ingress-system set env deploy/paygress-sidecar NOSTR_RELAYS="wss://your-relay.com,wss://another-relay.com"

# Configure whitelisted Cashu mints
kubectl -n ingress-system set env deploy/paygress-sidecar WHITELISTED_MINTS="https://mint.cashu.space,https://mint.f7z.io,https://your-mint.com"

# Update payment rate
kubectl -n ingress-system set env deploy/paygress-sidecar PAYMENT_RATE_SATS_PER_HOUR="200"
```
