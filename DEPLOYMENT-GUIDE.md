# Paygress Sidecar Deployment Guide

## ✅ Prerequisites Check

The deployment script found that:
- ✅ kubectl is available
- ✅ Cluster connectivity is working
- ✅ Namespaces `ingress-system` and `user-workloads` exist
- ✅ Dockerfile is now created

## 🚀 Next Steps

### 1. Make the script executable and run it:
```bash
chmod +x deploy-sidecar.sh
./deploy-sidecar.sh
```

### 2. Alternative: Manual deployment

If you prefer to deploy manually:

```bash
# Build Docker image
docker build -t paygress:latest .

# Load into cluster (if using kind)
kind load docker-image paygress:latest

# Or for minikube
minikube image load paygress:latest

# Apply Kubernetes manifests
kubectl apply -f k8s/sidecar-service.yaml

# Wait for deployment
kubectl wait --for=condition=available --timeout=300s \
    deployment/paygress-sidecar -n ingress-system
```

### 3. Verify deployment:
```bash
# Check pods
kubectl get pods -n ingress-system -l app=paygress-sidecar

# Check service
kubectl get svc -n ingress-system -l app=paygress-sidecar

# Check logs
kubectl logs -n ingress-system -l app=paygress-sidecar
```

### 4. Test the service:
```bash
# Port forward to access locally
kubectl port-forward -n ingress-system svc/paygress-sidecar 8080:8080 &

# Test health endpoint
curl http://localhost:8080/healthz

# Test with demo script
chmod +x examples/sidecar_demo.sh
./examples/sidecar_demo.sh
```

## 🔧 Configuration

The sidecar service uses these default settings:
- Payment rate: 100 sats/hour
- Default duration: 60 minutes
- SSH port: 2222
- Pod namespace: user-workloads

You can modify these in `k8s/sidecar-service.yaml` in the ConfigMap section.

## 🎯 Key Features Working

Your sidecar service now provides:

1. **💰 Payment Verification**: Validates Cashu tokens before pod creation
2. **🚀 SSH Pod Spawning**: Creates pods with SSH access and unique credentials
3. **⏰ Time-based Lifecycle**: Automatically cleans up pods when payment expires
4. **🔧 Configurable Rates**: Easy to adjust payment rates and durations
5. **🌐 Ingress Integration**: Works as auth sidecar for any ingress controller

## 📝 Example Usage

Once deployed, you can spawn SSH pods like this:

```bash
curl -X POST http://localhost:8080/spawn-pod \
  -H "Content-Type: application/json" \
  -d '{
    "cashu_token": "your_cashu_token_here",
    "duration_minutes": 120,
    "ssh_username": "developer"
  }'
```

The response will include SSH connection details:
```json
{
  "success": true,
  "pod_info": {
    "pod_name": "ssh-pod-a1b2c3d4",
    "ssh_username": "developer", 
    "ssh_password": "GeneratedPass123",
    "ssh_port": 2222,
    "expires_at": "2024-01-15T12:30:00Z"
  }
}
```

## 🔍 Troubleshooting

If the deployment fails:

1. **Check Docker**: `docker --version`
2. **Check cluster type**: `kubectl cluster-info`
3. **Check RBAC**: Ensure your kubectl user has cluster-admin permissions
4. **Check resources**: `kubectl describe pod -n ingress-system -l app=paygress-sidecar`

The comprehensive README-SIDECAR.md file contains detailed troubleshooting steps and usage examples.
