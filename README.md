# Paygress - NIP-17 Encrypted Private Message Pod Provisioning

## 🚀 **Overview**

🔧 **NIP-17 + NIP-44 + NIP-59 Encrypted Private Messages → Kubernetes Pod Provisioning with Cashu Payments**

**NIP-17 Encrypted Private Message-Driven Pod Provisioning:**
- Service listens for **NIP-17 gift wraps** (kind 1059) with Cashu tokens
- All sensitive data sent via NIP-17 encrypted private messages (Cashu tokens, SSH credentials)
- Users can select from multiple pod specifications (CPU/memory tiers)
- Each pod specification has its own pricing
- Replies with access details via **NIP-17 encrypted private messages** (kind 1059)

## 🎯 **Quick Start**

### **1. Setup Environment**

Create your `.env` file from the template:

```bash
# Copy the template to create your environment file
cp .env.template .env

# Edit the .env file with your actual configuration
nano .env
```

**⚠️ Important: You MUST configure these values in your `.env` file:**

1. **NOSTR_PRIVATE_KEY**: Replace `YOUR_NOSTR_PRIVATE_KEY_HERE` with your actual Nostr private key
2. **SSH_HOST**: Replace `YOUR_SERVER_PUBLIC_IP` with your server's public IP address
3. **NOSTR_RELAYS**: Update relay URLs if needed
4. **WHITELISTED_MINTS**: Update mint URLs if needed

**Example Configuration:**
```bash
# Service Configuration
RUN_MODE=nostr
BIND_ADDR=0.0.0.0:8080
CASHU_DB_PATH=/app/data/cashu.db
POD_NAMESPACE=user-workloads
MINIMUM_POD_DURATION_SECONDS=60
ENABLE_CLEANUP_TASK=true
RUST_LOG=info

# Nostr Configuration (REQUIRED)
NOSTR_RELAYS=wss://relay.damus.io,wss://nos.lol,wss://relay.nostr.band,wss://nostr.wine
NOSTR_PRIVATE_KEY=nsec1...

# Cashu Configuration (REQUIRED)
WHITELISTED_MINTS=https://mint.cashu.space,https://mint.f7z.io

# SSH Pod Configuration
BASE_IMAGE=linuxserver/openssh-server:latest
SSH_HOST=server-public-ip
SSH_PORT_RANGE_START=2000
SSH_PORT_RANGE_END=3000

# Pod Specifications (REQUIRED - JSON file path)
POD_SPECS_FILE=pod-specs.json
```

Create a `pod-specs.json` file with your pod specifications:

```json
[
  {
    "id": "basic",
    "name": "Basic",
    "description": "Basic VPS - 1 CPU core, 1GB RAM",
    "cpu_millicores": 1000,
    "memory_mb": 1024,
    "rate_msats_per_sec": 100
  },
  {
    "id": "standard", 
    "name": "Standard",
    "description": "Standard VPS - 2 CPU cores, 2GB RAM",
    "cpu_millicores": 2000,
    "memory_mb": 2048,
    "rate_msats_per_sec": 200
  },
  {
    "id": "premium",
    "name": "Premium", 
    "description": "Premium VPS - 4 CPU cores, 4GB RAM",
    "cpu_millicores": 4000,
    "memory_mb": 4096,
    "rate_msats_per_sec": 400
  }
]
```

### **2. Deploy to Kubernetes**

```bash
# 1. Build the Docker image with timestamp
IMAGE_TAG=$(date +%s)   # or use: $(git rev-parse --short HEAD)
docker build -t paygress:$IMAGE_TAG .

# 2. Load into your Kubernetes cluster
# For Minikube:
minikube image load paygress:$IMAGE_TAG

# For K3s:
# sudo k3s ctr images import <(docker save paygress:$IMAGE_TAG)


# 3. Create ConfigMaps from your configuration files (REQUIRED)
kubectl create configmap paygress-env-config \
    --from-env-file=.env \
    --namespace=ingress-system \
    --dry-run=client -o yaml | kubectl apply -f -

kubectl create configmap paygress-pod-specs \
  --from-file=pod-specs.json \
  --namespace=ingress-system \
  --dry-run=client -o yaml | kubectl apply -f -

# 4. Deploy the application with the timestamped image
kubectl apply -f k8s/sidecar-service.yaml
kubectl set image deployment/paygress-sidecar \
    paygress-sidecar=paygress:$IMAGE_TAG \
    -n ingress-system

# 5. Wait for deployment to be ready
kubectl wait --for=condition=available --timeout=300s \
    deployment/paygress-sidecar -n ingress-system

# 6. Check logs (to get public key or debug)
kubectl logs -n ingress-system -l app=paygress-sidecar
```

