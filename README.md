# Paygress - Nostr-Based Pod Provisioning

🔧 **Nostr Events → Kubernetes Pod Provisioning with Cashu Payments**

## Architecture

**Nostr-Driven Pod Provisioning:**
- Service listens for Nostr events (kind 1000) with Cashu tokens
- Automatically provisions SSH pods in Kubernetes with Tor onion access
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

# Install Tor for onion access
sudo apt install -y tor torsocks

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
- ✅ **No health checks** - No kubectl port-forward needed
- ✅ **Tor onion access** - Direct SSH via Tor
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
cashu mint 1000 --url https://nofees.testnut.cashu.space

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
# - onion_address: xxxxxxxx.onion (if Tor is working)
```

## 🔑 Step 7: Access Your Pod

You'll receive a response with SSH access details. You have **two options**:

### **Option A: Tor Onion Access (Recommended) 🌐**

**Benefits:**
- ✅ No public IP address required
- ✅ No kubectl or Kubernetes access needed
- ✅ NAT traversal handled by Tor
- ✅ Decentralized access through onion routing
- ✅ Works from anywhere with Tor installed
- ✅ **Fully automatic** - onion address provided in response

```bash
# Connect directly using the onion address from Nostr response
torsocks ssh -o PreferredAuthentications=password -o PubkeyAuthentication=no alice@<onion_address>.onion -p 2222
# Password: <from_nostr_response>
```

### **Option B: Traditional Port Forward**

```bash
# Port forward to SSH service (use pod name from response)
kubectl -n user-workloads port-forward svc/ssh-pod-<pod-id>-ssh 2222:2222

# SSH to the pod (use credentials from Nostr response)
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

# Check Tor sidecar logs
kubectl logs -n user-workloads ssh-pod-<pod-id> -c tor-sidecar
```

### Check Service Status
```bash
# Check sidecar service logs
kubectl logs -n ingress-system -l app=paygress-sidecar

# Check service health
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
3. **Service processes** → Verifies payment, creates SSH pod in Kubernetes with Tor sidecar
4. **Service replies** → Kind 1001 event with SSH access details (both traditional and Tor onion)
5. **User accesses pod** → Uses provided SSH credentials via port-forward or Tor onion

**Complete Nostr-based workflow - no HTTP endpoints needed!**

## Decentralized Architecture

- **Nostr Events**: All communication via decentralized relay network
- **Cashu Payments**: Bitcoin-based e-cash for payments
- **Tor Onion Services**: SSH access without public IP addresses
- **Kubernetes**: Container orchestration for pod management

**No centralized dependencies - fully decentralized pod provisioning!**

## 🔧 Troubleshooting

### Common Issues

#### 1. **Tor Onion Address Not Generated**
If you see errors like:
```
ERROR:root:Fail to setup from SERVICE_PORTS environment
ERROR:root:Ports.__init__() missing 1 required positional argument: 'dest'
No onion site
```

**Solution:**
```bash
# Update the configuration with the new Tor image
kubectl apply -f k8s/sidecar-service.yaml

# Restart the sidecar service
kubectl -n ingress-system rollout restart deploy/paygress-sidecar

# Wait for it to be ready
kubectl -n ingress-system rollout status deploy/paygress-sidecar

# Delete old pods with broken Tor configuration
kubectl -n user-workloads delete pod ssh-pod-<old-pod-id>
kubectl -n user-workloads delete svc ssh-pod-<old-pod-id>-ssh
kubectl -n user-workloads delete configmap ssh-pod-<old-pod-id>-tor-config

# Create a new pod with the updated configuration
# (Send a new Nostr request)
```

#### 2. **Pod Creation Fails**
```bash
# Check service account permissions
kubectl auth can-i create pods --as=system:serviceaccount:ingress-system:paygress-sidecar -n user-workloads

# Check logs
kubectl logs -n ingress-system -l app=paygress-sidecar
kubectl describe pod -n ingress-system -l app=paygress-sidecar
```

#### 3. **SSH Connection Fails**
```bash
# Check if pod is running
kubectl get pods -n user-workloads
kubectl get svc -n user-workloads

# Check pod logs
kubectl logs -n user-workloads ssh-pod-<pod-id>

# Check Tor sidecar logs
kubectl logs -n user-workloads ssh-pod-<pod-id> -c tor-sidecar
```

#### 4. **Payment Verification Fails**
```bash
# Check if mint is accessible
curl https://nofees.testnut.cashu.space/info

# Check Cashu database
kubectl exec -n ingress-system deployment/paygress-sidecar -- ls -la /app/data/

# Verify token manually
curl -X GET "http://localhost:8080/auth?token=YOUR_TOKEN&duration_minutes=60"
```

#### 5. **Nak Commands Not Working**
```bash
# Check if nak is installed
which nak
nak --version

# Test with a simple event
nak event --kind 1 --content "test" --sec $NSEC_HEX wss://relay.damus.io

# Check if relays are accessible
curl -I wss://relay.damus.io
```

## 🎬 Complete Example Workflow

Here's a complete example from start to finish:

### 1. **Setup (One-time)**
```bash
# Install everything
curl -LO https://storage.googleapis.com/minikube/releases/latest/minikube-linux-amd64
sudo install minikube-linux-amd64 /usr/local/bin/minikube
curl -LO "https://dl.k8s.io/release/$(curl -L -s https://dl.k8s.io/release/stable.txt)/bin/linux/amd64/kubectl"
sudo install -o root -g root -m 0755 kubectl /usr/local/bin/kubectl
sudo apt update && sudo apt install -y docker.io jq golang tor torsocks
sudo systemctl start docker
sudo usermod -aG docker $USER
go install github.com/fiatjaf/nak@latest
pip install cashu
echo 'export PATH="$HOME/go/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc
```

