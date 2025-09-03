# Complete Paygress Ingress Plugin

A full-featured Rust ingress plugin that combines Cashu payment verification, Kubernetes pod provisioning, and Nostr event publishing.

## 🚀 Features

### Core Capabilities
- **🔐 Cashu Payment Verification** - Native ecash token validation
- **☸️ Kubernetes Pod Provisioning** - Automatic resource creation on payment
- **📡 Nostr Event Publishing** - Payment notifications via Nostr protocol
- **🔌 Ingress Integration** - Works with NGINX, Traefik, and other ingress controllers

### Deployment Modes
- **Simple Mode** - Basic auth service (what you already have)
- **Complete Mode** - All features enabled with configurable options

## 🎯 Quick Start

### 1. Build the Complete Plugin

```bash
# Build with all dependencies
cargo build --release
```

### 2. Run in Complete Mode

```bash
export PAYGRESS_MODE=complete
export ENABLE_POD_PROVISIONING=true
export ENABLE_NOSTR_EVENTS=false  # Optional
export POD_NAMESPACE=user-workloads
cargo run
```

### 3. Test the Enhanced Features

```bash
# Health check with feature status
curl http://localhost:8080/healthz

# Auth with pod provisioning
curl "http://localhost:8080/auth?token=test&amount=1000&create_pod=true&service=my-app&image=nginx:alpine"

# Dedicated provisioning endpoint
curl -X POST http://localhost:8080/provision \
  -H "Content-Type: application/json" \
  -d '{"token":"test","amount":1000,"service":"my-service","image":"nginx:alpine"}'
```

## ⚙️ Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `PAYGRESS_MODE` | `simple` | `simple` or `complete` |
| `ENABLE_POD_PROVISIONING` | `true` | Enable Kubernetes pod creation |
| `ENABLE_NOSTR_EVENTS` | `false` | Enable Nostr event publishing |
| `DEFAULT_POD_IMAGE` | `nginx:alpine` | Default container image |
| `POD_NAMESPACE` | `default` | Namespace for provisioned pods |
| `NOSTR_RELAYS` | `wss://relay.damus.io` | Comma-separated relay URLs |
| `NOSTR_SECRET_KEY` | - | Nostr secret key (if events enabled) |

### Feature Matrix

| Mode | Cashu Auth | Pod Provisioning | Nostr Events |
|------|------------|------------------|--------------|
| Simple | ✅ | ❌ | ❌ |
| Complete | ✅ | ✅ (configurable) | ✅ (configurable) |

## 📡 Enhanced API

### GET /healthz
Health check with feature status
```json
{
  "status": "healthy",
  "service": "paygress-complete-plugin",
  "features": {
    "cashu_verification": true,
    "pod_provisioning": true,
    "nostr_events": false
  }
}
```

### GET /auth (Enhanced)
Complete authentication with optional pod provisioning

**Query Parameters:**
- `token` - Cashu token (required)
- `amount` - Payment amount in msat (default: 1000)
- `create_pod` - Create pod on successful payment (default: false)
- `service` - Service name for pod (default: "payment-service")
- `namespace` - Target namespace (default: configured namespace)
- `image` - Container image (default: configured image)

**Example:**
```bash
curl "http://localhost:8080/auth?token=cashuAbc123&amount=5000&create_pod=true&service=user-app&image=nginx:alpine"
```

**Response Headers:**
- `X-Payment-Verified: true`
- `X-Payment-Amount: 5000`
- `X-Provisioned-Pod: user-app-a1b2c3d4` (if pod created)
- `X-Nostr-Event: event-id-123` (if Nostr enabled)

### POST /provision
Dedicated service provisioning endpoint

**Request:**
```json
{
  "token": "cashuAbc123",
  "amount": 5000,
  "service": "my-service",
  "namespace": "user-workloads",
  "image": "nginx:alpine"
}
```

**Response:**
```json
{
  "status": "success",
  "pod_name": "my-service-a1b2c3d4",
  "service": "my-service",
  "namespace": "user-workloads",
  "image": "nginx:alpine"
}
```

## ☸️ Kubernetes Deployment

### 1. Deploy the Complete Plugin

```bash
# Create ingress-system namespace if needed
kubectl create namespace ingress-system

# Deploy with all RBAC permissions
kubectl apply -f k8s/complete-plugin.yaml
```

