#!/bin/bash

# Paygress Ubuntu Server Setup Script
# This script uses Ansible to set up a complete Kubernetes environment with Paygress

set -e

echo "🚀 Paygress Ubuntu Server Setup"
echo "================================"

# Check if Ansible is installed
if ! command -v ansible-playbook &> /dev/null; then
    echo "📦 Installing Ansible..."
    sudo apt update
    sudo apt install -y ansible
fi

# Check if inventory file exists
if [ ! -f "inventory.ini" ]; then
    echo "❌ Error: inventory.ini file not found!"
    echo "Please create inventory.ini with your server details."
    echo "Example:"
    echo "[ubuntu_servers]"
    echo "server1 ansible_host=YOUR_SERVER_IP ansible_user=ubuntu ansible_ssh_private_key_file=~/.ssh/your_key.pem"
    exit 1
fi

# Validate SSH key
echo "🔑 Testing SSH connection..."
ansible ubuntu_servers -i inventory.ini -m ping

if [ $? -ne 0 ]; then
    echo "❌ SSH connection failed. Please check your inventory.ini file."
    exit 1
fi

echo "✅ SSH connection successful!"

# Run the playbook
echo "🏗️  Running Ansible playbook..."
ansible-playbook -i inventory.ini ansible-setup.yml -v

echo ""
echo "🎉 Setup completed successfully!"
echo ""
echo "📋 Next steps:"
echo "1. SSH into your server"
echo "2. Update the Nostr private key in ~/paygress/paygress.env"
echo "3. Start the service: sudo systemctl start paygress"
echo "4. Check status: sudo systemctl status paygress"
echo ""
echo "🌐 Your Paygress service will be available at:"
echo "   http://YOUR_SERVER_IP:8080"
echo ""
echo "📖 For more details, check ~/setup-complete.sh on your server"