### 2. **Start and Deploy**
```bash
# Start Minikube
minikube start --memory=4096 --cpus=2

# Deploy Paygress
git clone <your-repo-url>
cd paygress

# Build and deploy
docker build -t paygress:latest .
minikube image load paygress:latest
kubectl apply -f k8s/sidecar-service.yaml

# Wait for deployment
kubectl wait --for=condition=available --timeout=300s \
    deployment/paygress-sidecar -n ingress-system
```

### 3. **Get Payment Token**
```bash
# Get 1000 sats (1000 minutes = ~16 hours)
cashu mint 1000 --url https://nofees.testnut.cashu.space
# Output: cashuAeyJ0b2tlbiI6W3sibWludCI6Imh0dHA...
```

### 4. **Send Request and Get Response**
```bash
# Generate keys
export NSEC_HEX=$(openssl rand -hex 32)
echo "Private key: $NSEC_HEX"

# Send request
nak event \
  --kind 1000 \
  --content '{"cashu_token":"cashuAeyJ0b2tlbiI6W3sibWludCI6Imh0dHA...","ssh_username":"alice"}' \
  --sec $NSEC_HEX \
  wss://relay.damus.io wss://nos.lol wss://relay.nostr.band

# Save the event ID from response: {"id":"abc123...","event":{...}}

# Listen for response
REQ_ID="abc123..."
nak req -k 1001 -e $REQ_ID --stream wss://relay.damus.io wss://nos.lol wss://relay.nostr.band
```

### 5. **Connect to Pod**
```bash
# From the response, you'll get:
# - pod_name: ssh-pod-xxxxx
# - ssh_username: alice
# - ssh_password: xxxxxxxx
# - onion_address: xxxxxxxx.onion

# Connect via Tor (recommended)
torsocks ssh -o PreferredAuthentications=password -o PubkeyAuthentication=no alice@xxxxxxx.onion -p 2222
# Password: xxxxxxxx

# Or connect via port-forward
kubectl -n user-workloads port-forward svc/ssh-pod-xxxxx-ssh 2222:2222 &
ssh -o PreferredAuthentications=password -o PubkeyAuthentication=no alice@localhost -p 2222
# Password: xxxxxxxx
```

### Expected Tor Logs (Working Configuration)
When Tor is working correctly, you should see:
```
[notice] Read configuration file "/etc/tor/torrc".
[notice] Opening Socks listener on 127.0.0.1:9050
[notice] Opening OR listener on 0.0.0.0:9001
[notice] Bootstrapped 100% (done): Done
[notice] Tor has successfully opened a circuit. Looks like client functionality is working.
[notice] New control connection opened.
[notice] New control connection closed.
[notice] Opening Socks listener on 127.0.0.1:9050
[notice] Opening OR listener on 0.0.0.0:9001
[notice] Bootstrapped 100% (done): Done
[notice] Tor has successfully opened a circuit. Looks like client functionality is working.
[notice] New control connection opened.
[notice] New control connection closed.
```

### Verification Commands
```bash
# Check if onion address exists
kubectl exec -n user-workloads ssh-pod-<pod-id> -c tor-sidecar -- cat /var/lib/tor/hidden_service/hostname

# Check Tor configuration
kubectl exec -n user-workloads ssh-pod-<pod-id> -c tor-sidecar -- cat /etc/tor/torrc

# Test SSH connection
kubectl exec -n user-workloads ssh-pod-<pod-id> -c ssh-server -- ps aux | grep ssh
```

## ⚙️ **Configuration**

### **Environment Variables**

| Variable | Default | Description |
|----------|---------|-------------|
| `RUN_MODE` | `nostr` | Service mode: `nostr` or `http` |
| `NOSTR_RELAYS` | `wss://relay.damus.io,wss://nos.lol,wss://relay.nostr.band` | Comma-separated list of Nostr relays |
| `WHITELISTED_MINTS` | `https://mint.cashu.space,https://mint.f7z.io,https://legend.lnbits.com/cashu/api/v1` | Comma-separated list of allowed Cashu mint URLs |
| `ENABLE_TOR_SIDECAR` | `true` | Enable Tor onion service for SSH access |
| `TOR_IMAGE` | `dperson/torproxy:latest` | Tor container image |
| `POD_NAMESPACE` | `user-workloads` | Kubernetes namespace for SSH pods |
| `PAYMENT_RATE_SATS_PER_HOUR` | `100` | Payment rate in satoshis per hour |

### **Custom Relay Configuration**

```bash
# Use custom relays
kubectl -n ingress-system set env deploy/paygress-sidecar NOSTR_RELAYS="wss://your-relay.com,wss://another-relay.com"

# Disable Tor (fallback to port-forward)
kubectl -n ingress-system set env deploy/paygress-sidecar ENABLE_TOR_SIDECAR=false

# Update Tor image
kubectl -n ingress-system set env deploy/paygress-sidecar TOR_IMAGE=dperson/torproxy:latest

# Configure whitelisted Cashu mints
kubectl -n ingress-system set env deploy/paygress-sidecar WHITELISTED_MINTS="https://mint.cashu.space,https://mint.f7z.io,https://your-mint.com"
```
