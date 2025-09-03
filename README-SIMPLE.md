# Simple NGINX Cashu Auth Plugin

A minimal Rust service that validates Cashu payments for NGINX `auth_request`.

## 🚀 Quick Start

### 1. Build and Run

```bash
# Build
cargo build --release

# Run
export CASHU_DB_PATH=./cashu.db
cargo run
```

The service starts on `http://localhost:8080`

### 2. Test the Auth Endpoint

```bash
# Health check
curl http://localhost:8080/healthz
# Response: OK

# Auth test (will fail without valid Cashu token)
curl "http://localhost:8080/auth?token=sample_token&amount=1000"
# Response: 401 Unauthorized (or 402 Payment Required)
```

### 3. NGINX Configuration

Add this to your NGINX config:

```nginx
server {
    listen 80;
    server_name api.example.com;

    # Protected endpoint
    location /premium {
        auth_request /auth-paygress;
        proxy_pass http://your-backend;
    }

    # Auth endpoint (internal)
    location = /auth-paygress {
        internal;
        proxy_pass http://localhost:8080/auth;
        proxy_pass_request_body off;
        proxy_set_header Content-Length "";
    }
}
```

## 🔌 How It Works

1. **User requests** `http://api.example.com/premium?token=cashuAbc123&amount=5000`
2. **NGINX calls** `http://localhost:8080/auth?token=cashuAbc123&amount=5000`
3. **Paygress verifies** the Cashu token
4. **Returns 200** (allow) or **401/402** (deny)
5. **NGINX forwards** request to backend if allowed

## 🐳 Docker Setup

```bash
# Build and run with Docker Compose
docker-compose up --build

# Test
curl "http://localhost/premium?token=your_cashu_token&amount=1000"
```

## ⚙️ Configuration

Environment variables:

- `BIND_ADDR` - Default: `0.0.0.0:8080`
- `CASHU_DB_PATH` - Default: `./cashu.db`
- `RUST_LOG` - Default: `info`

## 📋 API

### GET /healthz
Health check endpoint
```bash
curl http://localhost:8080/healthz
# Response: OK
```

### GET /auth
NGINX auth_request endpoint

Query parameters:
- `token` - Cashu token (required)
- `amount` - Required amount in msat (optional, default: 1000)

```bash
curl "http://localhost:8080/auth?token=cashuAbc123&amount=5000"
```

Responses:
- `200 OK` - Payment verified, allow request
- `401 Unauthorized` - No token provided  
- `402 Payment Required` - Invalid/insufficient payment
- `500 Internal Server Error` - Verification error

## 🎯 Token Formats

Pass Cashu tokens via:

**Query parameter:**
```
?token=cashuAbc123&amount=5000
```

**Authorization header:**
```
Authorization: Bearer cashuAbc123
```

**Custom header:**
```
X-Cashu-Token: cashuAbc123
```

## 🚨 Status Codes

| Code | Meaning | Action |
|------|---------|--------|
| 200 | ✅ Payment verified | Allow request |
| 401 | ❌ No token | Provide token |
| 402 | 💰 Payment required | Valid payment needed |
| 500 | 💥 Server error | Check logs |

## 📝 Example Usage

```bash
# Start the service
cargo run

# In another terminal, test with NGINX
curl -H "Authorization: Bearer your_cashu_token" \
  "http://localhost/premium"
```

## 🔍 Troubleshooting

**Check logs:**
```bash
export RUST_LOG=debug
cargo run
```

**Test auth directly:**
```bash
curl -v "http://localhost:8080/auth?token=test&amount=1000"
```

**Common issues:**
- 401: No token provided
- 402: Invalid Cashu token or insufficient amount
- 500: Cashu database not initialized

That's it! Simple NGINX auth plugin for Cashu payments. 🎉