### 2. Configure Features

```bash
# Enable/disable pod provisioning
kubectl patch configmap paygress-config -n ingress-system \
  --patch '{"data":{"ENABLE_POD_PROVISIONING":"true"}}'

# Enable Nostr events (optional)
kubectl patch configmap paygress-config -n ingress-system \
  --patch '{"data":{"ENABLE_NOSTR_EVENTS":"true"}}'

# Add Nostr secret key (if enabling events)
kubectl create secret generic paygress-secrets -n ingress-system \
  --from-literal=NOSTR_SECRET_KEY="your_nostr_secret_key"
```

### 3. Verify Deployment

```bash
# Check pod status
kubectl get pods -n ingress-system -l app=paygress-complete

# Check feature status
kubectl port-forward -n ingress-system svc/paygress-complete 8080:8080
curl http://localhost:8080/healthz
```

## 🔌 Ingress Integration

### NGINX Ingress Controller

```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: protected-service
  annotations:
    nginx.ingress.kubernetes.io/auth-url: "http://paygress-complete.ingress-system.svc.cluster.local:8080/auth"
    nginx.ingress.kubernetes.io/auth-response-headers: "X-Payment-Verified,X-Provisioned-Pod"
spec:
  rules:
  - host: api.example.com
    http:
      paths:
      - path: /premium
        pathType: Prefix
        backend:
          service:
            name: premium-service
            port:
              number: 80
```

### Usage Examples

#### Basic Payment Protection
```bash
curl "https://api.example.com/premium?token=cashuAbc123&amount=1000"
```

#### Payment + Pod Provisioning
```bash
curl "https://api.example.com/premium?token=cashuAbc123&amount=5000&create_pod=true&service=user-compute"
```

## 🎯 Use Cases

### 1. **Pay-per-Use API Gateway**
- Users pay with Cashu tokens for API access
- Each payment grants temporary access to protected endpoints
- Optional: Provision dedicated compute resources

### 2. **Dynamic Resource Provisioning**
- Payment verification triggers automatic pod creation
- Users get isolated compute environments
- Resources are labeled and tracked by payment

### 3. **Micropayment Infrastructure**
- Small payments (1000+ msat) for micro-services
- Automatic scaling based on payment volume
- Integration with Lightning Network via Cashu

### 4. **Event-Driven Workflows**
- Payments trigger Nostr events for coordination
- External systems react to payment notifications
- Audit trail of all transactions

## 🔍 Monitoring

### Logs
```bash
# View plugin logs
kubectl logs -n ingress-system -l app=paygress-complete -f

# View provisioned pods
kubectl get pods -n user-workloads -l payment-verified=true
```

### Metrics
```bash
# Health check
curl http://paygress-complete.ingress-system.svc.cluster.local:8080/healthz

# Feature status
kubectl get pods -n ingress-system -l app=paygress-complete -o jsonpath='{.items[0].metadata.annotations}'
```

## 🚨 Troubleshooting

### Common Issues

**Pod Provisioning Fails:**
```bash
# Check RBAC permissions
kubectl auth can-i create pods --as=system:serviceaccount:ingress-system:paygress-complete

# Check target namespace
kubectl get namespace user-workloads
```

**Nostr Events Not Publishing:**
```bash
# Check secret configuration
kubectl get secret paygress-secrets -n ingress-system -o yaml

# Verify relay connectivity in logs
kubectl logs -n ingress-system -l app=paygress-complete | grep -i nostr
```

**Payment Verification Issues:**
```bash
# Test auth endpoint directly
kubectl port-forward -n ingress-system svc/paygress-complete 8080:8080
curl "http://localhost:8080/auth?token=test&amount=1000"
```

## 🎉 What's Next?

You now have a **complete ingress plugin** that can:

1. **✅ Verify Cashu payments** - Working authentication
2. **✅ Provision Kubernetes pods** - Automatic resource creation
3. **✅ Publish Nostr events** - Payment notifications (optional)
4. **✅ Scale horizontally** - Multiple replicas supported
5. **✅ Mount on any ingress** - NGINX, Traefik, Envoy compatible

This is a **production-ready ingress plugin** that combines payments, orchestration, and events in a single service! 🚀
