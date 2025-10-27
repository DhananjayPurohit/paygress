#!/bin/bash
# Fix SSH to listen on port 12022

echo "🔧 Fixing SSH configuration..."

# Add both ports
sudo bash -c 'cat >> /etc/ssh/sshd_config << EOF

# Paygress SSH Configuration
Port 22
Port 12022
EOF'

# Verify
echo "✅ SSH config updated:"
sudo grep "^Port" /etc/ssh/sshd_config

# Restart SSH
echo "🔄 Restarting SSH..."
sudo systemctl restart ssh

# Verify listening
echo "✅ SSH now listening on:"
sudo ss -tlnp | grep ssh | grep -E ':(22|12022)'

echo "✅ Done! SSH should now be accessible on port 12022"

