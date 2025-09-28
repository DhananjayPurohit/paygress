# Paygress Setup Instructions

## 🔐 **Important: Secure Your Configuration**

Before running the Ansible setup, you need to create your configuration files from the templates.

### **Step 1: Create Your Inventory File**

```bash
# Copy the template
cp inventory.ini.template inventory.ini

# Edit with your actual server details
nano inventory.ini
```

Update the following in `inventory.ini`:
- `YOUR_SERVER_IP` → Your actual server IP address
- `YOUR_USERNAME` → Your server username
- `YOUR_PASSWORD` → Your server password (or use SSH key)

### **Step 2: Create Your Environment File**

```bash
# Copy the template
cp paygress.env.template .env

# Edit with your actual configuration
nano .env
```

Update the following in `.env`:
- `nsec1your_private_key_here` → Your actual Nostr private key
- `YOUR_SERVER_IP` → Your server's public IP address
- Update mint URLs if needed
- Update relay URLs if needed

### **Step 3: Run the Setup**

```bash
# Make setup script executable
chmod +x setup-paygress.sh

# Run the setup
./setup-paygress.sh
```

## 🚨 **Security Notes**

- **Never commit** `inventory.ini` or `.env` to git
- These files contain sensitive information (passwords, private keys)
- The `.gitignore` file is configured to exclude these files
- Always use the template files as starting points

## 📁 **File Structure**

```
paygress/
├── inventory.ini.template     # Template for server inventory
├── paygress.env.template      # Template for environment config
├── inventory.ini              # Your actual inventory (not in git)
├── .env                       # Your actual environment (not in git)
├── ansible-setup.yml          # Ansible playbook
├── setup-paygress.sh          # Setup script
└── .gitignore                 # Excludes sensitive files
```

## 🔧 **Alternative: SSH Key Setup (Recommended)**

For better security, use SSH keys instead of passwords:

1. **Generate SSH key:**
   ```bash
   ssh-keygen -t ed25519 -C "your_email@example.com" -f ~/.ssh/paygress_key
   ```

2. **Copy public key to server:**
   ```bash
   ssh-copy-id -i ~/.ssh/paygress_key.pub your_username@your_server_ip
   ```

3. **Update inventory.ini:**
   ```ini
   production ansible_host=YOUR_SERVER_IP ansible_user=YOUR_USERNAME ansible_ssh_private_key_file=~/.ssh/paygress_key
   ```

4. **Remove password lines:**
   ```ini
   # Remove these lines:
   # ansible_ssh_pass=YOUR_PASSWORD
   # ansible_become_pass=YOUR_PASSWORD
   ```
