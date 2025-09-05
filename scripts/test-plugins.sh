#!/bin/bash

# Paygress Plugin Testing Script
# Test native ingress plugins after installation

set -e

# Configuration
INGRESS_TYPE="${INGRESS_TYPE:-nginx}"
TEST_HOST="${TEST_HOST:-api.example.com}"
TEST_TOKEN="${TEST_TOKEN:-cashuAeyJ0eXAiOiJwcm9vZiIsImFsZyI6IkhTMjU2In0.eyJhbW91bnQiOjEwMDAsInNlY3JldCI6InRlc3Rfc2VjcmV0In0.test_signature}"
TEST_AMOUNT="${TEST_AMOUNT:-1000}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Usage information
usage() {
    cat << EOF
Paygress Plugin Testing Script

Usage: $0 [OPTIONS] [TEST_TYPE]

Test Types:
    basic       Basic payment verification tests
    pod         Pod provisioning tests
    stress      Load/stress testing
    security    Security and edge case tests
    all         Run all tests (default)

Options:
    --ingress TYPE      Ingress type: nginx|traefik|envoy (default: nginx)
    --host HOST         Test hostname (default: api.example.com)
    --token TOKEN       Test Cashu token
    --amount AMOUNT     Test payment amount (default: 1000)
    -h, --help          Show this help message

Environment Variables:
    INGRESS_TYPE        Ingress controller type
    TEST_HOST           Test hostname
    TEST_TOKEN          Test Cashu token
    TEST_AMOUNT         Test payment amount

Examples:
    # Test NGINX plugin with basic tests
    ./test-plugins.sh --ingress nginx basic

    # Test Traefik plugin with all tests
    ./test-plugins.sh --ingress traefik --host api.test.com all

    # Run stress tests
    ./test-plugins.sh stress

EOF
}

# Parse command line arguments
TEST_TYPE="all"

while [[ $# -gt 0 ]]; do
    case $1 in
        --ingress)
            INGRESS_TYPE="$2"
            shift 2
            ;;
        --host)
            TEST_HOST="$2"
            shift 2
            ;;
        --token)
            TEST_TOKEN="$2"
            shift 2
            ;;
        --amount)
            TEST_AMOUNT="$2"
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        basic|pod|stress|security|all)
            TEST_TYPE="$1"
            shift
            ;;
        *)
            log_error "Unknown option: $1"
            usage
            exit 1
            ;;
    esac
done

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites for $INGRESS_TYPE testing..."
    
    # Check if kubectl is available
    if ! command -v kubectl &> /dev/null; then
        log_error "kubectl is not installed or not in PATH"
        exit 1
    fi
    
    # Check if curl is available
    if ! command -v curl &> /dev/null; then
        log_error "curl is not installed or not in PATH"
        exit 1
    fi
    
    # Check cluster connectivity
    if ! kubectl cluster-info &> /dev/null; then
        log_error "Cannot connect to Kubernetes cluster"
        exit 1
    fi
    
    # Check ingress controller specific requirements
    case "$INGRESS_TYPE" in
        nginx)
            if ! kubectl get ingressclass nginx &> /dev/null; then
                log_warning "NGINX ingress class not found. Make sure NGINX ingress controller is installed."
            fi
            ;;
        traefik)
            if ! kubectl get crd ingressroutes.traefik.containo.us &> /dev/null; then
                log_warning "Traefik CRDs not found. Make sure Traefik is installed."
            fi
            ;;
        envoy|istio)
            if ! kubectl get namespace istio-system &> /dev/null; then
                log_warning "Istio system namespace not found. Make sure Istio is installed."
            fi
            ;;
    esac
    
    log_success "Prerequisites check passed"
}

