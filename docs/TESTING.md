# Testing Paygress Native Ingress Plugins

This guide shows you how to test your native ingress plugins after importing them into your ingress controller.

## 🚀 Quick Test

```bash
# Test all plugins with default settings
./scripts/test-plugins.sh

# Test specific ingress controller
./scripts/test-plugins.sh --ingress nginx
./scripts/test-plugins.sh --ingress traefik  
./scripts/test-plugins.sh --ingress envoy
```

## 📋 Test Types

### 1. Basic Payment Tests
Verify core payment functionality:

```bash
./scripts/test-plugins.sh basic
```

**What it tests:**
- ❌ Access without payment (should return 401/402)
- ✅ Access with valid Cashu token (should return 200)
- ✅ Public endpoints work without payment
- ✅ Response headers contain payment verification info

### 2. Pod Provisioning Tests
Test dynamic pod creation:

```bash
./scripts/test-plugins.sh pod
```

**What it tests:**
- ✅ Pod creation on successful payment
- ✅ Pod appears in Kubernetes with correct labels
- ✅ Pod provisioning headers in response
- ✅ Resource limits and security policies

### 3. Security Tests
Validate security measures:

```bash
./scripts/test-plugins.sh security
```

**What it tests:**
- ❌ Invalid token formats rejected
- ❌ Insufficient payment amounts rejected
- ❌ Malicious container images blocked
- ✅ Input validation and sanitization

### 4. Stress Tests
Performance and reliability:

```bash
./scripts/test-plugins.sh stress
```

**What it tests:**
- ⚡ Concurrent request handling
- 📊 Request per second (RPS) metrics
- 🔄 Plugin stability under load
- 💾 Memory usage patterns

## 🔧 Manual Testing

### Generate Test Cashu Token

For testing, you can use this sample token:
```bash
TEST_TOKEN="cashuAeyJ0eXAiOiJwcm9vZiIsImFsZyI6IkhTMjU2In0.eyJhbW91bnQiOjEwMDAsInNlY3JldCI6InRlc3Rfc2VjcmV0In0.test_signature"
```

### Test Different Payment Scenarios

#### 1. Basic Payment Verification

```bash
# Test protected endpoint
curl -H "Host: api.example.com" \
     -H "X-Cashu-Token: $TEST_TOKEN" \
     -H "X-Payment-Amount: 1000" \
     http://your-ingress/premium

# Expected: HTTP 200 with payment headers
```

#### 2. Pod Provisioning

```bash
# Request pod creation
curl -H "Host: api.example.com" \
     -H "X-Cashu-Token: $TEST_TOKEN" \
     -H "X-Payment-Amount: 2000" \
     -H "X-Create-Pod: true" \
     -H "X-Pod-Image: nginx:alpine" \
     -H "X-Service-Name: my-workspace" \
     http://your-ingress/workspace

# Expected: HTTP 200 with X-Provisioned-Pod header
```

#### 3. Different Payment Amounts

```bash
# Basic tier (500 msat)
curl -H "X-Cashu-Token: $TEST_TOKEN" \
     -H "X-Payment-Amount: 500" \
     http://your-ingress/basic

# Premium tier (2000 msat)  
curl -H "X-Cashu-Token: $TEST_TOKEN" \
     -H "X-Payment-Amount: 2000" \
     http://your-ingress/premium

# Enterprise tier (5000 msat + pod)
curl -H "X-Cashu-Token: $TEST_TOKEN" \
     -H "X-Payment-Amount: 5000" \
     -H "X-Create-Pod: true" \
     http://your-ingress/enterprise
```

## 🔍 Testing Each Plugin Type

### NGINX Plugin Testing

#### 1. Check Plugin Loading
```bash
# Verify module is loaded
nginx -V 2>&1 | grep -o with-http_auth_request_module
sudo nginx -t

# Check error logs
sudo tail -f /var/log/nginx/error.log
```

#### 2. Test Configuration
```nginx
server {
    listen 80;
    server_name test.local;
    
    # Enable debug logging
    error_log /var/log/nginx/paygress_debug.log debug;
    
    location /test {
        paygress_enable on;
        paygress_default_amount 1000;
        return 200 "Payment verified\n";
        add_header Content-Type text/plain;
    }
}
```

#### 3. Debug Commands
```bash
# Test configuration
sudo nginx -t

# Reload configuration  
sudo nginx -s reload

# Check module status
curl -H "X-Cashu-Token: test" http://test.local/test
```

### Traefik Plugin Testing

#### 1. Check Plugin Loading
```bash
# Verify plugin is loaded in Traefik logs
kubectl logs -n traefik deployment/traefik | grep paygress

# Check plugin status
curl http://traefik-dashboard:8080/api/plugins
```

#### 2. Test Configuration
```yaml
# Test middleware
apiVersion: traefik.containo.us/v1alpha1
kind: Middleware
metadata:
  name: paygress-test
spec:
  plugin:
    paygress:
      cashuDbPath: /tmp/cashu.db
      defaultAmount: 1000
      enablePodProvisioning: false  # Disable for testing
```

#### 3. Debug Commands
```bash
# Test middleware
curl -H "Host: test.local" \
     -H "X-Cashu-Token: test" \
     http://traefik-ingress/test

# Check middleware logs
kubectl logs -n traefik deployment/traefik -f
```

### Envoy/Istio Plugin Testing

#### 1. Check Plugin Loading
```bash
# Verify WASM module is loaded
kubectl logs -n istio-system deployment/istiod | grep paygress

# Check Envoy config
istioctl proxy-config listener <pod-name> -n <namespace>
```