**Note**: The YAML file references ConfigMaps (`paygress-env-config` and `paygress-pod-specs`) that must be created from your configuration files using the kubectl commands above. This approach ensures your actual files are the source of truth, not hardcoded values in YAML.

**After making code changes:**
```bash
# Rebuild and redeploy to pick up code changes
IMAGE_TAG=$(date +%s)
docker build -t paygress:$IMAGE_TAG .
minikube image load paygress:$IMAGE_TAG  # or your cluster equivalent
kubectl set image deployment/paygress-sidecar \
    paygress-sidecar=paygress:$IMAGE_TAG \
    -n ingress-system
kubectl logs -n ingress-system -l app=paygress-sidecar -f
```


## 📋 **Complete User Guide**

### **Step 1: Get Available Offers**

The service publishes offer events containing available pod specifications. To get the offers:

```bash
# Using nostr-tools to get offers
nostr-tools query --relay wss://relay.damus.io --kind 999 --limit 10

# Look for events with tags: ["paygress", "offer"]
```

**Example Offer Event:**
```json
{
  "kind": 999,
  "content": "{\"minimum_duration_seconds\":60,\"whitelisted_mints\":[\"https://mint.cashu.space\"],\"pod_specs\":[{\"id\":\"basic\",\"name\":\"Basic\",\"description\":\"Basic VPS - 1 CPU core, 1GB RAM\",\"cpu_millicores\":1000,\"memory_mb\":1024,\"rate_msats_per_sec\":100},{\"id\":\"standard\",\"name\":\"Standard\",\"description\":\"Standard VPS - 2 CPU cores, 2GB RAM\",\"cpu_millicores\":2000,\"memory_mb\":2048,\"rate_msats_per_sec\":200},{\"id\":\"premium\",\"name\":\"Premium\",\"description\":\"Premium VPS - 4 CPU cores, 4GB RAM\",\"cpu_millicores\":4000,\"memory_mb\":4096,\"rate_msats_per_sec\":400}]}",
  "tags": [
    ["t", "paygress"],
    ["t", "offer"]
  ],
  "pubkey": "service_public_key_here"
}
```

### **Step 2: Choose Pod Specification**

From the offer, you can see available pod specifications:

| Specification | CPU | Memory | Rate (msats/sec) | Description |
|---------------|-----|--------|------------------|-------------|
| `basic` | 1000 millicores | 1GB | 100 | Basic VPS - 1 CPU core, 1GB RAM |
| `standard` | 2000 millicores | 2GB | 200 | Standard VPS - 2 CPU cores, 2GB RAM |
| `premium` | 4000 millicores | 4GB | 400 | Premium VPS - 4 CPU cores, 4GB RAM |

**Calculate Payment Required:**
```
Payment (msats) = Duration (seconds) × Rate (msats/sec)
```

**Examples:**
- 1 hour (3600 sec) Basic: 3600 × 100 = 360,000 msats
- 2 hours (7200 sec) Standard: 7200 × 200 = 1,440,000 msats
- 30 minutes (1800 sec) Premium: 1800 × 400 = 720,000 msats

### **Step 3: Create Cashu Token**

Generate a Cashu token for the required payment amount:

```bash
# Using cashu-cli (example)
cashu-cli mint --amount 360000 --mint https://mint.cashu.space
```

### **Step 4: Send Pod Provisioning Request**

Send a NIP-17 Gift Wrap private message to the service:

**Request Structure:**
```json
{
  "cashu_token": "cashu...",
  "pod_spec_id": "standard",
  "pod_image": "linuxserver/openssh-server:latest",
  "ssh_username": "alice",
  "ssh_password": "my_secure_password"
}
```

