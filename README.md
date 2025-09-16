# Paygress - Nostr-Based Pod Provisioning

🔧 **Nostr Events → Kubernetes Pod Provisioning with Cashu Payments**

## Architecture

**Encrypted Nostr-Driven Pod Provisioning:**
- Service listens for **encrypted** Nostr events (kind 1000) with Cashu tokens
- All sensitive data encrypted using NIP-44 (Cashu tokens, SSH credentials)
- Automatically provisions SSH pods in Kubernetes with `activeDeadlineSeconds`
- Replies with **encrypted** access details via Nostr events (kind 1001)
- **Top-up Support**: Extend pod duration via Nostr (kind 1002) or HTTP
- **Kubernetes Native**: Uses `activeDeadlineSeconds` for automatic pod termination
- Fully decentralized - no HTTP endpoints needed
- **End-to-end encryption** - only you and the service can decrypt data

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

### Step 2: Configure Environment Variables

Create a `.env` file with your configuration:

```bash
# Create your configuration file
cat > paygress.env << EOF
# Service Configuration
RUN_MODE=nostr
BIND_ADDR=0.0.0.0:8080
CASHU_DB_PATH=/app/data/cashu.db
POD_NAMESPACE=user-workloads
PAYMENT_RATE_SATS_PER_HOUR=100
DEFAULT_POD_DURATION_MINUTES=60
ENABLE_CLEANUP_TASK=true
RUST_LOG=info

# Nostr Configuration
NOSTR_RELAYS=wss://relay.damus.io,wss://nos.lol,wss://relay.nostr.band
NOSTR_PRIVATE_KEY=

# Cashu Configuration
WHITELISTED_MINTS=https://nofees.testnut.cashu.space,https://testnut.cashu.space

# SSH Pod Configuration
SSH_BASE_IMAGE=linuxserver/openssh-server:latest
SSH_PORT=2222
SSH_HOST=localhost

EOF
```

### Step 3: Deploy Paygress

```bash
# Clone and navigate to project
git clone <your-repo-url>
cd paygress

# Build Docker image
docker build -t paygress:latest .

# Load image into Minikube
minikube image load paygress:latest

# Deploy to Kubernetes (creates namespace and all resources)
kubectl apply -f k8s/sidecar-service.yaml

# Create ConfigMap from your .env file (after deployment)
kubectl create configmap paygress-sidecar-config \
    --from-env-file=paygress.env \
    --namespace=ingress-system \
    --dry-run=client -o yaml | kubectl apply -f -

# Restart deployment to pick up new configuration
kubectl rollout restart deployment/paygress-sidecar -n ingress-system

# Wait for deployment to be ready
kubectl wait --for=condition=available --timeout=300s \
    deployment/paygress-sidecar -n ingress-system

# Get the service's public key (you'll need this for encryption)
kubectl logs -n ingress-system -l app=paygress-sidecar
```

### Step 4: Update Configuration

To update your configuration, simply modify the `paygress.env` file and reapply:

```bash
# 1. Update your configuration in paygress.env file
nano paygress.env

# 2. Reapply the ConfigMap (updates the configuration)
kubectl create configmap paygress-sidecar-config \
    --from-env-file=paygress.env \
    --namespace=ingress-system \
    --dry-run=client -o yaml | kubectl apply -f -

# 3. Restart deployment to pick up new configuration
kubectl rollout restart deployment/paygress-sidecar -n ingress-system
```

**Note**: The order is important - always create/update the ConfigMap first, then restart the deployment so it picks up the new configuration.

### Step 5: Verify Deployment
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

## 💰 Step 6: Get Cashu Tokens

### Get Test Tokens
```bash
# Get tokens from test mint (1000 sats = 1000 minutes = ~16 hours)
cashu mint 1000 --url https://mint.cashu.space

# This will output a Cashu token like:
# cashuAeyJ0b2tlbiI6W3sibWludCI6Imh0dHA...
```

## 📡 Step 7: Send Encrypted Nostr Request

### Understanding Key Formats

**For user configuration, we'll use bech32 format (`nsec1...` and `npub1...`) which is the standard in the Nostr ecosystem:**

```bash
# Bech32 format (user-friendly, standard format)
PRIVATE_KEY="nsec1abc123..."  # Private key with nsec1 prefix
PUBLIC_KEY="npub1def456..."   # Public key with npub1 prefix

# nak can work with both bech32 and raw hex formats
# We'll use bech32 for all user-facing operations
```