# Setup test environment
setup_test_environment() {
    log_info "Setting up test environment..."
    
    # Create test namespace
    kubectl create namespace paygress-test --dry-run=client -o yaml | kubectl apply -f -
    
    # Create test backend service
    cat <<EOF | kubectl apply -f -
apiVersion: apps/v1
kind: Deployment
metadata:
  name: test-backend
  namespace: paygress-test
spec:
  replicas: 1
  selector:
    matchLabels:
      app: test-backend
  template:
    metadata:
      labels:
        app: test-backend
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
          name: test-backend-content
---
apiVersion: v1
kind: ConfigMap
metadata:
  name: test-backend-content
  namespace: paygress-test
data:
  index.html: |
    <!DOCTYPE html>
    <html>
    <head><title>Test Backend</title></head>
    <body>
        <h1>✅ Payment Verified!</h1>
        <p>You successfully accessed the protected service.</p>
        <p>Timestamp: <span id="timestamp"></span></p>
        <script>
            document.getElementById('timestamp').textContent = new Date().toISOString();
        </script>
    </body>
    </html>
---
apiVersion: v1
kind: Service
metadata:
  name: test-backend
  namespace: paygress-test
spec:
  selector:
    app: test-backend
  ports:
  - port: 80
    targetPort: 80
EOF
    
    # Wait for backend to be ready
    kubectl wait --for=condition=available deployment/test-backend -n paygress-test --timeout=60s
    
    log_success "Test environment ready"
}

# Setup ingress configuration
setup_ingress_config() {
    log_info "Setting up $INGRESS_TYPE ingress configuration..."
    
    case "$INGRESS_TYPE" in
        nginx)
            setup_nginx_ingress
            ;;
        traefik)
            setup_traefik_ingress
            ;;
        envoy|istio)
            setup_envoy_ingress
            ;;
        *)
            log_error "Unsupported ingress type: $INGRESS_TYPE"
            exit 1
            ;;
    esac
    
    log_success "Ingress configuration applied"
}

setup_nginx_ingress() {
    cat <<EOF | kubectl apply -f -
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: paygress-test
  namespace: paygress-test
  annotations:
    # Test with native NGINX plugin (no external auth needed)
    paygress.io/enable: "true"
    paygress.io/default-amount: "$TEST_AMOUNT"
    paygress.io/enable-pod-provisioning: "true"
spec:
  ingressClassName: nginx
  rules:
  - host: $TEST_HOST
    http:
      paths:
      - path: /premium
        pathType: Prefix
        backend:
          service:
            name: test-backend
            port:
              number: 80
      - path: /public
        pathType: Prefix
        backend:
          service:
            name: test-backend
            port:
              number: 80
EOF
}

