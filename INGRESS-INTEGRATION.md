# Paygress Ingress Controller Integration

🎯 **The Right Way: External Authentication Pattern**

Your Paygress service integrates with **any ingress controller** using the **external auth pattern**. This is the **recommended approach** for production systems.

## 🏗️ Architecture

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   Client        │    │ Ingress Controller│    │ Your App        │
│                 │    │ (NGINX/Traefik)  │    │                 │
└─────────┬───────┘    └─────────┬────────┘    └─────────┬───────┘
          │                      │                       │
          │ 1. Request           │                       │
          ├─────────────────────►│                       │
          │                      │                       │
          │                      │ 2. Auth Check         │
          │                      ├──────────────┐        │
          │                      │              │        │
          │                      │              ▼        │
          │                      │    ┌─────────────────┐ │
          │                      │    │ Paygress Auth   │ │
          │                      │    │ Service         │ │
          │                      │    │ - Verify Cashu  │ │
          │                      │    │ - Provision Pod │ │
          │                      │    └─────────────────┘ │
          │                      │              │        │
          │                      │ 3. Auth Result         │
          │                      │◄─────────────┘        │
          │                      │                       │
          │                      │ 4. Forward (if OK)    │
          │                      ├──────────────────────►│
          │                      │                       │
          │ 5. Response          │                       │
          │◄─────────────────────┤                       │
```

## 🚀 Quick Deploy

### **1. Deploy Paygress Service:**
```bash
kubectl apply -f k8s/nginx-ingress-paygress.yaml
```

### **2. Configure Your Domain:**
```bash
# Edit the ingress
kubectl edit ingress paygress-protected-ingress

# Change 'your-domain.com' to your actual domain
```

### **3. Test the Integration:**
```bash
# Free content (no payment required)
curl http://your-domain.com/free

# Premium content (payment required)
curl http://your-domain.com/premium
# → 402 Payment Required

# Premium content with Cashu token
curl -H "Authorization: Bearer <cashu-token>" http://your-domain.com/premium
# → Access granted + pod provisioned
```

## 🔧 Ingress Controller Support

### **NGINX Ingress Controller** ✅
```yaml
annotations:
  nginx.ingress.kubernetes.io/auth-url: "http://paygress-auth.paygress-system.svc.cluster.local:8080/auth"
  nginx.ingress.kubernetes.io/auth-method: "GET"
```

### **Traefik** ✅
```yaml
apiVersion: traefik.containo.us/v1alpha1
kind: Middleware
metadata:
  name: paygress-auth
spec:
  forwardAuth:
    address: http://paygress-auth.paygress-system.svc.cluster.local:8080/auth
```

### **Istio/Envoy** ✅
```yaml
apiVersion: security.istio.io/v1beta1
kind: AuthorizationPolicy
metadata:
  name: paygress-auth
spec:
  action: CUSTOM
  provider:
    name: "paygress-ext-authz"
```

## 📋 Configuration Options

### **Payment Amounts:**
```yaml
env:
- name: DEFAULT_PAYMENT_AMOUNT
  value: "1000"  # 1000 satoshis
```

### **Pod Provisioning:**
```yaml
env:
- name: ENABLE_POD_PROVISIONING
  value: "true"
- name: POD_NAMESPACE
  value: "user-workloads"
```

### **Custom Headers:**
```yaml
nginx.ingress.kubernetes.io/auth-response-headers: |
  X-Payment-Verified,X-Payment-Amount,X-Provisioned-Pod,X-User-ID
```

## 🔍 How It Works

### **1. Request Flow:**
1. Client makes request to protected endpoint
2. Ingress controller intercepts request
3. Makes auth check to Paygress service: `GET /auth`
4. Paygress verifies Cashu token from `Authorization` header
5. If valid: provisions pod + returns 200
6. If invalid: returns 402 with payment requirements
7. Ingress forwards request to backend (if authorized)

### **2. Auth Endpoint:**
```bash
GET /auth
Headers:
  Authorization: Bearer <cashu-token>
  X-Original-URL: https://domain.com/premium
  X-Forwarded-Method: GET

Response (Success):
  Status: 200
  Headers:
    X-Payment-Verified: true
    X-Payment-Amount: 1000
    X-Provisioned-Pod: user-pod-abc123

Response (Failure):
  Status: 402
  Body: {"error": "Payment Required", "amount": 1000}
```

## 🎯 Why External Auth > Native Modules

| Aspect | External Auth ✅ | Native Module ❌ |
|--------|------------------|-------------------|
| **Deployment** | Standard K8s | Rebuild ingress |
| **Updates** | Rolling updates | Restart ingress |
| **Scaling** | Auto-scaling | Manual restart |
| **Cloud Support** | Any provider | Custom builds |
| **Maintenance** | Easy | Complex |
| **Security** | Isolated | Embedded |

## 🛠️ Advanced Configuration

### **Multiple Payment Tiers:**
```yaml
# Different amounts for different paths
nginx.ingress.kubernetes.io/configuration-snippet: |
  set $payment_amount 1000;
  if ($request_uri ~ ^/premium-plus) {
    set $payment_amount 5000;
  }
  proxy_set_header X-Required-Amount $payment_amount;
```

### **Rate Limiting:**
```yaml
nginx.ingress.kubernetes.io/rate-limit: "10"
nginx.ingress.kubernetes.io/rate-limit-window: "1m"
```

### **Custom Error Pages:**
```yaml
nginx.ingress.kubernetes.io/custom-http-errors: "402,403"
nginx.ingress.kubernetes.io/default-backend: "paygress-error-pages"
```

## 📊 Monitoring

### **Metrics:**
- Payment verification rate
- Pod provisioning success rate
- Response times
- Error rates

### **Logs:**
```bash
# Paygress service logs
kubectl logs -n paygress-system deployment/paygress-auth -f

# Ingress controller logs
kubectl logs -n ingress-nginx deployment/ingress-nginx-controller -f
```

## 🚀 Production Checklist

- [ ] SSL/TLS certificates configured
- [ ] Resource limits set on Paygress pods
- [ ] Monitoring and alerting enabled
- [ ] Backup strategy for Cashu database
- [ ] Rate limiting configured
- [ ] Security policies applied
- [ ] Load testing completed

## 📚 Next Steps

1. **Deploy**: Use `k8s/nginx-ingress-paygress.yaml`
2. **Configure**: Set your domain and payment amounts
3. **Test**: Verify payment flow works
4. **Monitor**: Set up observability
5. **Scale**: Add more Paygress replicas as needed

Your **existing external auth approach is perfect**! 🎉
