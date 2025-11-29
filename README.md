# Paygress

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

## 📝 Usage

```bash
curl -X POST http://your-server/pods/spawn \
  -H "Content-Type: application/json" \
  -H "X-Cashu: Cashu cashuAeyJ0b2tlbiI6..." \
  -d '{
    "pod_spec_id": "basic",
    "pod_image": "linuxserver/openssh-server:latest",
    "ssh_username": "user",
    "ssh_password": "password"
  }'
```

## 🛠️ Commands

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
