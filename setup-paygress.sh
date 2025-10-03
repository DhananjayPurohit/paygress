#!/bin/bash

# Paygress Ansible Setup Script
# This script runs the Ansible playbook to deploy Paygress

set -e

echo "🚀 Running Paygress Ansible Setup"
echo "=================================="

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
    exit 1
fi

# Check if playbook exists
if [ ! -f "ansible-setup.yml" ]; then
    echo "❌ Error: ansible-setup.yml file not found!"
    exit 1
fi

echo "✅ Files found, starting deployment..."

# Run the Ansible playbook
ansible-playbook -i inventory.ini ansible-setup.yml -v

echo "🎉 Ansible setup completed!"