### Deriving Public Key from Private Key

**If you only have a private key, you can derive the public key:**

```bash
# From bech32 private key (nsec1...) - convert to hex first
NSEC="nsec1abc123..."
# Note: nak key public needs hex, so we need to decode bech32 first
# For now, we'll work with hex keys for nak operations

# From raw hex private key - this works directly
PRIVATE_HEX="f2cbda3e2094446a232fb3fff285091314167271ff3130e7f6a663528165d4662"
npub=$(nak key public "$PRIVATE_HEX")
echo "Public key (hex): $npub"

# Convert to bech32 format
npub_bech32=$(echo "$npub" | nak encode npub)
echo "Public key (bech32): $npub_bech32"
```

### Generate User Keys with nak
```bash
# Generate user keys using nak (safer - no manual key handling)
hex=$(nak key generate)
echo "hex: $hex"

# Convert hex to bech32 format (nsec1...)
nsec=$(echo "$hex" | nak encode nsec)
echo "nsec: $nsec"

# Get public key from private key (use hex, not bech32)
npub=$(nak key public "$hex")
echo "npub: $npub"

# Convert public key to bech32 format
npub_bech32=$(echo "$npub" | nak encode npub)
echo "npub (bech32): $npub_bech32"

# Store your keys in bech32 format (user-friendly)
export NSEC="$nsec"  # Your private key (bech32 format)
export NPUB="$npub_bech32"  # Your public key (bech32 format)

# Verify your keys work
echo "Your private key: $NSEC"
echo "Your public key: $NPUB"
```

### Working with Existing Keys

**If you already have a private key (from another source):**

```bash
# If you have a bech32 private key (nsec1...) - recommended for storage
EXISTING_NSEC="nsec1abc123..."
# Note: nak key public needs hex, so we need to decode bech32 first
# For now, we'll work with hex keys for nak operations

# If you have a raw hex private key, this works directly
EXISTING_PRIVATE_HEX="f2cbda3e2094446a232fb3fff285091314167271ff3130e7f6a663528165d4662"
npub=$(nak key public "$EXISTING_PRIVATE_HEX")
echo "Public key (hex): $npub"

# Convert to bech32 format for user-friendly storage
nsec=$(echo "$EXISTING_PRIVATE_HEX" | nak encode nsec)
npub_bech32=$(echo "$npub" | nak encode npub)

export NSEC="$nsec"
export NPUB="$npub_bech32"

# Verify the key pair is valid
echo "Private key: $NSEC"
echo "Public key: $NPUB"
```

### Create Encrypted Request
```bash
# Create your request JSON
REQUEST_JSON='{"cashu_token":"<YOUR_CASHU_TOKEN>","ssh_username":"alice","pod_image":"linuxserver/openssh-server:latest","duration_minutes":120}'

# Get the service's public key from logs (you'll need this for encryption)
# Check service logs to find the service public key:
kubectl logs -n ingress-system -l app=paygress-sidecar | grep "Service public key"
# Look for output like: "Service public key: npub1abc123..."

SERVICE_NPUB="npub1abc123..."  # Replace with actual service public key from logs

# Convert npub to hex format (nak encrypt requires 64-char hex, not npub)
SERVICE_PUBKEY_HEX=$(echo "$SERVICE_NPUB" | nak pubkey --hex)

# Encrypt the request using nak with NIP-44 (default encryption)
# Note: The service now uses NIP-44 encryption for better security
ENCRYPTED_CONTENT=$(nak encrypt --sec "$NSEC" --recipient-pubkey "$SERVICE_PUBKEY_HEX" "$REQUEST_JSON")

# Send the encrypted event
nak event \
  --kind 1000 \
  --content "$ENCRYPTED_CONTENT" \
  --sec "$NSEC" \
  --tag "paygress" \
  --tag "encrypted" \
  --tag "provisioning" \
  wss://relay.damus.io wss://nos.lol wss://relay.nostr.band
```

## 🎯 Step 8: Listen for Encrypted Response

