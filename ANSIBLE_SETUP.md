# Ansible Setup for Paygress

This document explains how to use the Ansible playbook to deploy Paygress.

## Configuration

All configuration is done through the `inventory.ini` file. The Ansible script will:

1. Clone the repository
2. Update the `.env` file with settings from `inventory.ini`
3. Build and deploy the service

## Required Configuration

Before running the Ansible playbook, configure your `inventory.ini` file:

```ini
[paygress_servers]
production ansible_host=YOUR-SERVER-IP ansible_user=YOUR-USERNAME ansible_ssh_pass=YOUR-PASSWORD ansible_become_pass=YOUR-PASSWORD ansible_ssh_port=22

[paygress_servers:vars]
public_ip=YOUR-SERVER-IP
ssh_port_start=1000
ssh_port_end=1999

# Nostr Configuration
nostr_relays=wss://relay.damus.io,wss://nos.lol,wss://relay.nostr.band,wss://nostr.wine,wss://nostr.mom
nostr_private_key=nsec1...  # Replace with your actual private key

# Cashu Configuration
whitelisted_mints=https://nofees.testnut.cashu.space,https://testnut.cashu.space
```

## Running the Playbook

1. Copy the template and configure it:
   ```bash
   cp inventory.ini.template inventory.ini
   # Edit inventory.ini with your server details and configuration
   ```

2. Run the playbook:
   ```bash
   ansible-playbook ansible-setup.yml
   ```

## What the Playbook Does

- Installs Docker, Kubernetes, and Rust
- Clones the Paygress repository
- Updates `.env` with server-specific settings
- Builds the Paygress service
- Creates and starts the systemd service
- Configures firewall rules

## After Deployment

1. Update your `inventory.ini` file with actual values (if needed)
2. Re-run the playbook to apply changes: `ansible-playbook ansible-setup.yml`
3. Start the service: `sudo systemctl start paygress`
4. Check status: `sudo systemctl status paygress`
5. View logs: `sudo journalctl -u paygress -f`