**Field Descriptions:**
- `cashu_token`: Payment token for pod provisioning
- `pod_spec_id`: Which specification to use (`basic`, `standard`, `premium`) - optional, defaults to first available
- `pod_image`: Container image to use for the pod (required)
- `ssh_username`: SSH username for pod access
- `ssh_password`: SSH password for pod access

**Using nostr-tools:**
```bash
# Create request JSON
echo '{
  "cashu_token": "your_cashu_token_here",
  "pod_spec_id": "standard",
  "pod_image": "linuxserver/openssh-server:latest",
  "ssh_username": "alice",
  "ssh_password": "my_secure_password"
}' > request.json

# Send as NIP-17 Gift Wrap private message
nostr-tools encrypt --key your_nsec_key --pubkey service_npub_key < request.json | \
nostr-tools publish --relay wss://relay.damus.io --kind 1059
```

### **Step 5: Receive Access Details**

The service will send back access details via NIP-17 Gift Wrap private message:

**Access Details Structure:**
```json
{
  "pod_npub": "npub...",
  "node_port": 2500,
  "expires_at": "2025-09-23T15:30:00Z",
  "cpu_millicores": 2000,
  "memory_mb": 2048,
  "pod_spec_name": "Standard",
  "pod_spec_description": "Standard VPS - 2 CPU cores, 2GB RAM",
  "instructions": [
    "🚀 SSH access available:",
    "",
    "Direct access (no kubectl needed):",
    "   ssh -o PreferredAuthentications=password -o PubkeyAuthentication=no alice@-PUBLIC-IP -p 2500",
    "",
    "⚠️  Pod expires at:",
    "   2025-09-23 15:30:00 UTC",
    "",
    "📋 Pod Details:",
    "   Pod NPUB: npub1abc123def456...",
    "   Specification: Standard (Standard VPS - 2 CPU cores, 2GB RAM)",
    "   CPU: 2000 millicores",
    "   Memory: 2048 MB",
    "   Duration: 7200 seconds"
  ]
}
```

### **Step 6: Connect to Your Pod**

Use the provided SSH command:

```bash
   ssh -o PreferredAuthentications=password -o PubkeyAuthentication=no alice@server-public-ip -p 2500
```

### **Step 7: Top-Up Pod Duration**

To extend your pod's lifetime, send a top-up request:

**Top-Up Request Structure:**
```json
{
  "pod_npub": "npub...",
  "cashu_token": "cashu..."
}
```

**Field Descriptions:**
- `pod_npub`: The pod's NPUB identifier (from access details)
- `cashu_token`: Payment token for the extension

**Using nostr-tools:**
```bash
# Create top-up request JSON
echo '{
  "pod_npub": "npub...",
  "cashu_token": "your_topup_cashu_token_here"
}' > topup.json

# Send as NIP-17 Gift Wrap private message
nostr-tools encrypt --key your_nsec_key --pubkey service_npub_key < topup.json | \
nostr-tools publish --relay wss://relay.damus.io --kind 1059
```

### **Step 8: Receive Top-Up Confirmation**

The service will send back a top-up confirmation via NIP-17 Gift Wrap private message:

**Top-Up Success Response:**
```json
{
  "success": true,
  "pod_npub": "npub...",
  "extended_duration_seconds": 3600,
  "new_expires_at": "2025-09-23T16:30:00Z",
  "message": "Pod successfully topped up!"
}
```

**Top-Up Error Response:**
```json
{
  "error_type": "pod_not_found",
  "message": "Pod not found or expired",
  "details": "Pod with NPUB 'npub1abc123...' not found or already expired"
}
```

## 🔐 **Security Features**

### **NIP-17 Encryption**
- All requests and responses use NIP-17 Gift Wrap encryption
- Only the intended recipient can decrypt messages
- No sensitive data is publicly visible on relays

### **Payment Verification**
- All Cashu tokens are verified against whitelisted mints
- Payment amounts are validated before pod creation
- Automatic pod termination when payment expires

### **Resource Isolation**
- Each pod runs in its own namespace
- CPU and memory limits enforced per specification
- Automatic cleanup when pods expire

## ⚠️ **Error Handling**