```bash
# Listen for encrypted response (kind 1001) - use the event ID from above
REQ_ID="<event_id_from_above>"
nak req -k 1001 -e $REQ_ID --stream wss://relay.damus.io wss://nos.lol wss://relay.nostr.band

# The response will be encrypted! You'll need to decrypt it using your private key.
# To decrypt the response:
ENCRYPTED_RESPONSE="<encrypted_content_from_response>"
SERVICE_NPUB="npub1abc123..."  # Service public key from logs

# Decrypt using nak (works with bech32 keys)
DECRYPTED_RESPONSE=$(nak decrypt --sec "$NSEC" --sender-pubkey "$SERVICE_NPUB" "$ENCRYPTED_RESPONSE")
echo "$DECRYPTED_RESPONSE"

# The decrypted response contains:
# - pod_name: ssh-pod-xxxxx
# - ssh_username: alice
# - ssh_password: xxxxxxxx
# - node_port: 3xxxx
# - All other sensitive data
# - **Sent directly from the pod itself!**
```

## 🔑 Step 9: Access Your Pod

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

## 🔄 Step 10: Extend Pod Duration (Top-ups)

**✅ Extend your pod's lifetime with additional payments!**

### HTTP Mode Top-up:
```bash
# Extend existing pod duration
curl -X POST http://localhost:8080/top-up-pod \
  -H "Content-Type: application/json" \
  -d '{
    "pod_name": "ssh-pod-abc12345",
    "cashu_token": "your_topup_token_here"
  }'
```

### Nostr Mode Top-up:
```bash
# Create top-up request
TOPUP_JSON='{"pod_name":"ssh-pod-abc12345","cashu_token":"<YOUR_TOPUP_TOKEN>"}'

# Get the service's public key from logs
SERVICE_NPUB="npub1abc123..."  # Replace with actual service public key from logs

# Encrypt the top-up request (works with bech32 keys)
ENCRYPTED_TOPUP=$(echo "$TOPUP_JSON" | nak encrypt --sec "$NSEC" --pub "$SERVICE_NPUB")

# Send encrypted top-up event (kind 1002)
nak event \
  --kind 1002 \
  --content "$ENCRYPTED_TOPUP" \
  --sec "$NSEC" \
  --tag "paygress" \
  --tag "encrypted" \
  --tag "topup" \
  wss://relay.damus.io wss://nos.lol wss://relay.nostr.band
```

### Top-up Features:
- **Extend Duration**: Add more time to existing pods
- **Payment Verification**: Validates Cashu tokens for top-ups
- **Automatic Extension**: Updates `activeDeadlineSeconds` in Kubernetes
- **No Interruption**: Pod continues running during extension
- **Flexible Payment**: Pay any amount to extend by that many minutes

## ⏰ Automatic Pod Lifecycle Management

**✅ Your pods are automatically managed using Kubernetes' built-in `activeDeadlineSeconds`!**

### How It Works:
- **Payment = Duration**: 1 sat = 1 minute (e.g., 100 sats = 100 minutes)
- **Kubernetes Native**: Uses `activeDeadlineSeconds` for automatic pod termination
- **No Polling**: No cleanup tasks or CronJobs needed - Kubernetes handles everything
- **Immediate Termination**: Pods are terminated as soon as their time expires
- **Resource Cleanup**: Both the pod and its associated service are removed automatically

### Pod Duration & Top-ups:
- **Specify Duration**: Set `duration_minutes` in your request for custom duration
- **Extend Duration**: Use top-up requests to extend existing pods
- **Automatic Management**: Kubernetes handles all timing automatically

### Configuration:
```bash
# Payment rate (1 sat = 1 minute by default)
PAYMENT_RATE_SATS_PER_HOUR=100

# Default duration if not specified in request
DEFAULT_POD_DURATION_MINUTES=60

# No cleanup task needed - Kubernetes handles everything
ENABLE_CLEANUP_TASK=false
```

### Examples:
- **10 sats** → Pod runs for **10 minutes**, then gets terminated
- **100 sats** → Pod runs for **100 minutes** (~1.7 hours), then gets terminated
- **1440 sats** → Pod runs for **1440 minutes** (24 hours), then gets terminated
- **Top-up 60 sats** → Extends existing pod by 60 minutes

**Note**: Kubernetes `activeDeadlineSeconds` ensures pods are terminated exactly when their paid duration expires. No external cleanup processes needed!

## 🚀 **Docker Hub Deployment with GitHub Actions**

This repository includes GitHub Actions workflows to automatically build and push Docker images to Docker Hub.

### **Setup GitHub Secrets**

1. Go to your GitHub repository settings
2. Navigate to **Secrets and variables** → **Actions**
3. Add the following secrets:

```bash
DOCKERHUB_USERNAME=your_dockerhub_username
DOCKERHUB_TOKEN=your_dockerhub_access_token
```

### **Docker Hub Access Token**

1. Go to [Docker Hub](https://hub.docker.com/)
2. Navigate to **Account Settings** → **Security**
3. Click **New Access Token**
4. Give it a name (e.g., "github-actions")
5. Set permissions to **Read, Write, Delete**
6. Copy the token and add it as `DOCKERHUB_TOKEN` secret

### **Automatic Builds**

The workflow will automatically:
- **Build** on every push to `main`/`master` branch
- **Build** on every pull request to `main`/`master` branch  
- **Build and push** on version tags (e.g., `v1.0.0`)
- **Build** for both `linux/amd64` and `linux/arm64` platforms

### **Image Tags**

Images will be tagged as:
- `latest` - Latest commit on default branch
- `main`/`master` - Branch name
- `v1.0.0` - Version tags
- `v1.0` - Major.minor version
- `v1` - Major version

### **Using the Images**

```bash
# Pull the latest image
docker pull yourusername/paygress-sidecar:latest

# Pull a specific version
docker pull yourusername/paygress-sidecar:v1.0.0

# Run the container
docker run -d \
  --name paygress-sidecar \
  -p 8080:8080 \
  -e NOSTR_PRIVATE_KEY=your_nsec_key \
  -e NOSTR_RELAYS=wss://relay.damus.io,wss://nos.lol \
  yourusername/paygress-sidecar:latest
```

## 🔧 Configuration Examples

### Common Configuration Changes

**Change SSH User/Password:**
```bash
# Edit your paygress.env file
nano paygress.env

# Reapply configuration
kubectl create configmap paygress-sidecar-config \
    --from-env-file=paygress.env \
    --namespace=ingress-system \
    --dry-run=client -o yaml | kubectl apply -f -

kubectl rollout restart deployment/paygress-sidecar -n ingress-system
```

## 🔧 Step 10: Monitor and Manage

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
2. **User sends encrypted Nostr event** → Kind 1000 with encrypted Cashu token and pod requirements
3. **Service processes** → Verifies payment, creates SSH pod in Kubernetes with `activeDeadlineSeconds`
4. **Pod sends encrypted response** → Kind 1001 event with SSH access details (sent by the pod itself!)
5. **User accesses pod** → Uses provided SSH credentials via NodePort or port-forward
6. **Optional: Extend duration** → Send Kind 1002 top-up event or HTTP POST to extend pod lifetime
7. **Automatic termination** → Kubernetes terminates pod when `activeDeadlineSeconds` expires

**Complete encrypted Nostr-based workflow - no HTTP endpoints needed!**

## 🌐 HTTP Mode (Alternative)

**✅ Also supports HTTP endpoints for traditional API access!**

### Available Endpoints:
- `GET /healthz` - Health check with feature status
- `POST /spawn-pod` - Create new pod with duration
- `POST /top-up-pod` - Extend existing pod duration
- `GET /pods` - List all active pods
- `GET /pods/:name` - Get specific pod info

### HTTP Mode Usage:
```bash
# Create pod via HTTP
curl -X POST http://localhost:8080/spawn-pod \
  -H "Content-Type: application/json" \
  -d '{
    "cashu_token": "your_token_here",
    "duration_minutes": 120,
    "ssh_username": "alice"
  }'

# Extend pod via HTTP
curl -X POST http://localhost:8080/top-up-pod \
  -H "Content-Type: application/json" \
  -d '{
    "pod_name": "ssh-pod-abc12345",
    "cashu_token": "your_topup_token"
  }'
```

### Run in HTTP Mode:
```bash
# Set environment variable to enable HTTP mode
export RUN_MODE=http
cargo run
```

## Decentralized Architecture

- **Nostr Events**: All communication via decentralized relay network
- **Cashu Payments**: Bitcoin-based e-cash for payments
- **Kubernetes**: Container orchestration with `activeDeadlineSeconds` for pod lifecycle
- **Top-up Support**: Extend pod duration via Nostr (kind 1002) or HTTP
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
# Generate keys using nak
hex=$(nak key generate)
echo "hex: $hex"

# Convert hex to bech32 format
nsec=$(echo "$hex" | nak encode nsec)
echo "nsec: $nsec"

# Get public key (use hex, not bech32)
npub=$(nak key public "$hex")
echo "npub: $npub"

# Convert public key to bech32 format
npub_bech32=$(echo "$npub" | nak encode npub)
echo "npub (bech32): $npub_bech32"

# Store your keys
export NSEC="$nsec"
export NPUB="$npub_bech32"

# Get payment
cashu mint 1000 --url https://mint.cashu.space

# Get service public key from logs
SERVICE_NPUB="npub1abc123..."  # Replace with actual service public key

# Create and encrypt request
REQUEST_JSON='{"cashu_token":"YOUR_TOKEN","ssh_username":"alice","duration_minutes":60}'
ENCRYPTED_CONTENT=$(echo "$REQUEST_JSON" | nak encrypt --sec "$NSEC" --pub "$SERVICE_NPUB")

# Send encrypted request
nak event \
  --kind 1000 \
  --content "$ENCRYPTED_CONTENT" \
  --sec "$NSEC" \
  --tag "paygress" \
  --tag "encrypted" \
  wss://relay.damus.io wss://nos.lol wss://relay.nostr.band

# Listen for encrypted response
nak req -k 1001 -e $EVENT_ID --stream wss://relay.damus.io wss://nos.lol wss://relay.nostr.band

# Decrypt response
ENCRYPTED_RESPONSE="<encrypted_content_from_response>"
DECRYPTED_RESPONSE=$(echo "$ENCRYPTED_RESPONSE" | nak decrypt --sec "$NSEC" --pub "$SERVICE_NPUB")
echo "$DECRYPTED_RESPONSE"

# Connect via SSH using decrypted credentials
ssh -o PreferredAuthentications=password -o PubkeyAuthentication=no alice@$(minikube ip) -p $NODE_PORT
```

## ⚙️ **Configuration**

### **Environment Variables**

| Variable | Default | Description |
|----------|---------|-------------|
| `RUN_MODE` | `nostr` | Service mode: `nostr` or `http` |
| `NOSTR_RELAYS` | `wss://relay.damus.io,wss://nos.lol,wss://relay.nostr.band` | Comma-separated list of Nostr relays |
| `NOSTR_PRIVATE_KEY` | `""` | Service's private key (nsec format) for consistent identity |
| `WHITELISTED_MINTS` | `https://mint.cashu.space,https://mint.f7z.io,https://legend.lnbits.com/cashu/api/v1` | Comma-separated list of allowed Cashu mint URLs |
| `POD_NAMESPACE` | `user-workloads` | Kubernetes namespace for SSH pods |
| `PAYMENT_RATE_SATS_PER_HOUR` | `100` | Payment rate in satoshis per hour |
| `SSH_BASE_IMAGE` | `linuxserver/openssh-server:latest` | SSH server container image |

### **Custom Configuration**

```bash
# Set a consistent service identity (recommended for production)
kubectl -n ingress-system set env deploy/paygress-sidecar NOSTR_PRIVATE_KEY="nsec1your_private_key_here"

# Use custom relays
kubectl -n ingress-system set env deploy/paygress-sidecar NOSTR_RELAYS="wss://your-relay.com,wss://another-relay.com"

# Configure whitelisted Cashu mints
kubectl -n ingress-system set env deploy/paygress-sidecar WHITELISTED_MINTS="https://mint.cashu.space,https://mint.f7z.io,https://your-mint.com"

# Update payment rate
kubectl -n ingress-system set env deploy/paygress-sidecar PAYMENT_RATE_SATS_PER_HOUR="200"
```

### **Setting Up Consistent Service Identity**

For production use, you should set a consistent private key so your service always has the same public key:

```bash
# Generate a service keypair using nak
hex=$(nak key generate)
echo "hex: $hex"

# Convert hex to bech32 format (nsec1...)
nsec=$(echo "$hex" | nak encode nsec)
echo "nsec: $nsec"

# Get public key from private key (use hex, not bech32)
npub=$(nak key public "$hex")
echo "npub: $npub"

# Convert public key to bech32 format
npub_bech32=$(echo "$npub" | nak encode npub)
echo "npub (bech32): $npub_bech32"

# Set the private key in your deployment (use bech32 format)
SERVICE_PRIVATE_KEY="$nsec"  # Your service private key in bech32 format
kubectl -n ingress-system set env deploy/paygress-sidecar NOSTR_PRIVATE_KEY="$SERVICE_PRIVATE_KEY"

# Share the public key with users
echo "Service public key: $npub_bech32"
```
