# Paygress Sidecar Service

A Kubernetes sidecar service that verifies Cashu payments and spawns SSH-accessible pods with configurable payment rates and time-based lifecycle management.

## Features

- 🔐 **Cashu Payment Verification**: Verify Cashu tokens for pod access
- ⚡ **Configurable Payment Rates**: Set custom sats/hour rates
- 🚀 **SSH Pod Provisioning**: Spawn pods with SSH access
- ⏰ **Time-based Lifecycle**: Automatic pod cleanup after payment period
- 🔑 **Secure Access**: Generated SSH credentials per pod
- 📊 **Pod Management**: Track and manage active pods
- 🌐 **Ingress Integration**: Works as auth sidecar for NGINX ingress

## Quick Start

### 1. Deploy the Sidecar Service

```bash
# Apply the Kubernetes manifests
kubectl apply -f k8s/sidecar-service.yaml

# Check the deployment
kubectl get pods -n ingress-system -l app=paygress-sidecar
```

### 2. Build and Deploy the Container

```bash
# Build the Docker image
docker build -t paygress:latest .

# Load into your cluster (if using kind/minikube)
kind load docker-image paygress:latest
# OR for minikube
minikube image load paygress:latest
```

### 3. Verify Service is Running

```bash
# Check health endpoint
kubectl port-forward -n ingress-system svc/paygress-sidecar 8080:8080 &
curl http://localhost:8080/healthz
```

## Configuration

The sidecar service is configured via environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `PAYGRESS_MODE` | `sidecar` | Service mode |
| `BIND_ADDR` | `0.0.0.0:8080` | Listen address |
| `CASHU_DB_PATH` | `./cashu.db` | Cashu database path |
| `POD_NAMESPACE` | `user-workloads` | Namespace for spawned pods |
| `PAYMENT_RATE_SATS_PER_HOUR` | `100` | Payment rate in sats/hour |
| `DEFAULT_POD_DURATION_MINUTES` | `60` | Default pod duration |
| `SSH_BASE_IMAGE` | `linuxserver/openssh-server:latest` | SSH server image |
| `SSH_PORT` | `2222` | SSH port for pods |
| `ENABLE_CLEANUP_TASK` | `true` | Enable automatic cleanup |

## API Endpoints

### 1. Health Check
```bash
GET /healthz
```

**Response:**
```json
{
  "status": "healthy",
  "service": "paygress-sidecar",
  "version": "0.1.0",
  "config": {
    "payment_model": "1 sat = 1 minute",
    "minimum_payment": "1 sat",
    "namespace": "user-workloads"
  },
  "active_pods": 3
}
```

### 2. Auth Verification (for Ingress)
```bash
GET /auth?token=CASHU_TOKEN&duration_minutes=60
```

### 3. Spawn SSH Pod
```bash
POST /spawn-pod
Content-Type: application/json

{
  "cashu_token": "cashuAeyJ0b2tlbiI6W3sibWludCI6Imh0dHA...",
  "duration_minutes": 120,
  "pod_image": "linuxserver/openssh-server:latest",
  "ssh_username": "myuser"
}
```

**Response:**
```json
{
  "success": true,
  "message": "Pod created successfully. SSH access available for 120 minutes",
  "pod_info": {
    "pod_name": "ssh-pod-a1b2c3d4",
    "namespace": "user-workloads",
    "created_at": "2024-01-15T10:30:00Z",
    "expires_at": "2024-01-15T12:30:00Z",
    "ssh_port": 2222,
    "ssh_username": "myuser",
    "ssh_password": "SecurePass123",
    "payment_amount_sats": 200,
    "duration_minutes": 120
  }
}
```

### 4. List Active Pods
```bash
GET /pods
```

**Response:**
```json
[
  {
    "pod_name": "ssh-pod-a1b2c3d4",
    "namespace": "user-workloads",
    "created_at": "2024-01-15T10:30:00Z",
    "expires_at": "2024-01-15T12:30:00Z",
    "ssh_port": 2222,
    "ssh_username": "myuser",
    "ssh_password": "SecurePass123",
    "payment_amount_sats": 200,
    "duration_minutes": 120
  }
]
```

### 5. Get Pod Info
```bash
GET /pods/ssh-pod-a1b2c3d4
```

### 6. Get Port-Forward Command
```bash
GET /pods/ssh-pod-a1b2c3d4/port-forward
```

**Response:**
```json
{
  "pod_name": "ssh-pod-a1b2c3d4",
  "ssh_port": 22001,
  "port_forward_command": "kubectl -n user-workloads port-forward svc/ssh-pod-a1b2c3d4-ssh 22001:22001",
  "ssh_command": "ssh -o PreferredAuthentications=password -o PubkeyAuthentication=no testuser@localhost -p 22001",
  "instructions": [
    "Run: kubectl -n user-workloads port-forward svc/ssh-pod-a1b2c3d4-ssh 22001:22001",
    "In another terminal, run: ssh -o PreferredAuthentications=password -o PubkeyAuthentication=no testuser@localhost -p 22001",
    "Password: SecurePass123",
    "Keep port-forward running for SSH access"
  ]
}
```

## Unique SSH Ports

Each spawned pod gets a **unique SSH port** in the range **22000-22999**. This prevents port conflicts when running multiple pods.

## Usage Examples

