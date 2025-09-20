# Paygress Ubuntu Server Setup with Ansible

This Ansible playbook automatically sets up a complete Kubernetes environment with the Paygress service on Ubuntu servers.

## 🎯 What This Playbook Does

- ✅ Installs Docker and Kubernetes (kubeadm, kubelet, kubectl)
- ✅ Sets up a single-node Kubernetes cluster
- ✅ Installs Rust development environment
- ✅ Clones and builds the Paygress service
- ✅ Creates systemd service for automatic startup
- ✅ Configures firewall rules
- ✅ Sets up proper networking for SSH pod access

## 📋 Prerequisites

### Local Machine (Control Node)
- Ansible installed
- SSH access to target Ubuntu server(s)
- SSH private key for server access

### Target Server(s)
- Ubuntu 20.04 or 22.04
- Minimum 2GB RAM, 2 CPU cores
- Public IP address
- SSH access with sudo privileges

## 🚀 Quick Start

### 1. Clone the Repository
```bash
git clone https://github.com/your-username/paygress.git
cd paygress
```

### 2. Update Inventory File
Edit `inventory.ini` with your server details:
```ini
[ubuntu_servers]
production ansible_host=203.0.113.10 ansible_user=ubuntu ansible_ssh_private_key_file=~/.ssh/production.pem
```

### 3. Run the Setup
```bash
chmod +x setup-paygress.sh
./setup-paygress.sh
```

## 📁 File Structure

```
paygress/
├── ansible-setup.yml      # Main Ansible playbook
├── inventory.ini          # Server inventory
├── setup-paygress.sh      # Setup runner script
└── ANSIBLE_SETUP.md       # This file
```

## ⚙️ Configuration

### Inventory File (`inventory.ini`)

```ini
[ubuntu_servers]
# Production server
prod ansible_host=203.0.113.10 ansible_user=ubuntu ansible_ssh_private_key_file=~/.ssh/prod.pem

# Staging server
staging ansible_host=203.0.113.11 ansible_user=ubuntu ansible_ssh_private_key_file=~/.ssh/staging.pem

[ubuntu_servers:vars]
ansible_python_interpreter=/usr/bin/python3
ansible_ssh_common_args='-o StrictHostKeyChecking=no'
```

### Variables (in `ansible-setup.yml`)

```yaml
vars:
  kubernetes_version: "1.28"          # Kubernetes version
  paygress_user: "{{ ansible_user }}" # User for running paygress
  paygress_dir: "/home/{{ paygress_user }}/paygress"
```

## 🔧 Post-Setup Configuration

### 1. Update Nostr Configuration
SSH into your server and update the Nostr private key:
```bash
ssh ubuntu@YOUR_SERVER_IP
nano ~/paygress/paygress.env
```

Update these fields:
```bash
NOSTR_PRIVATE_KEY=your_actual_nsec_key_here
SSH_HOST=YOUR_SERVER_PUBLIC_IP
```

### 2. Start the Service
```bash
sudo systemctl start paygress
sudo systemctl enable paygress
```

### 3. Check Service Status
```bash
sudo systemctl status paygress
sudo journalctl -u paygress -f
```

## 🌐 Service Access

### Paygress API
- **URL**: `http://YOUR_SERVER_IP:8080`
- **Health Check**: `http://YOUR_SERVER_IP:8080/healthz`

### SSH Pods
SSH pods will be accessible on ports 30000-32000:
```bash
ssh alice@YOUR_SERVER_IP -p 30001
ssh alice@YOUR_SERVER_IP -p 30002
```

## 🔒 Security Configuration

### Firewall Rules
The playbook automatically configures UFW with these rules:
- SSH (port 22): Allow
- Kubernetes API (port 6443): Allow
- Paygress service (port 8080): Allow
- SSH pods (ports 30000-32000): Allow

### Additional Security (Recommended)
```bash
# Limit SSH access to specific IPs
sudo ufw delete allow 22
sudo ufw allow from YOUR_IP_ADDRESS to any port 22

# Use fail2ban for brute force protection
sudo apt install fail2ban
```

## 🐛 Troubleshooting

### Common Issues

1. **Ansible Connection Failed**
   ```bash
   # Test SSH connection
   ssh -i ~/.ssh/your_key.pem ubuntu@YOUR_SERVER_IP
   
   # Verify inventory file
   ansible ubuntu_servers -i inventory.ini -m ping
   ```

2. **Kubernetes Not Starting**
   ```bash
   # Check kubelet status
   sudo systemctl status kubelet
   
   # Reset and reinitialize
   sudo kubeadm reset
   sudo kubeadm init --pod-network-cidr=10.244.0.0/16
   ```

3. **Paygress Service Fails**
   ```bash
   # Check logs
   sudo journalctl -u paygress -f
   
   # Check environment
   cat ~/paygress/paygress.env
   
   # Rebuild if needed
   cd ~/paygress
   cargo build --release
   ```

4. **SSH Pod Access Issues**
   ```bash
   # Check if pods are running
   kubectl get pods -n user-workloads
   
   # Test with port forwarding
   kubectl port-forward pod/pod-name 2222:2222 -n user-workloads
   ssh alice@localhost -p 2222
   ```

### Log Locations
- **Paygress logs**: `sudo journalctl -u paygress -f`
- **Kubernetes logs**: `kubectl logs -n kube-system <pod-name>`
- **System logs**: `/var/log/syslog`

## 🔄 Updates and Maintenance

### Update Paygress
```bash
cd ~/paygress
git pull origin main
cargo build --release
sudo systemctl restart paygress
```

### Update Kubernetes
```bash
# Update packages
sudo apt update
sudo apt upgrade kubelet kubeadm kubectl
sudo systemctl restart kubelet
```

## 📊 Monitoring

### Service Status
```bash
# Paygress service
sudo systemctl status paygress

# Kubernetes cluster
kubectl get nodes
kubectl get pods --all-namespaces

# Resource usage
kubectl top nodes
kubectl top pods -n user-workloads
```

### Health Checks
```bash
# Paygress health
curl http://localhost:8080/healthz

# Kubernetes health
kubectl get componentstatuses
```

## 🎯 Production Checklist

- [ ] Server has public IP and proper DNS
- [ ] SSH key authentication configured
- [ ] Firewall rules properly configured
- [ ] Nostr private key updated
- [ ] Service starting automatically
- [ ] Monitoring and alerting set up
- [ ] Backup strategy in place
- [ ] SSL/TLS certificates configured (if needed)

## 📞 Support

For issues with this Ansible setup:
1. Check the troubleshooting section above
2. Review Ansible logs: `ansible-playbook -vvv`
3. Check server logs: `sudo journalctl -f`
4. Open an issue in the Paygress repository

## 🚀 Advanced Usage

### Multiple Servers
You can deploy to multiple servers by adding them to the inventory:
```bash
ansible-playbook -i inventory.ini ansible-setup.yml --limit production
ansible-playbook -i inventory.ini ansible-setup.yml --limit staging
```

### Custom Configuration
Override variables:
```bash
ansible-playbook -i inventory.ini ansible-setup.yml -e "kubernetes_version=1.29"
```

### Rolling Updates
For zero-downtime updates:
```bash
ansible-playbook -i inventory.ini ansible-setup.yml --serial 1
```
