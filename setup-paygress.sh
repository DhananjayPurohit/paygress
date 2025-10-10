#!/bin/bash
# Paygress Deployment Script
# Deploys all three interfaces: HTTP, Nostr, and MCP (Context VM)

set -e

echo "🚀 Paygress Deployment"
echo "======================"
echo ""

# Check if Ansible is installed
if ! command -v ansible-playbook &> /dev/null; then
    echo "📦 Installing Ansible..."
    sudo apt update
    sudo apt install -y ansible
fi

# Check if inventory file exists
if [ ! -f "inventory.ini" ]; then
    echo "❌ Error: inventory.ini not found"
    echo ""
    echo "Create it from template:"
    echo "  cp inventory.ini.template inventory.ini"
    echo "  nano inventory.ini"
    echo ""
    exit 1
fi

# Check if playbook exists
if [ ! -f "ansible-setup.yml" ]; then
    echo "❌ Error: ansible-setup.yml not found"
    exit 1
fi

echo "✅ Running deployment..."
echo ""

# Run the Ansible playbook
ansible-playbook -i inventory.ini ansible-setup.yml

echo ""
echo "🎉 Deployment complete!"
echo ""
echo "Services status:"
echo "  sudo systemctl status paygress contextvm"
echo ""
echo "View logs:"
echo "  sudo journalctl -u paygress -f"
echo "  sudo journalctl -u contextvm -f"
echo ""
