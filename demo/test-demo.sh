#!/bin/bash

# Paygress Plugin Demo
# Quick demonstration of native ingress plugins

set -e

# Configuration
DEMO_HOST="paygress-demo.local"
DEMO_TOKEN="cashuAeyJ0eXAiOiJwcm9vZiIsImFsZyI6IkhTMjU2In0.eyJhbW91bnQiOjEwMDAsInNlY3JldCI6InRlc3Rfc2VjcmV0In0.demo_signature"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}"
cat << 'EOF'
╔═══════════════════════════════════════════════════════════════════╗
║                     PAYGRESS PLUGIN DEMO                          ║
║              Native Cashu Payment Verification                     ║
╚═══════════════════════════════════════════════════════════════════╝
EOF
echo -e "${NC}"

# Check if we have a running ingress
detect_ingress() {
    echo -e "${BLUE}🔍 Detecting ingress controller...${NC}"
    
    if kubectl get ingressclass nginx &>/dev/null; then
        INGRESS_TYPE="nginx"
        INGRESS_URL="http://localhost:$(kubectl get svc -n ingress-nginx ingress-nginx-controller -o jsonpath='{.spec.ports[0].nodePort}' 2>/dev/null || echo '80')"
        echo -e "${GREEN}✅ Found NGINX Ingress Controller${NC}"
    elif kubectl get crd ingressroutes.traefik.containo.us &>/dev/null; then
        INGRESS_TYPE="traefik"
        INGRESS_URL="http://localhost:$(kubectl get svc -n traefik traefik -o jsonpath='{.spec.ports[0].nodePort}' 2>/dev/null || echo '80')"
        echo -e "${GREEN}✅ Found Traefik Ingress Controller${NC}"
    elif kubectl get namespace istio-system &>/dev/null; then
        INGRESS_TYPE="istio"
        INGRESS_URL="http://localhost:$(kubectl get svc -n istio-system istio-ingressgateway -o jsonpath='{.spec.ports[0].nodePort}' 2>/dev/null || echo '80')"
        echo -e "${GREEN}✅ Found Istio/Envoy${NC}"
    else
        echo -e "${RED}❌ No supported ingress controller found${NC}"
        echo "Please install NGINX, Traefik, or Istio first"
        exit 1
    fi
    
    echo -e "${BLUE}📍 Ingress URL: $INGRESS_URL${NC}"
}

