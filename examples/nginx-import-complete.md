# Complete NGINX Plugin Import Example

## 1. Build and Install

```bash
# Build the plugin
./scripts/build-plugins.sh --release nginx

# Install into NGINX
sudo cp target/plugins/nginx/ngx_http_paygress_module.so /etc/nginx/modules/
sudo chmod 644 /etc/nginx/modules/ngx_http_paygress_module.so
```

## 2. Complete nginx.conf

```nginx
# Load the Paygress module at the top
load_module modules/ngx_http_paygress_module.so;

events {
    worker_connections 1024;
}

http {
    include       /etc/nginx/mime.types;
    default_type  application/octet-stream;
    
    # Logging
    log_format paygress '$remote_addr - $remote_user [$time_local] "$request" '
                       '$status $body_bytes_sent "$http_referer" '
                       '"$http_user_agent" "$http_x_cashu_token" '
                       '"$sent_http_x_payment_verified"';
    
    access_log /var/log/nginx/paygress.log paygress;
    
    server {
        listen 80;
        server_name api.example.com;
        
        # Global Paygress configuration
        paygress_enable on;
        paygress_cashu_db_path /var/lib/nginx/cashu.db;
        paygress_default_amount 1000;
        paygress_enable_pod_provisioning on;
        paygress_pod_namespace user-workloads;
        
        # Premium API (requires payment)
        location /premium {
            proxy_pass http://premium-backend;
            proxy_set_header Host $host;
            proxy_set_header X-Real-IP $remote_addr;
            proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        }
        
        # Enterprise API (higher payment + pod provisioning)
        location /enterprise {
            paygress_default_amount 5000;
            paygress_enable_pod_provisioning on;
            proxy_pass http://enterprise-backend;
        }
        
        # Public API (no payment required)
        location /public {
            paygress_enable off;
            proxy_pass http://public-backend;
        }
        
        # Health check
        location /health {
            paygress_enable off;
            return 200 "OK\n";
            add_header Content-Type text/plain;
        }
    }
    
    # Backend definitions
    upstream premium-backend {
        server premium-service:80;
    }
    
    upstream enterprise-backend {
        server enterprise-service:80;
    }
    
    upstream public-backend {
        server public-service:80;
    }
}
```

## 3. Apply Configuration

```bash
# Test configuration
sudo nginx -t

# Apply changes
sudo systemctl reload nginx

# Verify module is loaded
nginx -V 2>&1 | grep -o "paygress"
```

## 4. Test the Plugin

```bash
# Should fail (no payment)
curl -H "Host: api.example.com" http://localhost/premium

# Should succeed
curl -H "Host: api.example.com" \
     -H "X-Cashu-Token: cashuAeyJ0eXAiOiJ..." \
     -H "X-Payment-Amount: 1000" \
     http://localhost/premium

# Check logs
sudo tail -f /var/log/nginx/paygress.log
```

## 5. Kubernetes Integration

If using NGINX Ingress Controller in Kubernetes:

```yaml
# ConfigMap for NGINX configuration
apiVersion: v1
kind: ConfigMap
metadata:
  name: nginx-paygress-config
  namespace: ingress-nginx
data:
  load-modules: |
    load_module modules/ngx_http_paygress_module.so;
  
  main-snippet: |
    load_module modules/ngx_http_paygress_module.so;

---
# Patch NGINX Ingress Controller deployment
apiVersion: apps/v1
kind: Deployment
metadata:
  name: ingress-nginx-controller
  namespace: ingress-nginx
spec:
  template:
    spec:
      initContainers:
      - name: install-paygress-plugin
        image: busybox
        command:
        - sh
        - -c
        - |
          cp /plugin/ngx_http_paygress_module.so /modules/
          chmod 644 /modules/ngx_http_paygress_module.so
        volumeMounts:
        - name: plugin-volume
          mountPath: /plugin
        - name: nginx-modules
          mountPath: /modules
      containers:
      - name: controller
        volumeMounts:
        - name: nginx-modules
          mountPath: /etc/nginx/modules
      volumes:
      - name: plugin-volume
        configMap:
          name: paygress-plugin-binary
      - name: nginx-modules
        emptyDir: {}

---
# Ingress with plugin configuration
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: premium-api
  annotations:
    nginx.ingress.kubernetes.io/server-snippet: |
      paygress_enable on;
      paygress_default_amount 1000;
      paygress_enable_pod_provisioning on;
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

The plugin is now fully integrated into NGINX and will intercept all requests to verify Cashu payments before allowing access to your services.