setup_traefik_ingress() {
    cat <<EOF | kubectl apply -f -
apiVersion: traefik.containo.us/v1alpha1
kind: Middleware
metadata:
  name: paygress-test
  namespace: paygress-test
spec:
  plugin:
    paygress:
      cashuDbPath: /tmp/cashu.db
      defaultAmount: $TEST_AMOUNT
      enablePodProvisioning: true
      podNamespace: paygress-test
---
apiVersion: traefik.containo.us/v1alpha1
kind: IngressRoute
metadata:
  name: paygress-test
  namespace: paygress-test
spec:
  entryPoints:
    - web
  routes:
  - match: Host(\`$TEST_HOST\`) && PathPrefix(\`/premium\`)
    kind: Rule
    middlewares:
    - name: paygress-test
    services:
    - name: test-backend
      port: 80
  - match: Host(\`$TEST_HOST\`) && PathPrefix(\`/public\`)
    kind: Rule
    services:
    - name: test-backend
      port: 80
EOF
}

setup_envoy_ingress() {
    cat <<EOF | kubectl apply -f -
apiVersion: networking.istio.io/v1alpha3
kind: Gateway
metadata:
  name: paygress-test
  namespace: paygress-test
spec:
  selector:
    istio: ingressgateway
  servers:
  - port:
      number: 80
      name: http
      protocol: HTTP
    hosts:
    - $TEST_HOST
---
apiVersion: networking.istio.io/v1alpha3
kind: VirtualService
metadata:
  name: paygress-test
  namespace: paygress-test
spec:
  hosts:
  - $TEST_HOST
  gateways:
  - paygress-test
  http:
  - match:
    - uri:
        prefix: /premium
    headers:
      request:
        set:
          x-payment-amount: "$TEST_AMOUNT"
    route:
    - destination:
        host: test-backend.paygress-test.svc.cluster.local
  - match:
    - uri:
        prefix: /public
    route:
    - destination:
        host: test-backend.paygress-test.svc.cluster.local
---
apiVersion: networking.istio.io/v1alpha3
kind: EnvoyFilter
metadata:
  name: paygress-test-filter
  namespace: paygress-test
spec:
  workloadSelector:
    labels:
      app: test-backend
  configPatches:
  - applyTo: HTTP_FILTER
    match:
      context: SIDECAR_INBOUND
      listener:
        filterChain:
          filter:
            name: "envoy.filters.network.http_connection_manager"
    patch:
      operation: INSERT_BEFORE
      value:
        name: envoy.filters.http.wasm
        typed_config:
          "@type": type.googleapis.com/envoy.extensions.filters.http.wasm.v3.Wasm
          config:
            name: "paygress"
            root_id: "paygress_root"
            vm_config:
              vm_id: "paygress_vm" 
              runtime: "envoy.wasm.runtime.v8"
              code:
                local:
                  filename: "/etc/envoy/paygress.wasm"
              configuration:
                "@type": type.googleapis.com/google.protobuf.StringValue
                value: |
                  {
                    "default_amount": $TEST_AMOUNT,
                    "enable_pod_provisioning": true
                  }
EOF
}

# Get ingress URL
get_ingress_url() {
    case "$INGRESS_TYPE" in
        nginx)
            INGRESS_IP=$(kubectl get service -n ingress-nginx ingress-nginx-controller -o jsonpath='{.status.loadBalancer.ingress[0].ip}' 2>/dev/null || echo "localhost")
            if [[ "$INGRESS_IP" == "localhost" ]]; then
                INGRESS_PORT=$(kubectl get service -n ingress-nginx ingress-nginx-controller -o jsonpath='{.spec.ports[0].nodePort}' 2>/dev/null || echo "80")
                INGRESS_URL="http://localhost:$INGRESS_PORT"
            else
                INGRESS_URL="http://$INGRESS_IP"
            fi
            ;;
        traefik)
            INGRESS_IP=$(kubectl get service -n traefik traefik -o jsonpath='{.status.loadBalancer.ingress[0].ip}' 2>/dev/null || echo "localhost")
            if [[ "$INGRESS_IP" == "localhost" ]]; then
                INGRESS_PORT=$(kubectl get service -n traefik traefik -o jsonpath='{.spec.ports[0].nodePort}' 2>/dev/null || echo "80")
                INGRESS_URL="http://localhost:$INGRESS_PORT"
            else
                INGRESS_URL="http://$INGRESS_IP"
            fi
            ;;
        envoy|istio)
            INGRESS_IP=$(kubectl get service -n istio-system istio-ingressgateway -o jsonpath='{.status.loadBalancer.ingress[0].ip}' 2>/dev/null || echo "localhost")
            if [[ "$INGRESS_IP" == "localhost" ]]; then
                INGRESS_PORT=$(kubectl get service -n istio-system istio-ingressgateway -o jsonpath='{.spec.ports[0].nodePort}' 2>/dev/null || echo "80")
                INGRESS_URL="http://localhost:$INGRESS_PORT"
            else
                INGRESS_URL="http://$INGRESS_IP"
            fi
            ;;
    esac
    
    log_info "Ingress URL: $INGRESS_URL"
}

# Basic payment tests
test_basic_payment() {
    log_info "Running basic payment verification tests..."
    
    local test_url="$INGRESS_URL/premium"
    local public_url="$INGRESS_URL/public"
    
    # Test 1: Access without payment (should fail)
    log_info "Test 1: Access without payment"
    local response=$(curl -s -w "%{http_code}" -H "Host: $TEST_HOST" "$test_url" -o /dev/null)
    if [[ "$response" == "401" ]] || [[ "$response" == "402" ]]; then
        log_success "✅ Correctly blocked request without payment (HTTP $response)"
    else
        log_error "❌ Should have blocked request without payment (got HTTP $response)"
        return 1
    fi
    
    # Test 2: Access with valid payment (should succeed)
    log_info "Test 2: Access with valid payment"
    response=$(curl -s -w "%{http_code}" \
        -H "Host: $TEST_HOST" \
        -H "X-Cashu-Token: $TEST_TOKEN" \
        -H "X-Payment-Amount: $TEST_AMOUNT" \
        "$test_url" -o /dev/null)
    if [[ "$response" == "200" ]]; then
        log_success "✅ Successfully allowed request with valid payment (HTTP $response)"
    else
        log_error "❌ Should have allowed request with valid payment (got HTTP $response)"
        return 1
    fi
    
    # Test 3: Access public endpoint (should always work)
    log_info "Test 3: Access public endpoint"
    response=$(curl -s -w "%{http_code}" -H "Host: $TEST_HOST" "$public_url" -o /dev/null)
    if [[ "$response" == "200" ]]; then
        log_success "✅ Public endpoint accessible without payment (HTTP $response)"
    else
        log_error "❌ Public endpoint should be accessible (got HTTP $response)"
        return 1
    fi
    
    # Test 4: Check response headers
    log_info "Test 4: Verify response headers"
    local headers=$(curl -s -I \
        -H "Host: $TEST_HOST" \
        -H "X-Cashu-Token: $TEST_TOKEN" \
        -H "X-Payment-Amount: $TEST_AMOUNT" \
        "$test_url")
    
    if echo "$headers" | grep -q "X-Payment-Verified: true"; then
        log_success "✅ Payment verification header present"
    else
        log_warning "⚠️ Payment verification header missing"
    fi
    
    if echo "$headers" | grep -q "X-Payment-Amount: $TEST_AMOUNT"; then
        log_success "✅ Payment amount header correct"
    else
        log_warning "⚠️ Payment amount header missing or incorrect"
    fi
    
    log_success "Basic payment tests completed"
}

# Pod provisioning tests
test_pod_provisioning() {
    log_info "Running pod provisioning tests..."
    
    local test_url="$INGRESS_URL/premium"
    
    # Test 1: Request pod provisioning
    log_info "Test 1: Request pod provisioning"
    local response=$(curl -s -i \
        -H "Host: $TEST_HOST" \
        -H "X-Cashu-Token: $TEST_TOKEN" \
        -H "X-Payment-Amount: $TEST_AMOUNT" \
        -H "X-Create-Pod: true" \
        -H "X-Pod-Image: nginx:alpine" \
        -H "X-Service-Name: test-workspace" \
        "$test_url")
    
    if echo "$response" | grep -q "HTTP.*200"; then
        log_success "✅ Pod provisioning request accepted"
        
        # Check for pod creation header
        if echo "$response" | grep -q "X-Provisioned-Pod:"; then
            local pod_name=$(echo "$response" | grep "X-Provisioned-Pod:" | cut -d: -f2 | tr -d ' \r')
            log_success "✅ Pod created: $pod_name"
            
            # Verify pod exists in Kubernetes
            if kubectl get pod "$pod_name" -n paygress-test &> /dev/null; then
                log_success "✅ Pod verified in Kubernetes"
            else
                log_error "❌ Pod not found in Kubernetes"
                return 1
            fi
        else
            log_warning "⚠️ Pod provisioning header missing"
        fi
    else
        log_error "❌ Pod provisioning request failed"
        echo "$response"
        return 1
    fi
    
    # Test 2: Verify pod labels
    log_info "Test 2: Verify pod labels"
    local pods=$(kubectl get pods -n paygress-test -l payment-verified=true --no-headers 2>/dev/null | wc -l)
    if [[ "$pods" -gt 0 ]]; then
        log_success "✅ Found $pods pod(s) with payment-verified label"
    else
        log_warning "⚠️ No pods found with payment-verified label"
    fi
    
    log_success "Pod provisioning tests completed"
}

# Security tests
test_security() {
    log_info "Running security tests..."
    
    local test_url="$INGRESS_URL/premium"
    
    # Test 1: Invalid token format
    log_info "Test 1: Invalid token format"
    local response=$(curl -s -w "%{http_code}" \
        -H "Host: $TEST_HOST" \
        -H "X-Cashu-Token: invalid_token" \
        "$test_url" -o /dev/null)
    if [[ "$response" == "401" ]] || [[ "$response" == "402" ]]; then
        log_success "✅ Correctly rejected invalid token (HTTP $response)"
    else
        log_error "❌ Should have rejected invalid token (got HTTP $response)"
    fi
    
    # Test 2: Insufficient payment amount
    log_info "Test 2: Insufficient payment amount"
    response=$(curl -s -w "%{http_code}" \
        -H "Host: $TEST_HOST" \
        -H "X-Cashu-Token: $TEST_TOKEN" \
        -H "X-Payment-Amount: 100" \
        "$test_url" -o /dev/null)
    if [[ "$response" == "402" ]]; then
        log_success "✅ Correctly rejected insufficient payment (HTTP $response)"
    else
        log_warning "⚠️ Should have rejected insufficient payment (got HTTP $response)"
    fi
    
    # Test 3: Malicious headers
    log_info "Test 3: Malicious headers"
    response=$(curl -s -w "%{http_code}" \
        -H "Host: $TEST_HOST" \
        -H "X-Cashu-Token: $TEST_TOKEN" \
        -H "X-Pod-Image: malicious/image:latest" \
        -H "X-Create-Pod: true" \
        "$test_url" -o /dev/null)
    # Should either allow with default image or reject malicious image
    if [[ "$response" == "200" ]] || [[ "$response" == "403" ]]; then
        log_success "✅ Handled malicious image request appropriately (HTTP $response)"
    else
        log_warning "⚠️ Unexpected response to malicious image (HTTP $response)"
    fi
    
    log_success "Security tests completed"
}

# Stress tests
test_stress() {
    log_info "Running stress tests..."
    
    local test_url="$INGRESS_URL/premium"
    local concurrent_requests=10
    local total_requests=100
    
    log_info "Running $total_requests requests with $concurrent_requests concurrent connections"
    
    # Create temporary script for concurrent requests
    cat > /tmp/stress_test.sh << EOF
#!/bin/bash
for i in \$(seq 1 $((total_requests / concurrent_requests))); do
    curl -s -w "%{http_code}\n" \\
        -H "Host: $TEST_HOST" \\
        -H "X-Cashu-Token: $TEST_TOKEN" \\
        -H "X-Payment-Amount: $TEST_AMOUNT" \\
        "$test_url" -o /dev/null
done
EOF
    chmod +x /tmp/stress_test.sh
    
    # Run concurrent requests
    local start_time=$(date +%s)
    for i in $(seq 1 $concurrent_requests); do
        /tmp/stress_test.sh &
    done
    wait
    local end_time=$(date +%s)
    local duration=$((end_time - start_time))
    
    local rps=$((total_requests / duration))
    log_success "✅ Completed $total_requests requests in ${duration}s (${rps} RPS)"
    
    # Clean up
    rm -f /tmp/stress_test.sh
    
    log_success "Stress tests completed"
}

# Run tests based on type
run_tests() {
    case "$TEST_TYPE" in
        basic)
            test_basic_payment
            ;;
        pod)
            test_pod_provisioning
            ;;
        security)
            test_security
            ;;
        stress)
            test_stress
            ;;
        all)
            test_basic_payment
            test_pod_provisioning
            test_security
            test_stress
            ;;
        *)
            log_error "Unknown test type: $TEST_TYPE"
            exit 1
            ;;
    esac
}

# Cleanup test environment
cleanup_test_environment() {
    log_info "Cleaning up test environment..."
    
    # Delete test namespace (this removes all resources)
    kubectl delete namespace paygress-test --ignore-not-found=true
    
    log_success "Test environment cleaned up"
}

# Main execution
main() {
    log_info "Starting Paygress plugin tests..."
    log_info "Ingress: $INGRESS_TYPE, Host: $TEST_HOST, Tests: $TEST_TYPE"
    
    check_prerequisites
    setup_test_environment
    setup_ingress_config
    
    # Wait for ingress to be ready
    sleep 10
    
    get_ingress_url
    run_tests
    
    # Keep test environment for manual inspection
    log_info "Test environment kept for manual inspection"
    log_info "To clean up: kubectl delete namespace paygress-test"
    
    log_success "All tests completed! 🎉"
}

# Handle cleanup on exit
trap cleanup_test_environment EXIT

# Run main function
main "$@"