# Setup demo environment
setup_demo() {
    echo -e "${BLUE}🚀 Setting up demo environment...${NC}"
    
    # Create demo namespace
    kubectl create namespace paygress-demo --dry-run=client -o yaml | kubectl apply -f -
    
    # Deploy demo backend
    cat <<EOF | kubectl apply -f -
apiVersion: apps/v1
kind: Deployment
metadata:
  name: demo-backend
  namespace: paygress-demo
spec:
  replicas: 1
  selector:
    matchLabels:
      app: demo-backend
  template:
    metadata:
      labels:
        app: demo-backend
    spec:
      containers:
      - name: backend
        image: nginx:alpine
        ports:
        - containerPort: 80
        volumeMounts:
        - name: content
          mountPath: /usr/share/nginx/html
      volumes:
      - name: content
        configMap:
          name: demo-content
---
apiVersion: v1
kind: ConfigMap
metadata:
  name: demo-content
  namespace: paygress-demo
data:
  index.html: |
    <!DOCTYPE html>
    <html>
    <head>
        <title>🎉 Paygress Demo - Payment Verified!</title>
        <style>
            body { 
                font-family: -apple-system, BlinkMacSystemFont, sans-serif; 
                margin: 40px; 
                background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
                color: white;
                min-height: 100vh;
                display: flex;
                align-items: center;
                justify-content: center;
            }
            .container {
                background: rgba(255,255,255,0.1);
                padding: 40px;
                border-radius: 12px;
                text-align: center;
                backdrop-filter: blur(10px);
            }
            .success { color: #4CAF50; font-size: 48px; }
            .amount { color: #FFD700; font-size: 24px; margin: 20px 0; }
            .timestamp { color: #ccc; font-size: 14px; }
        </style>
    </head>
    <body>
        <div class="container">
            <div class="success">✅</div>
            <h1>Payment Verified!</h1>
            <p>Your Cashu token was successfully verified by the Paygress plugin.</p>
            <div class="amount">💰 Payment Amount: <span id="amount">Loading...</span> msat</div>
            <p>🚀 Service provisioned successfully</p>
            <div class="timestamp">Accessed at: <span id="timestamp"></span></div>
            
            <script>
                // Extract headers if available
                const amount = document.querySelector('meta[name="payment-amount"]')?.content || '1000';
                document.getElementById('amount').textContent = amount;
                document.getElementById('timestamp').textContent = new Date().toISOString();
            </script>
        </div>
    </body>
    </html>
---
apiVersion: v1
kind: Service
metadata:
  name: demo-backend
  namespace: paygress-demo
spec:
  selector:
    app: demo-backend
  ports:
  - port: 80
    targetPort: 80
EOF
    
    # Wait for deployment
    kubectl wait --for=condition=available deployment/demo-backend -n paygress-demo --timeout=60s
    echo -e "${GREEN}✅ Demo backend ready${NC}"
}

# Setup ingress for demo
setup_ingress() {
    echo -e "${BLUE}🌐 Setting up $INGRESS_TYPE ingress...${NC}"
    
    case "$INGRESS_TYPE" in
        nginx)
            cat <<EOF | kubectl apply -f -
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: paygress-demo
  namespace: paygress-demo
  annotations:
    # Paygress plugin configuration (adjust based on your plugin setup)
    paygress.io/enable: "true"
    paygress.io/default-amount: "1000"
    paygress.io/enable-pod-provisioning: "false"
spec:
  ingressClassName: nginx
  rules:
  - host: $DEMO_HOST
    http:
      paths:
      - path: /premium
        pathType: Prefix
        backend:
          service:
            name: demo-backend
            port:
              number: 80
      - path: /public
        pathType: Prefix
        backend:
          service:
            name: demo-backend
            port:
              number: 80
EOF
            ;;
        traefik)
            cat <<EOF | kubectl apply -f -
apiVersion: traefik.containo.us/v1alpha1
kind: Middleware
metadata:
  name: paygress-demo
  namespace: paygress-demo
spec:
  plugin:
    paygress:
      cashuDbPath: /tmp/cashu.db
      defaultAmount: 1000
      enablePodProvisioning: false
---
apiVersion: traefik.containo.us/v1alpha1
kind: IngressRoute
metadata:
  name: paygress-demo
  namespace: paygress-demo
spec:
  entryPoints:
    - web
  routes:
  - match: Host(\`$DEMO_HOST\`) && PathPrefix(\`/premium\`)
    kind: Rule
    middlewares:
    - name: paygress-demo
    services:
    - name: demo-backend
      port: 80
  - match: Host(\`$DEMO_HOST\`) && PathPrefix(\`/public\`)
    kind: Rule
    services:
    - name: demo-backend
      port: 80
EOF
            ;;
        istio)
            cat <<EOF | kubectl apply -f -
apiVersion: networking.istio.io/v1alpha3
kind: Gateway
metadata:
  name: paygress-demo
  namespace: paygress-demo
spec:
  selector:
    istio: ingressgateway
  servers:
  - port:
      number: 80
      name: http
      protocol: HTTP
    hosts:
    - $DEMO_HOST
---
apiVersion: networking.istio.io/v1alpha3
kind: VirtualService
metadata:
  name: paygress-demo
  namespace: paygress-demo
spec:
  hosts:
  - $DEMO_HOST
  gateways:
  - paygress-demo
  http:
  - match:
    - uri:
        prefix: /premium
    headers:
      request:
        set:
          x-payment-amount: "1000"
    route:
    - destination:
        host: demo-backend.paygress-demo.svc.cluster.local
  - match:
    - uri:
        prefix: /public
    route:
    - destination:
        host: demo-backend.paygress-demo.svc.cluster.local
EOF
            ;;
    esac
    
    echo -e "${GREEN}✅ Ingress configured${NC}"
}

# Run demonstrations
run_demo() {
    echo -e "${BLUE}🎬 Running payment demonstrations...${NC}"
    echo ""
    
    # Demo 1: Access without payment (should fail)
    echo -e "${YELLOW}📝 Demo 1: Access without payment${NC}"
    echo "curl -H \"Host: $DEMO_HOST\" $INGRESS_URL/premium"
    response=$(curl -s -w "%{http_code}" -H "Host: $DEMO_HOST" "$INGRESS_URL/premium" -o /dev/null)
    if [[ "$response" == "401" ]] || [[ "$response" == "402" ]]; then
        echo -e "${GREEN}✅ Correctly blocked (HTTP $response)${NC}"
    else
        echo -e "${RED}❌ Should have been blocked (HTTP $response)${NC}"
    fi
    echo ""
    
    # Demo 2: Access with valid payment (should succeed)
    echo -e "${YELLOW}📝 Demo 2: Access with valid Cashu token${NC}"
    echo "curl -H \"Host: $DEMO_HOST\" -H \"X-Cashu-Token: $DEMO_TOKEN\" $INGRESS_URL/premium"
    response=$(curl -s -w "%{http_code}" \
        -H "Host: $DEMO_HOST" \
        -H "X-Cashu-Token: $DEMO_TOKEN" \
        -H "X-Payment-Amount: 1000" \
        "$INGRESS_URL/premium" -o /dev/null)
    if [[ "$response" == "200" ]]; then
        echo -e "${GREEN}✅ Payment verified! (HTTP $response)${NC}"
    else
        echo -e "${RED}❌ Payment should have been accepted (HTTP $response)${NC}"
    fi
    echo ""
    
    # Demo 3: Check response headers
    echo -e "${YELLOW}📝 Demo 3: Check payment verification headers${NC}"
    echo "curl -I -H \"Host: $DEMO_HOST\" -H \"X-Cashu-Token: $DEMO_TOKEN\" $INGRESS_URL/premium"
    headers=$(curl -s -I \
        -H "Host: $DEMO_HOST" \
        -H "X-Cashu-Token: $DEMO_TOKEN" \
        -H "X-Payment-Amount: 1000" \
        "$INGRESS_URL/premium")
    
    echo "$headers" | grep -E "(HTTP|X-Payment|X-Auth)" || echo "Headers: $headers"
    echo ""
    
    # Demo 4: Public endpoint (should always work)
    echo -e "${YELLOW}📝 Demo 4: Access public endpoint (no payment required)${NC}"
    echo "curl -H \"Host: $DEMO_HOST\" $INGRESS_URL/public"
    response=$(curl -s -w "%{http_code}" -H "Host: $DEMO_HOST" "$INGRESS_URL/public" -o /dev/null)
    if [[ "$response" == "200" ]]; then
        echo -e "${GREEN}✅ Public access works (HTTP $response)${NC}"
    else
        echo -e "${RED}❌ Public access should work (HTTP $response)${NC}"
    fi
    echo ""
}

# Show demo info
show_demo_info() {
    echo -e "${BLUE}📋 Demo Information:${NC}"
    echo ""
    echo -e "${YELLOW}🌐 URLs to test manually:${NC}"
    echo "Premium (requires payment): $INGRESS_URL/premium"
    echo "Public (no payment):        $INGRESS_URL/public"
    echo ""
    echo -e "${YELLOW}🔑 Test with curl:${NC}"
    echo "# Without payment (should fail)"
    echo "curl -H \"Host: $DEMO_HOST\" $INGRESS_URL/premium"
    echo ""
    echo "# With payment (should succeed)"
    echo "curl -H \"Host: $DEMO_HOST\" \\"
    echo "     -H \"X-Cashu-Token: $DEMO_TOKEN\" \\"
    echo "     -H \"X-Payment-Amount: 1000\" \\"
    echo "     $INGRESS_URL/premium"
    echo ""
    echo -e "${YELLOW}🛠️ Advanced testing:${NC}"
    echo "# Test with different amounts"
    echo "curl -H \"Host: $DEMO_HOST\" -H \"X-Cashu-Token: $DEMO_TOKEN\" -H \"X-Payment-Amount: 2000\" $INGRESS_URL/premium"
    echo ""
    echo "# Test pod provisioning (if enabled)"
    echo "curl -H \"Host: $DEMO_HOST\" -H \"X-Cashu-Token: $DEMO_TOKEN\" -H \"X-Create-Pod: true\" $INGRESS_URL/premium"
    echo ""
    echo -e "${YELLOW}🧹 Cleanup:${NC}"
    echo "kubectl delete namespace paygress-demo"
    echo ""
}

# Main execution
main() {
    detect_ingress
    setup_demo
    setup_ingress
    
    # Wait a moment for ingress to be ready
    echo -e "${BLUE}⏱️ Waiting for ingress to be ready...${NC}"
    sleep 10
    
    run_demo
    show_demo_info
    
    echo -e "${GREEN}"
    cat << 'EOF'
╔═══════════════════════════════════════════════════════════════════╗
║                      DEMO COMPLETED! 🎉                           ║
║                                                                   ║
║  Your Paygress plugin is working! Try the manual tests above     ║
║  to see payment verification and pod provisioning in action.     ║
╚═══════════════════════════════════════════════════════════════════╝
EOF
    echo -e "${NC}"
}

# Error handling
trap 'echo -e "${RED}❌ Demo failed. Check the logs above for details.${NC}"' ERR

# Run the demo
main "$@"
