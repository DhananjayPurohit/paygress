# 🚀 Paygress Production Deployment Guide

## 📋 **Production Files Overview**

### **Core Files (Required)**
- `Cargo.toml` - Rust project configuration
- `src/` - Source code
- `start-paygress.sh` - Production start script
- `ansible-setup.yml` - Automated deployment
- `Dockerfile` - Container deployment
- `k8s/sidecar-service.yaml` - Kubernetes manifests

### **Configuration Files**
- `.env` - Environment variables (create from template)
- `pod-specs.json` - Pod specifications
- `inventory.ini.template` - Ansible inventory template

### **Documentation**
- `README.md` - Main documentation
- `SETUP_INSTRUCTIONS.md` - Setup guide
- `ANSIBLE_SETUP.md` - Ansible deployment guide

## 🏗️ **Production Deployment Methods**

### **Method 1: Direct Binary Deployment**
```bash
# Build for production
cargo build --release

# Configure environment
cp inventory.ini.template inventory.ini
# Edit inventory.ini with your server details

# Deploy with Ansible
ansible-playbook -i inventory.ini ansible-setup.yml
```

### **Method 2: Docker Deployment**
```bash
# Build Docker image
docker build -t paygress:latest .

# Run container
docker run -d \
  --name paygress \
  --env-file .env \
  -p 8080:8080 \
  paygress:latest
```

### **Method 3: Kubernetes Deployment**
```bash
# Apply Kubernetes manifests
kubectl apply -f k8s/sidecar-service.yaml

# Configure environment
kubectl create secret generic paygress-config --from-env-file=.env
```

## ⚙️ **Production Configuration**

### **Environment Variables (.env)**
```bash
# Service Configuration
ENABLE_NOSTR=true
ENABLE_MCP=true
ENABLE_HTTP=true
HTTP_BIND_ADDR=0.0.0.0:8080

# Nostr Configuration
NOSTR_RELAYS=wss://relay.damus.io,wss://nos.lol
NOSTR_PRIVATE_KEY=nsec1...

# Cashu Configuration
WHITELISTED_MINTS=https://mint.cashu.space,https://mint.f7z.io
CASHU_DB_PATH=./data/cashu.db

# Kubernetes Configuration
POD_NAMESPACE=user-workloads
SSH_HOST=your-server-ip
POD_SPECS_FILE=./pod-specs.json

# Logging
RUST_LOG=info
```

### **Pod Specifications (pod-specs.json)**
```json
[
  {
    "id": "basic",
    "name": "Basic",
    "description": "Basic VPS - 1 CPU core, 1GB RAM",
    "cpu_millicores": 1000,
    "memory_mb": 1024,
    "rate_msats_per_sec": 100
  }
]
```

## 🔧 **Production Commands**

### **Start Service**
```bash
./start-paygress.sh
```

### **Build for Production**
```bash
cargo build --release
```

### **Run with Systemd (Linux)**
```bash
# Install systemd service
sudo cp paygress.service /etc/systemd/system/
sudo systemctl enable paygress
sudo systemctl start paygress
```

## 📊 **Monitoring & Health Checks**

### **HTTP Health Check**
```bash
curl http://localhost:8080/health
```

### **Service Status**
```bash
# Check if service is running
ps aux | grep paygress

# Check logs
journalctl -u paygress -f
```

## 🛡️ **Security Considerations**

1. **Environment Variables**: Keep `.env` secure, never commit to git
2. **Private Keys**: Store Nostr private keys securely
3. **Database**: Backup Cashu database regularly
4. **Network**: Use firewall rules for SSH ports
5. **Updates**: Keep dependencies updated

## 🔄 **Updates & Maintenance**

### **Update Service**
```bash
git pull
cargo build --release
sudo systemctl restart paygress
```

### **Database Backup**
```bash
cp ./data/cashu.db ./backups/cashu-$(date +%Y%m%d).db
```

## 📝 **Production Checklist**

- [ ] Build with `cargo build --release`
- [ ] Configure all environment variables
- [ ] Set up proper logging
- [ ] Configure firewall rules
- [ ] Set up monitoring
- [ ] Configure backups
- [ ] Test all interfaces (HTTP, Nostr, MCP)
- [ ] Set up systemd service
- [ ] Configure SSL/TLS if needed
- [ ] Set up log rotation