### Example 1: Spawn a 2-hour SSH Pod

```bash
# Payment calculation: 1 sat = 1 minute, so 120 minutes = 120 sats
# Create a Cashu token worth at least 120 sats

curl -X POST http://localhost:8080/spawn-pod \
  -H "Content-Type: application/json" \
  -d '{
    "cashu_token": "cashuAeyJ0b2tlbiI6W3sibWludCI6Imh0dHA..."
  }'
```

### Example 2: Spawn with Custom SSH Username

```bash
curl -X POST http://localhost:8080/spawn-pod \
  -H "Content-Type: application/json" \
  -d '{
    "cashu_token": "cashuAeyJ0b2tlbiI6W3sibWludCI6Imh0dHA...",
    "duration_minutes": 60,
    "ssh_username": "developer"
  }'
```

### Example 3: Connect to SSH Pod

After spawning a pod, use the returned credentials:

```bash
# Get the pod's NodePort
kubectl get svc -n user-workloads ssh-pod-a1b2c3d4-ssh

# Connect via SSH
ssh myuser@<node-ip> -p <nodeport>
# Password: SecurePass123 (from API response)
```

### Example 4: Monitor Pod Status

```bash
# List all active pods
curl http://localhost:8080/pods

# Get specific pod info
curl http://localhost:8080/pods/ssh-pod-a1b2c3d4

# Check Kubernetes pod status
kubectl get pods -n user-workloads -l app=paygress-ssh-pod
```

### Example 4: Automatic SSH Setup

```bash
# Use the automated setup script
./examples/setup-ssh.sh ssh-pod-a1b2c3d4

# Or get port-forward instructions manually
curl http://localhost:8080/pods/ssh-pod-a1b2c3d4/port-forward | jq .
```

## Payment Calculation

The service calculates required payment based on duration:

```
Required Sats = (Duration in Hours) × (Rate per Hour)
```

Examples with default rate of 100 sats/hour:
- 30 minutes = 0.5 hours = 50 sats
- 1 hour = 1 hour = 100 sats
- 2 hours = 2 hours = 200 sats
- 1 day = 24 hours = 2,400 sats

## Ingress Integration

The sidecar can be used as an auth service for NGINX ingress:

```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: protected-ssh-service
  annotations:
    nginx.ingress.kubernetes.io/auth-url: "http://paygress-sidecar.ingress-system.svc.cluster.local:8080/auth"
    nginx.ingress.kubernetes.io/auth-method: "GET"
spec:
  rules:
  - host: ssh.example.com
    http:
      paths:
      - path: /
        pathType: Prefix
        backend:
          service:
            name: your-service
            port:
              number: 80
```

## Pod Lifecycle Management

### Automatic Cleanup
- Pods are automatically deleted when their payment period expires
- A background cleanup task runs every minute to remove expired pods
- Both the pod and its associated SSH service are cleaned up

### Manual Management
```bash
# List active pods
kubectl get pods -n user-workloads -l managed-by=paygress-sidecar

# Delete a specific pod manually
kubectl delete pod ssh-pod-a1b2c3d4 -n user-workloads

# Check pod logs
kubectl logs ssh-pod-a1b2c3d4 -n user-workloads
```

## Security Considerations

### SSH Security
- Each pod gets a unique, randomly generated password
- SSH keys can be added via volume mounts if needed
- Pods run with restricted security contexts
- Network policies can be applied to limit pod communication

### Payment Security
- Cashu tokens are verified against the mint
- Double-spending protection via token tracking
- Payment amounts are validated before pod creation

### Kubernetes Security
- Service account with minimal required permissions
- Pods run as non-root user (1000)
- Read-only root filesystem
- Capability dropping for security

## Troubleshooting

### Common Issues

1. **Pod Creation Fails**
   ```bash
   # Check service account permissions
   kubectl auth can-i create pods --as=system:serviceaccount:ingress-system:paygress-sidecar -n user-workloads
   
   # Check logs
   kubectl logs -n ingress-system -l app=paygress-sidecar
   ```

2. **SSH Connection Fails**
   ```bash
   # Check if pod is running
   kubectl get pods -n user-workloads
   
   # Check service
   kubectl get svc -n user-workloads
   
   # Check pod logs
   kubectl logs <pod-name> -n user-workloads
   ```

3. **Payment Verification Fails**
   ```bash
   # Check Cashu database
   kubectl exec -n ingress-system deployment/paygress-sidecar -- ls -la /app/data/
   
   # Verify token manually
   curl -X GET "http://localhost:8080/auth?token=YOUR_TOKEN&duration_minutes=60"
   ```

### Logs and Monitoring

```bash
# View sidecar service logs
kubectl logs -f -n ingress-system -l app=paygress-sidecar

# View SSH pod logs
kubectl logs -f -n user-workloads <pod-name>

# Monitor resource usage
kubectl top pods -n user-workloads
kubectl top pods -n ingress-system -l app=paygress-sidecar
```

## Development

### Local Development

```bash
# Run locally with Docker Compose
docker-compose up

# Or run with Cargo
PAYGRESS_MODE=sidecar cargo run
```

### Testing

```bash
# Unit tests
cargo test

# Integration tests with local Kubernetes
make test-integration
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Submit a pull request

## License

MIT License - see LICENSE file for details.