The service provides comprehensive error responses for all failure scenarios:

### **Pod Provisioning Errors**

| Error Type | Description | When Sent |
|------------|-------------|-----------|
| `invalid_spec` | Pod specification not found | Invalid `pod_spec_id` |
| `invalid_token` | Cashu token decode/verification failed | Token issues |
| `insufficient_payment` | Payment below minimum requirement | Not enough payment |
| `resource_unavailable` | SSH port allocation failed | No ports available |
| `pod_creation_failed` | Kubernetes pod creation failed | K8s API errors |

### **Top-Up Errors**

| Error Type | Description | When Sent |
|------------|-------------|-----------|
| `pod_not_found` | Pod not found or expired | Top-up on non-existent pod |
| `payment_failed` | Top-up payment verification failed | Invalid top-up token |
| `internal_error` | Server-side processing error | Unexpected failures |

### **Error Response Format**

All error responses follow this structure:

```json
{
  "error_type": "insufficient_payment",
  "message": "Insufficient payment: 50000 msats",
  "details": "Minimum required: 100000 msats for 60 seconds with Basic spec (rate: 100 msats/sec)"
}
```

## 🛠️ **VPS Provider Configuration**

### **Custom Pod Specifications**

VPS providers can customize their offerings by modifying the `POD_SPECS` environment variable:

```bash
# Example: High-performance server offerings
POD_SPECS=[
  {
    "id": "starter",
    "name": "Starter",
    "description": "Starter VPS - 0.5 CPU, 512MB RAM",
    "cpu_millicores": 500,
    "memory_mb": 512,
    "rate_msats_per_sec": 50
  },
  {
    "id": "professional",
    "name": "Professional", 
    "description": "Professional VPS - 8 CPU, 16GB RAM",
    "cpu_millicores": 8000,
    "memory_mb": 16384,
    "rate_msats_per_sec": 800
  },
  {
    "id": "enterprise",
    "name": "Enterprise",
    "description": "Enterprise VPS - 16 CPU, 32GB RAM", 
    "cpu_millicores": 16000,
    "memory_mb": 32768,
    "rate_msats_per_sec": 1600
  }
]
```

### **Environment Variables Reference**

| Variable | Required | Description | Example |
|----------|----------|-------------|---------|
| `NOSTR_RELAYS` | ✅ | Comma-separated relay URLs | `wss://relay.damus.io,wss://nos.lol` |
| `NOSTR_PRIVATE_KEY` | ✅ | Service's Nostr private key (nsec) | `nsec1...` |
| `WHITELISTED_MINTS` | ✅ | Comma-separated mint URLs | `https://mint.cashu.space,https://mint.f7z.io` |
| `POD_SPECS` | ✅ | JSON array of pod specifications | See example above |
| `SSH_HOST` | ✅ | Public IP for SSH access | `server-public-ip` |
| `POD_NAMESPACE` | ❌ | Kubernetes namespace | `user-workloads` |
| `MINIMUM_POD_DURATION_SECONDS` | ❌ | Minimum pod lifetime | `60` |
| `BASE_IMAGE` | ❌ | Base container image | `linuxserver/openssh-server:latest` |
| `SSH_PORT_RANGE_START` | ❌ | Start of port range | `2000` |
| `SSH_PORT_RANGE_END` | ❌ | End of port range | `3000` |

## 🚀 **Kubernetes Deployment**

### **Prerequisites**

Ensure you have:
- Kubernetes cluster running
- `kubectl` configured to access your cluster
- Your environment configuration file (`.env`)

### **Deployment Steps**

```bash
# 1. Apply the Kubernetes manifests
kubectl apply -f k8s/

# 2. Create configmaps with your environment configuration
kubectl create configmap paygress-env-config --from-env-file=.env --namespace=ingress-system --dry-run=client -o yaml | kubectl apply -f -

kubectl create configmap paygress-pod-specs --from-file=pod-specs.json --namespace=ingress-system --dry-run=client -o yaml | kubectl apply -f -

# 3. Verify deployment
kubectl get pods -n ingress-system
kubectl get services -n ingress-system

# 4. Check logs
kubectl logs -n ingress-system -l app=paygress-sidecar

# 5. Update configuration (if needed)
kubectl delete configmap paygress-env-config -n ingress-system
kubectl delete configmap paygress-pod-specs -n ingress-system
kubectl create configmap paygress-env-config --from-env-file=.env --namespace=ingress-system --dry-run=client -o yaml | kubectl apply -f -
kubectl create configmap paygress-pod-specs --from-file=pod-specs.json --namespace=ingress-system --dry-run=client -o yaml | kubectl apply -f -
kubectl rollout restart deployment/paygress-sidecar -n ingress-system
```

