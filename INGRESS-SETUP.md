# 🚀 Paygress Ingress Plugin Setup Guide

## Overview

This guide shows you how to deploy Paygress as a proper ingress plugin that:
- ✅ Verifies Cashu payments before allowing access
- ✅ Automatically provisions Kubernetes pods for paid services  
- ✅ Publishes Nostr events (optional)
- ✅ Works with NGINX Ingress Controller or Traefik

## 🎯 Architecture

```
Internet → Ingress Controller → Paygress Plugin → Your Service
                ↓                      ↓
        Payment Required         Pod Provisioning
            (if no valid token)      (if payment verified)
```

## 🚀 Quick Deployment

### 1. Deploy the Plugin

```bash
# Make deployment script executable
chmod +x deploy.sh

# Deploy everything
./deploy.sh
```

### 2. Configure Your Domain

```bash
# For local testing, add to /etc/hosts:
echo "127.0.0.1 api.example.com" | sudo tee -a /etc/hosts

# Get your ingress IP
kubectl get ingress paygress-example
```

### 3. Test the Setup

```bash
# Health check
curl http://api.example.com/healthz

# Try accessing protected endpoint (should get 402 Payment Required)
curl -v http://api.example.com/premium

# With a Cashu token (replace with real token)
curl -v "http://api.example.com/premium?token=YOUR_CASHU_TOKEN&amount=5000"
```

## 🔧 Configuration Options

### Environment Variables

The plugin supports these environment variables:

```yaml
# In k8s/ingress-plugin.yaml ConfigMap
PAYGRESS_MODE: "complete"                    # simple|complete
ENABLE_POD_PROVISIONING: "true"             # true|false
ENABLE_NOSTR_EVENTS: "false"                # true|false
DEFAULT_POD_IMAGE: "nginx:alpine"           # Default image for provisioned pods
POD_NAMESPACE: "default"                    # Where to create pods
CASHU_DB_PATH: "/data/cashu.db"             # Cashu database location
NOSTR_RELAYS: "wss://relay.damus.io,wss://nos.lol"  # Nostr relays
RUST_LOG: "info"                            # Logging level
```

### Ingress Annotations

#### NGINX Ingress Controller

```yaml
annotations:
  nginx.ingress.kubernetes.io/auth-url: "http://paygress-plugin.paygress-system.svc.cluster.local:8080/auth"
  nginx.ingress.kubernetes.io/auth-method: "GET"
  nginx.ingress.kubernetes.io/auth-response-headers: "X-Payment-Verified,X-Payment-Amount,X-Provisioned-Pod"
```

#### Traefik

```yaml
# Use middleware (see k8s/traefik-ingress.yaml)
middlewares:
- name: paygress-auth
```

## 🎯 Usage Examples

### 1. Basic Payment Gate

```yaml
# Ingress that requires payment
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: premium-api
  annotations:
    nginx.ingress.kubernetes.io/auth-url: "http://paygress-plugin.paygress-system.svc.cluster.local:8080/auth"
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

### 2. Payment + Auto Provisioning

```yaml
# Ingress that creates a pod on payment
annotations:
  nginx.ingress.kubernetes.io/auth-url: "http://paygress-plugin.paygress-system.svc.cluster.local:8080/auth?create_pod=true&service=user-workspace"
```

### 3. Different Payment Amounts

```bash
# Different routes can require different amounts
curl "http://api.example.com/basic?token=TOKEN&amount=1000"     # 1000 sats
curl "http://api.example.com/premium?token=TOKEN&amount=5000"   # 5000 sats
curl "http://api.example.com/enterprise?token=TOKEN&amount=10000" # 10000 sats
```

## 🔍 Monitoring & Debugging

### Check Plugin Status

```bash
# Plugin health
kubectl get pods -n paygress-system
kubectl logs -f deployment/paygress-plugin -n paygress-system

# Direct health check
kubectl port-forward -n paygress-system svc/paygress-plugin 8080:8080
curl http://localhost:8080/healthz
```

### Check Ingress Status

```bash
# NGINX Ingress
kubectl get ingress
kubectl describe ingress paygress-example

# Ingress controller logs
kubectl logs -n ingress-nginx deployment/ingress-nginx-controller
```

### Payment Verification Logs

```bash
# Watch payment attempts
kubectl logs -f deployment/paygress-plugin -n paygress-system | grep -E "(Payment|Cashu|Auth)"
```

## 🐛 Troubleshooting

### Common Issues

1. **502 Bad Gateway**
   ```bash
   # Check if plugin is running
   kubectl get pods -n paygress-system
   
   # Check service
   kubectl get svc -n paygress-system
   ```

2. **Payment Always Fails**
   ```bash
   # Check Cashu database
   kubectl exec -it deployment/paygress-plugin -n paygress-system -- ls -la /data/
   
   # Check logs for Cashu errors
   kubectl logs deployment/paygress-plugin -n paygress-system | grep -i cashu
   ```

3. **Pods Not Being Created**
   ```bash
   # Check RBAC permissions
   kubectl auth can-i create pods --as=system:serviceaccount:paygress-system:paygress-plugin
   
   # Check plugin configuration
   kubectl get configmap paygress-config -n paygress-system -o yaml
   ```

### Debug Mode

```bash
# Enable debug logging
kubectl patch configmap paygress-config -n paygress-system --patch '{"data":{"RUST_LOG":"debug"}}'

# Restart pods to pick up new config
kubectl rollout restart deployment/paygress-plugin -n paygress-system
```

## 🔄 Updates

### Update Plugin Image

```bash
# Build new image
docker build -t paygress:v1.1 .

# Load into cluster (kind/minikube)
kind load docker-image paygress:v1.1

# Update deployment
kubectl set image deployment/paygress-plugin -n paygress-system paygress=paygress:v1.1
```

### Update Configuration

```bash
# Edit config
kubectl edit configmap paygress-config -n paygress-system

# Restart to apply
kubectl rollout restart deployment/paygress-plugin -n paygress-system
```

## 🎯 Production Considerations

1. **Security**
   - Use TLS/HTTPS for all traffic
   - Secure Cashu database with proper permissions
   - Limit RBAC permissions to minimum required

2. **High Availability**
   - Run multiple replicas
   - Use persistent storage for Cashu database
   - Set up proper health checks

3. **Performance**
   - Monitor response times
   - Scale based on traffic
   - Consider caching verified tokens

4. **Monitoring**
   - Set up metrics collection
   - Alert on payment failures
   - Monitor pod provisioning

## 📚 API Reference

### Endpoints

- `GET /healthz` - Health check
- `GET /auth` - Payment verification (used by ingress)
- `POST /provision` - Direct pod provisioning

### Query Parameters

- `token` - Cashu token
- `amount` - Required payment amount in satoshis
- `create_pod` - Whether to create pod on success
- `service` - Service name for pod
- `image` - Docker image for pod
- `namespace` - Kubernetes namespace

### Response Headers

- `X-Payment-Verified` - true/false
- `X-Payment-Amount` - Amount paid
- `X-Provisioned-Pod` - Created pod name
- `X-Auth-Reason` - Detailed reason
