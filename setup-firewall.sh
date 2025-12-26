#!/bin/bash
set -e

# Configuration
SSH_PORT_START=11000  # Default, should match .env
SSH_PORT_END=11999    # Default
API_PORT=6443
HTTP_PORT_PAYGRESS=8080
HTTP_PORT_NGINX=80
WIREGUARD_PORT=51820
POD_CIDR="10.244.0.0/16"
SERVICE_CIDR="10.96.0.0/12" # CoreDNS/Service network

# 1. Check if UFW is installed
if ! command -v ufw &> /dev/null; then
    echo "UFW is not installed. Installing..."
    apt-get update && apt-get install -y ufw
fi

echo "Configuring Firewall Rules..."

# 2. Allow SSH (Standard & Range)
ufw allow ssh  # Allow standard 22
ufw allow ${SSH_PORT_START}:${SSH_PORT_END}/tcp comment 'Paygress Pod SSH Access'

# 3. Allow Kubernetes API
ufw allow ${API_PORT}/tcp comment 'Kubernetes API'

# 4. Allow WireGuard
ufw allow ${WIREGUARD_PORT}/udp comment 'WireGuard VPN'

# 5. Allow HTTP/Nginx
ufw allow ${HTTP_PORT_PAYGRESS}/tcp comment 'Paygress Backend (Internal/Debug)'
ufw allow ${HTTP_PORT_NGINX}/tcp comment 'Nginx L402 Paywall'

# 6. Default Policies
ufw default deny incoming
ufw default allow outgoing

# 7. Enable Forwarding for Kubernetes
# UFW by default blocks forwarding. We need to enable it for host network + CNI.
sed -i 's/^DEFAULT_FORWARD_POLICY="DROP"/DEFAULT_FORWARD_POLICY="ACCEPT"/' /etc/default/ufw

# 8. Add Kubernetes-specific iptables rules (if not present)
# These are sometimes needed on top of UFW for Flannel/CNI to work properly across interfaces
if ! iptables -C FORWARD -s ${POD_CIDR} -j ACCEPT 2>/dev/null; then
    iptables -I FORWARD 1 -s ${POD_CIDR} -j ACCEPT
fi
if ! iptables -C FORWARD -d ${POD_CIDR} -j ACCEPT 2>/dev/null; then
    iptables -I FORWARD 1 -d ${POD_CIDR} -j ACCEPT
fi

echo "Enabling UFW..."
# Non-interactive enable
ufw --force enable

echo "Firewall setup complete. Status:"
ufw status numbered