## 📊 **Event Flow**

```
User                    Service
  |                       |
  |-- Query offers ------>|  (Kind 999 events)
  |<-- Offer response ----|  (Available pod specs)
  |                       |
  |-- Provision request ->|  (NIP-17 Gift Wrap with Cashu token)
  |                       |-- Creates pod with selected spec
  |<-- Access details ----|  (NIP-17 Gift Wrap with SSH details)
  |                       |
  |-- Top-up request ---->|  (NIP-17 Gift Wrap with Cashu token)
  |                       |-- Extends pod lifetime
  |                       |  (No response needed)
```

## 🎉 **Features**

### **✅ Multiple Pod Specifications**
- VPS providers can offer different CPU/memory tiers
- Each specification has its own pricing
- Users can select the specification that fits their needs

### **✅ Encrypted Communication**
- All requests are encrypted using NIP-17 Gift Wrap
- Only the service can decrypt your requests
- Your private keys stay secure

### **✅ Automatic Pod Management**
- Pods created with `activeDeadlineSeconds`
- Top-ups extend the deadline automatically
- Kubernetes handles termination

### **✅ No HTTP Required**
- Pure Nostr protocol
- Works with any Nostr client
- Decentralized and censorship-resistant

### **✅ Same Functionality as HTTP**
- Create pods with custom specifications
- Extend pod duration with top-ups
- All payment verification included

## 🔧 **Troubleshooting**

### **Common Issues**

1. **"POD_SPECS environment variable is required"**
   - Ensure `POD_SPECS` is set with valid JSON
   - Check JSON syntax is correct

2. **"No pod specification found for ID"**
   - Verify the `pod_spec_id` matches an available specification
   - Check the offer event for available IDs

3. **"Insufficient payment"**
   - Calculate required payment: `Duration × Rate`
   - Ensure Cashu token has sufficient value

4. **"Failed to parse private message content"**
   - Check JSON syntax in your request
   - Ensure all required fields are present

### **Logs**

Check service logs for detailed information:
```bash
kubectl logs -n ingress-system -l app=paygress-sidecar
```

## 📝 **API Reference**

### **Pod Provisioning Request**
```json
{
  "cashu_token": "string",      // Required: Payment token
  "pod_spec_id": "string",      // Optional: Specification ID (defaults to first)
  "pod_image": "string",        // Required: Container image for the pod
  "ssh_username": "string",     // Required: SSH username
  "ssh_password": "string"      // Required: SSH password
}
```

### **Top-Up Request**
```json
{
  "pod_npub": "string",         // Required: Pod's NPUB identifier
  "cashu_token": "string"       // Required: Payment token for extension
}
```

### **Access Details Response**
```json
{
  "pod_npub": "string",         // Pod's NPUB identifier
  "node_port": "number",        // SSH port
  "expires_at": "string",       // ISO 8601 expiration time
  "cpu_millicores": "number",   // CPU allocation in millicores
  "memory_mb": "number",        // Memory allocation in MB
  "pod_spec_name": "string",    // Human-readable spec name
  "pod_spec_description": "string", // Spec description
  "instructions": ["string"]    // SSH connection instructions
}
```

### **Top-Up Success Response**
```json
{
  "success": "boolean",         // Always true for success
  "pod_npub": "string",         // Pod's NPUB identifier
  "extended_duration_seconds": "number", // Seconds added to pod
  "new_expires_at": "string",   // New expiration time (ISO 8601)
  "message": "string"           // Success message
}
```

### **Error Response**
```json
{
  "error_type": "string",       // Type of error (see error types table)
  "message": "string",          // Human-readable error message
  "details": "string"           // Additional error details (optional)
}
```

