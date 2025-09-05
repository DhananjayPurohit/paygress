# Complete Traefik Plugin Import Example

## 1. Build and Prepare Plugin

```bash
# Build the plugin
./scripts/build-plugins.sh --release traefik

# Create plugin repository
mkdir -p paygress-traefik-plugin
cp target/plugins/traefik/* paygress-traefik-plugin/
```

## 2. Plugin Repository Structure

```
paygress-traefik-plugin/
├── .traefik.yml              # Plugin metadata
├── paygress.go              # Go wrapper for Traefik
├── paygress.wasm            # Compiled Rust WebAssembly
├── go.mod                   # Go module definition
└── README.md                # Installation instructions
```

### .traefik.yml
```yaml
displayName: "Paygress Payment Plugin"
type: middleware
import: github.com/your-org/paygress-traefik-plugin
summary: "Cashu payment verification and pod provisioning for Traefik"

testData:
  cashuDbPath: /tmp/cashu.db
  defaultAmount: 1000
  enablePodProvisioning: true
  podNamespace: user-workloads
  allowedImages:
    - nginx:alpine
    - httpd:alpine
```

### go.mod
```go
module github.com/your-org/paygress-traefik-plugin

go 1.21

require (
    github.com/traefik/plugindemo v0.0.0-20220919181416-85fbc9e9d4bb
)
```

## 3. Publish Plugin

```bash
cd paygress-traefik-plugin

# Initialize Git repository
git init
git add .
git commit -m "Initial Paygress Traefik plugin"

# Create GitHub repository and push
git remote add origin https://github.com/your-org/paygress-traefik-plugin.git
git branch -M main
git push -u origin main

# Tag version
git tag v1.0.0
git push origin v1.0.0
```

## 4. Configure Traefik Static Configuration

### File-based Configuration (traefik.yml)
```yaml
# Static configuration
api:
  dashboard: true
  insecure: true

entryPoints:
  web:
    address: ":80"
  websecure:
    address: ":443"

providers:
  file:
    filename: /etc/traefik/dynamic.yml
    watch: true

experimental:
  plugins:
    paygress:
      moduleName: github.com/your-org/paygress-traefik-plugin
      version: v1.0.0

log:
  level: DEBUG
```

### Dynamic Configuration (dynamic.yml)
```yaml
# Dynamic configuration
http:
  middlewares:
    # Basic payment middleware
    paygress-basic:
      plugin:
        paygress:
          cashuDbPath: /tmp/cashu.db
          defaultAmount: 1000
          enablePodProvisioning: false
          podNamespace: user-workloads
    
    # Premium payment middleware with pod provisioning
    paygress-premium:
      plugin:
        paygress:
          cashuDbPath: /tmp/cashu.db
          defaultAmount: 2000
          enablePodProvisioning: true
          podNamespace: premium-workloads
          defaultPodImage: nginx:alpine
          allowedImages:
            - nginx:alpine
            - httpd:alpine
            - redis:alpine

  routers:
    # Premium API router
    premium-api:
      rule: "Host(`api.example.com`) && PathPrefix(`/premium`)"
      middlewares:
        - paygress-premium
      service: premium-service
      entryPoints:
        - web

    # Basic API router  
    basic-api:
      rule: "Host(`api.example.com`) && PathPrefix(`/basic`)"
      middlewares:
        - paygress-basic
      service: basic-service
      entryPoints:
        - web

    # Public API router (no payment required)
    public-api:
      rule: "Host(`api.example.com`) && PathPrefix(`/public`)"
      service: public-service
      entryPoints:
        - web

  services:
    premium-service:
      loadBalancer:
        servers:
          - url: "http://premium-backend:80"

    basic-service:
      loadBalancer:
        servers:
          - url: "http://basic-backend:80"

    public-service:
      loadBalancer:
        servers:
          - url: "http://public-backend:80"
```

## 5. Kubernetes Deployment