#### 2. Test Configuration
```bash
# Apply test EnvoyFilter
kubectl apply -f - <<EOF
apiVersion: networking.istio.io/v1alpha3
kind: EnvoyFilter
metadata:
  name: paygress-test
  namespace: test
spec:
  workloadSelector:
    labels:
      app: test-app
  configPatches:
  - applyTo: HTTP_FILTER
    match:
      context: SIDECAR_INBOUND
    patch:
      operation: INSERT_BEFORE
      value:
        name: envoy.filters.http.wasm
        typed_config:
          "@type": type.googleapis.com/envoy.extensions.filters.http.wasm.v3.Wasm
          config:
            name: "paygress"
            code:
              local:
                filename: "/etc/envoy/paygress.wasm"
EOF
```

#### 3. Debug Commands
```bash
# Test through Istio gateway
curl -H "X-Cashu-Token: test" \
     http://istio-gateway/test

# Check Envoy logs
kubectl logs <pod-name> -c istio-proxy -f
```

## 📊 Monitoring and Metrics

### Check Plugin Metrics

```bash
# NGINX metrics
curl http://nginx-server/nginx_status

# Traefik metrics
curl http://traefik:8080/metrics | grep paygress

# Envoy metrics  
curl http://envoy-admin:15000/stats/prometheus | grep paygress
```

### Key Metrics to Monitor

```
# Payment verification metrics
paygress_payments_total{status="success"}
paygress_payments_total{status="failed"}
paygress_payment_duration_seconds

# Pod provisioning metrics
paygress_pods_created_total
paygress_pods_failed_total
paygress_pod_creation_duration_seconds

# Performance metrics
paygress_request_duration_seconds
paygress_memory_usage_bytes
paygress_cpu_usage_percent
```

## 🐛 Troubleshooting

### Common Issues

#### 1. Plugin Not Loading
```bash
# Check plugin file exists
ls -la /path/to/plugin/file

# Check permissions
sudo chown nginx:nginx /path/to/plugin.so
sudo chmod 755 /path/to/plugin.so

# Check dependencies
ldd /path/to/plugin.so
```

#### 2. Payment Verification Fails
```bash
# Check Cashu database
sqlite3 /tmp/cashu.db "SELECT * FROM tokens LIMIT 5;"

# Verify token format
echo "cashuAeyJ0eXAiOiJ..." | base64 -d | jq .

# Check plugin logs
tail -f /var/log/nginx/error.log | grep paygress
```

#### 3. Pod Creation Fails
```bash
# Check Kubernetes permissions
kubectl auth can-i create pods --as=system:serviceaccount:ingress:nginx

# Check pod events
kubectl get events --sort-by=.metadata.creationTimestamp

# Verify image availability
kubectl run test --image=nginx:alpine --dry-run=client
```

#### 4. Performance Issues
```bash
# Check resource usage
top -p $(pgrep nginx)
kubectl top pods -n ingress-system

# Profile plugin
perf record -g curl -H "X-Cashu-Token: test" http://test/premium
perf report
```

### Debug Configuration

```yaml
# Enable debug mode in plugin config
debug: true
logLevel: debug
verboseLogging: true

# Increase log verbosity
RUST_LOG=debug,paygress=trace
```

## 📈 Performance Testing

### Load Testing with Apache Bench

```bash
# Basic load test
ab -n 1000 -c 10 \
   -H "X-Cashu-Token: $TEST_TOKEN" \
   -H "X-Payment-Amount: 1000" \
   http://your-ingress/premium

# Sustained load test
ab -n 10000 -c 100 -t 60 \
   -H "X-Cashu-Token: $TEST_TOKEN" \
   http://your-ingress/premium
```

### Load Testing with wrk

```bash
# Install wrk
sudo apt-get install wrk

# Run load test
wrk -t10 -c100 -d30s \
    -H "X-Cashu-Token: $TEST_TOKEN" \
    -H "X-Payment-Amount: 1000" \
    http://your-ingress/premium

# With Lua script for dynamic tokens
wrk -t10 -c100 -d30s -s token_script.lua http://your-ingress/premium
```

### Load Testing Script

```lua
-- token_script.lua
local tokens = {
    "cashuAeyJ0eXAiOiJwcm9vZiIsImFsZyI6IkhTMjU2In0.test1",
    "cashuAeyJ0eXAiOiJwcm9vZiIsImFsZyI6IkhTMjU2In0.test2",
    "cashuAeyJ0eXAiOiJwcm9vZiIsImFsZyI6IkhTMjU2In0.test3"
}

request = function()
    local token = tokens[math.random(1, #tokens)]
    local headers = {}
    headers["X-Cashu-Token"] = token
    headers["X-Payment-Amount"] = "1000"
    return wrk.format("GET", nil, headers)
end
```

## ✅ Test Checklist

Before deploying to production:

- [ ] Plugin loads successfully
- [ ] Basic payment verification works
- [ ] Invalid tokens are rejected
- [ ] Pod provisioning works (if enabled)
- [ ] Security policies are enforced
- [ ] Performance meets requirements
- [ ] Monitoring and metrics work
- [ ] Error handling is appropriate
- [ ] Resource limits are respected
- [ ] Logs are properly formatted

## 🎯 Next Steps

After successful testing:

1. **Deploy to staging** with production-like traffic
2. **Set up monitoring** and alerting
3. **Configure backup** and disaster recovery
4. **Document** operational procedures
5. **Train team** on troubleshooting
6. **Plan rollout** strategy for production

The testing framework ensures your Paygress plugins work correctly and securely before handling real payments and provisioning actual workloads.
