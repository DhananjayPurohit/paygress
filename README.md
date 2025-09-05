# Paygress - Embedded Nostr NGINX Plugin

🔧 **Single NGINX Plugin: Nostr Events → Pod Provisioning + HTTP Payment Verification**

## Architecture

**One Component:**
- **NGINX Plugin** with embedded Nostr listener that:
  1. Listens for Nostr events (kind 1000) in background
  2. Verifies Cashu payments and provisions pods automatically  
  3. Verifies payments for HTTP access to provisioned pods

## Quick Start

```bash
# 1. Build and run
docker-compose up -d

# 2. Deploy to Kubernetes  
kubectl apply -f ingress.yaml

# 3. Send Nostr event (kind 1000) with Cashu token → Pod auto-provisioned
# 4. Access pod via HTTP with payment verification
curl -H "Authorization: Bearer 1000sat-token" -H "X-Pod-ID: pod-abc123" http://api.example.com/premium
```

## Files

- `src/nginx_plugin.rs` - Complete plugin with embedded Nostr listener
- `src/nostr.rs` - Nostr client library
- `src/lib.rs` - Core library
- `Dockerfile.nginx-ingress` - NGINX Ingress + plugin
- `docker-compose.yaml` - Single container setup
- `ingress.yaml` - Kubernetes deployment

## How it works

1. **Plugin loads** → Starts background Nostr listener automatically
2. **Nostr event received** → Verify Cashu → Provision pod → Store in memory
3. **HTTP request** → Verify payment → Check pod exists → Allow/deny access

**Everything happens inside the NGINX plugin - no separate services!**
