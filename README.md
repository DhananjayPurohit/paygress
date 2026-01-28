# Paygress

### Demo video
https://github.com/user-attachments/assets/627d2bb1-1a9b-4e66-bc42-7c91a1804fe1

**Cashu Payment Gateway for Kubernetes Pod Provisioning with ngx_l402**

## 🚀 Deploy

```bash
# 1. Configure
cp inventory.ini.template inventory.ini
nano inventory.ini

# 2. Deploy
chmod +x setup-paygress.sh
./setup-paygress.sh deploy
```

### Docker Deployment

```bash
# 1. Configure
cp .env.template .env
nano .env

# 2. Deploy
docker-compose up -d
```

## 💰 How It Works

```
Client → nginx + ngx_l402 → Paygress → Kubernetes Pod
         (validates payment)  (decodes → calculates duration)
```

**Payment determines duration:** `duration = payment_msats ÷ tier_rate`

## 📊 Pricing

| Tier | Rate | CPU | RAM | 60k msats |
|------|------|-----|-----|-----------|
| basic | 100 msats/sec | 1 core | 1GB | 10 min |
| standard | 200 msats/sec | 2 cores | 2GB | 5 min |
| premium | 400 msats/sec | 4 cores | 4GB | 2.5 min |

## 📝 API Usage

```bash
curl -X POST http://your-server:<http-port>/pods/spawn \
  -H "Content-Type: application/json" \
  -H "X-Cashu Cashu cashuAeyJ0b2tlbiI6..." \
  -d '{
    "pod_spec_id": "basic",
    "pod_image": "linuxserver/openssh-server:latest",
    "ssh_username": "user",
    "ssh_password": "password"
  }'
```

## 🖥️ CLI Tool

### Build

```bash
cargo build --bin paygress-cli
```

### Commands

```bash
# Spawn a pod with Cashu payment
paygress-cli spawn \
  -s http://your-server:<http-port> \
  --tier basic \
  --token "cashuBo2F..." \
  --ssh-user myuser \
  --ssh-pass mypassword

# Check pod status
paygress-cli status \
  -s http://your-server:<http-port> \
  --pod-id <POD_NPUB>

# Top up a pod
paygress-cli topup \
  -s http://your-server:<http-port> \
  --pod-id <POD_NPUB> \
  --token "cashuBo2F..."

# List available offers/tiers
./target/debug/paygress-cli offers -s http://your-server:<http-port>
```

### CLI Options

| Option | Description | Default |
|--------|-------------|---------|
| `-s, --server` | Paygress server URL | `http://localhost:8080` |
| `-t, --tier` | Pod tier (basic, standard, premium) | Required |
| `-k, --token` | Cashu token for payment | Required |
| `-i, --image` | Container image | `linuxserver/openssh-server:latest` |
| `-u, --ssh-user` | SSH username | `user` |
| `-p, --ssh-pass` | SSH password | `password` |

## 🛠️ Server Commands

```bash
./setup-paygress.sh deploy    # Deploy
./setup-paygress.sh status    # Check status
./setup-paygress.sh logs      # View logs
./setup-paygress.sh test      # Test API
./setup-paygress.sh restart   # Restart
./setup-paygress.sh fix-k8s   # Fix Kubernetes
```

## 🔧 Fix Container Issues

If pods are stuck in `ContainerCreating`:

```bash
ssh c03rad0r@192.168.8.229

# Restart containerd and kubelet
sudo systemctl restart containerd
sudo systemctl restart kubelet

# Wait
sleep 30

# Check status
kubectl get pods -n user-workloads

# If still stuck, delete failed pods
kubectl delete pod --all -n user-workloads --force --grace-period=0

# Check nodes
kubectl get nodes
```

## ⚙️ Configuration

**inventory.ini:** Server details, pricing, mints  
**pod-specs.json:** Pricing tiers  

## 🏗️ Architecture

- **ngx_l402** - Payment enforcement at nginx
- **Paygress** - Token decoding & duration calculation
- **Kubernetes** - Pod provisioning & management

Payment verification: ngx_l402 only  
Duration calculation: Paygress (payment ÷ rate)