---

## 🧪 Local Development (Kubernetes)

For local development, you can deploy Paygress using Kubernetes instead of systemd.

### Prerequisites
- Docker installed
- K3s or Kubernetes cluster running locally
- kubectl configured

### Configuration
The Kubernetes deployment automatically reads environment variables from your `.env` file. Simply edit `.env` to modify the configuration.

### Deployment
```bash
# Build the Docker image
docker build -t paygress:latest .

# Import the image into K3s (if using K3s)
sudo k3s ctr images import <(docker save paygress:latest)

# Create ConfigMap from .env file
kubectl create configmap paygress-env-config --from-env-file=paygress.env -n ingress-system --dry-run=client -o yaml | kubectl apply -f -

# Apply the Kubernetes manifests
kubectl apply -f k8s/sidecar-service.yaml

# Wait for deployment to be ready
kubectl wait --for=condition=available --timeout=300s deployment/paygress-sidecar -n ingress-system
```

### Management
```bash
# View logs
kubectl logs -f deployment/paygress-sidecar -n ingress-system

# Port forward for local access
kubectl port-forward deployment/paygress-sidecar 8080:8080 -n ingress-system

# Delete deployment
kubectl delete -f k8s/sidecar-service.yaml
```

---

## 🤖 **Context VM Integration**

Paygress now supports **Context VM** integration through the Model Context Protocol (MCP). This allows AI assistants and other tools to interact with your pod provisioning service directly.

### **Context VM Setup**

1. **Install gateway-cli**:
   ```bash
   # Install gateway-cli for Context VM
   npm install -g @contextvm/gateway-cli
   ```

2. **Configure your Context VM**:
   ```bash
   # Set up your private key and relay
   export CONTEXTVM_PRIVATE_KEY="your_nostr_private_key_here"
   export CONTEXTVM_RELAY="wss://relay.contextvm.org"
   ```

3. **Start the MCP Server**:
   ```bash
   # Build the MCP server
   cargo build --bin paygress-mcp-server
   
   # Run the MCP server
   ./start-paygress-context-vm.sh
   ```

4. **Connect with gateway-cli**:
   ```bash
   gateway-cli \
     --private-key "your_private_key_here" \
     --relays "wss://relay.contextvm.org" \
     --server ./start-paygress-context-vm.sh
   ```

### **Available MCP Tools**

The Context VM integration provides these tools:

| Tool | Description | Parameters |
|------|-------------|------------|
| `spawn_pod` | Create a new SSH-accessible pod | `cashu_token`, `pod_image`, `ssh_username`, `ssh_password`, `pod_spec_id` (optional) |
| `topup_pod` | Extend pod duration | `pod_npub`, `cashu_token` |
| `get_offers` | Get available pod specifications | None |

### **Example Context VM Usage**

```bash
# Get available offers
gateway-cli --tool get_offers

# Spawn a new pod
gateway-cli --tool spawn_pod \
  --cashu_token "cashuA..." \
  --pod_image "linuxserver/openssh-server:latest" \
  --ssh_username "alice" \
  --ssh_password "secure_password" \
  --pod_spec_id "standard"


# Top up a pod
gateway-cli --tool topup_pod \
  --pod_npub "npub1..." \
  --cashu_token "cashuB..."
```

### **Context VM Configuration**

Add these environment variables to your `.env` file for Context VM support:

```bash
# Context VM Configuration
CONTEXTVM_PRIVATE_KEY=nsec1...
CONTEXTVM_RELAY=wss://relay.contextvm.org
CONTEXTVM_ENABLED=true
```

---

**Complete NIP-04 and NIP-17 encrypted private message-based workflow - no HTTP endpoints needed!** 🚀

## 🔄 Switching Between Deployments

### From Kubernetes to Systemd (Production)
```bash
# Stop Kubernetes deployment
kubectl delete -f k8s/sidecar-service.yaml

# Deploy with Ansible
ansible-playbook -i inventory.ini ansible-setup.yml
```

### From Systemd to Kubernetes (Local Development)
```bash
# Stop systemd service
sudo systemctl stop paygress
sudo systemctl disable paygress

# Deploy to Kubernetes (see Local Development section above)
```