### Traefik with Plugin
```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: traefik-config
  namespace: traefik
data:
  traefik.yml: |
    api:
      dashboard: true
    
    entryPoints:
      web:
        address: ":80"
    
    providers:
      kubernetescrd: {}
    
    experimental:
      plugins:
        paygress:
          moduleName: github.com/your-org/paygress-traefik-plugin
          version: v1.0.0

---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: traefik
  namespace: traefik
spec:
  replicas: 1
  selector:
    matchLabels:
      app: traefik
  template:
    metadata:
      labels:
        app: traefik
    spec:
      containers:
      - name: traefik
        image: traefik:v3.0
        args:
          - --configfile=/config/traefik.yml
        ports:
        - containerPort: 80
        - containerPort: 8080
        volumeMounts:
        - name: config
          mountPath: /config
      volumes:
      - name: config
        configMap:
          name: traefik-config

---
# Middleware definition
apiVersion: traefik.containo.us/v1alpha1
kind: Middleware
metadata:
  name: paygress-auth
  namespace: default
spec:
  plugin:
    paygress:
      cashuDbPath: /tmp/cashu.db
      defaultAmount: 1000
      enablePodProvisioning: true
      podNamespace: user-workloads

---
# IngressRoute using the plugin
apiVersion: traefik.containo.us/v1alpha1
kind: IngressRoute
metadata:
  name: premium-api
  namespace: default
spec:
  entryPoints:
    - web
  routes:
  - match: Host(`api.example.com`) && PathPrefix(`/premium`)
    kind: Rule
    middlewares:
    - name: paygress-auth
    services:
    - name: premium-service
      port: 80
```

## 6. Test the Plugin

```bash
# Start Traefik
traefik --configfile=traefik.yml

# Test without payment (should fail)
curl -H "Host: api.example.com" http://localhost/premium

# Test with payment (should succeed)
curl -H "Host: api.example.com" \
     -H "X-Cashu-Token: cashuAeyJ0eXAiOiJ..." \
     -H "X-Payment-Amount: 1000" \
     http://localhost/premium

# Test pod provisioning
curl -H "Host: api.example.com" \
     -H "X-Cashu-Token: cashuAeyJ0eXAiOiJ..." \
     -H "X-Payment-Amount: 2000" \
     -H "X-Create-Pod: true" \
     -H "X-Service-Name: my-workspace" \
     http://localhost/premium

# Check Traefik logs
docker logs traefik-container -f
```

## 7. Advanced Configuration

### Multiple Payment Tiers
```yaml
http:
  middlewares:
    paygress-micro:
      plugin:
        paygress:
          defaultAmount: 100
          enablePodProvisioning: false
    
    paygress-standard:
      plugin:
        paygress:
          defaultAmount: 1000
          enablePodProvisioning: false
    
    paygress-enterprise:
      plugin:
        paygress:
          defaultAmount: 5000
          enablePodProvisioning: true
          defaultPodImage: enterprise-app:latest

  routers:
    micro-tier:
      rule: "Host(`api.example.com`) && PathPrefix(`/micro`)"
      middlewares: ["paygress-micro"]
      service: micro-service
    
    standard-tier:
      rule: "Host(`api.example.com`) && PathPrefix(`/standard`)"
      middlewares: ["paygress-standard"]
      service: standard-service
    
    enterprise-tier:
      rule: "Host(`api.example.com`) && PathPrefix(`/enterprise`)"
      middlewares: ["paygress-enterprise"]
      service: enterprise-service
```

## 8. Monitoring and Debugging

### Enable Plugin Debugging
```yaml
# In traefik.yml
log:
  level: DEBUG
  format: json

accessLog:
  format: json
  fields:
    headers:
      names:
        X-Cashu-Token: keep
        X-Payment-Amount: keep
        X-Payment-Verified: keep
```

### Check Plugin Status
```bash
# Traefik API to check plugins
curl http://localhost:8080/api/plugins

# Check middleware status
curl http://localhost:8080/api/http/middlewares

# View logs
journalctl -u traefik -f
```

The plugin is now fully integrated into Traefik and will automatically verify Cashu payments for any routes that use the paygress middleware.
