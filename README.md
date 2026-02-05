# Paygress

## 🎥 Demo
https://github.com/user-attachments/assets/627d2bb1-1a9b-4e66-bc42-7c91a1804fe1

**Decentralized Pay-per-Use Compute with Lightening + Nostr**

Paygress is a platform that allows anyone to buy and sell compute resources instantly using **Lightening** for payments and **Nostr** for discovery and communication. No accounts, no signups, just pay and compute.

🌐 **Website:** [paygress.net](https://paygress.net)

<video width="100%" controls>
  <source src="assets/paygress-demo.mov" type="video/quicktime">
  Your browser does not support the video tag. <a href="assets/paygress-demo.mov">Download video</a>.
</video>

## ✨ Features

- **Anonymous & Private**: No KYC, no accounts. Payments are settled instantly via Cashu tokens.
- **Decentralized Discovery**: Providers broadcast availability via Nostr. Clients discover and negotiate directly.
- **Multi-Backend Support**:
    - **LXD (Native)**: Perfect for VPS and bare metal.
    - **Proxmox VE**: Enterprise-grade virtualization management.
    - **Kubernetes**: Scalable pod provisioning.
- **End-to-End Encryption**: All communication between client and provider is encrypted (NIP-04/NIP-17).

---

## 🚀 Quick Start: Using the Marketplace

The **Paygress CLI** is your gateway to buying compute.

### 1. Build the CLI
```bash
cargo build --bin paygress-cli
# Optional: Install to path
cargo install --path . --bin paygress-cli
```

### 2. List Available Providers
Find providers offering compute resources.
```bash
# List all providers
paygress-cli market list

# Sort by price
paygress-cli market list --sort price

# Filter by capability
paygress-cli market list --capability lxd
```

### 3. Spawn a Workload
Provision a container instantly. You need a **Cashu token** (minted from a Cashu wallet like [Nutstash](https://nutstash.app/)).

```bash
paygress-cli market spawn \
  --provider <PROVIDER_NPUB> \
  --tier basic \
  --token "cashuA..."
```

> **Note:** If you don't provide a `--nostr-key`, the CLI will automatically generate a new identity for you and save it to `~/.paygress/identity`.

**Output:**
```
🎉 Workload Provisioned Successfully!

  Pod ID:   container-1001
  Expires:   2026-01-29T17:15:00+00:00
  Spec:   1 vCPU, 1024 MB RAM

Connection Instructions:
  • 🚀 Workload provisioned successfully!
  • 👤 Username: root
  • 🔑 Password: my-secure-password
  • ⌛ Expires: 2026-01-29 17:15:00 UTC
  • Access: You can connect to the container using SSH.
  •   ssh -p <PORT> root@<PROVIDER_IP>
```

### 4. Connect
Use the provided SSH command to access your container.
```bash
ssh -p <PORT> root@<PROVIDER_IP>
```

---

## ☁️ Become a Provider

Monetize your idle hardware by joining the Paygress network.

### One-Click Bootstrap (LXD/Proxmox)

The CLI can automatically set up your server as a Paygress Provider.

**Requirements:**
- **Linux** (with systemd)
- **LXD** or **Proxmox VE** installed (or let bootstrap install them)
- Root access

```bash
paygress-cli bootstrap \
  --host <YOUR_SERVER_IP> \
  --user ubuntu \
  --port 22 \
  --name "My Compute Node" \
  --location "US-West" \
```

> **Note:** The bootstrap command supports non-root users (like `ubuntu`) and will automatically use `sudo` for installation.

This command will:
1. SSH into your server.
2. Install dependencies (LXD or Proxmox).
3. configure networking and storage.
4. Deploy the Paygress Provider service.
5. Generate a Nostr identity and start broadcasting offers.

---

## 🔧 Supported Backends

Paygress supports multiple compute backends to suit different needs.

| Backend | Description | Best For | Status |
|---------|-------------|----------|--------|
| **LXD** | Lightweight Linux Containers. Fast startup, low overhead. | Linux VPS, Bare Metal | ✅ **Verified** |
| **Proxmox** | Full VM and Container management via API. | Home Labs, Enterprise | ✅ **Verified** |
| **Kubernetes** | Pod provisioning in a K8s cluster. | Scalable Cloud Installs | 🚧 **Beta** |

### Kubernetes Mode
To run Paygress as a Kubernetes operator/gateway:

1. **Deploy Ingress Controller:** Ensure Nginx is set up with `ngx_l402` for payment validation.
2. **Deploy Paygress Service:**
   ```bash
   ./setup-paygress.sh deploy
   ```
3. **Usage:**
   Clients send HTTP requests with Cashu tokens headers to the ingress endpoint.

---

## 🛠️ CLI Command Showcase

### System Management
```bash
# Reset the provider service on a remote host (useful for debugging)
paygress-cli system reset --host <IP>

# View provider logs
ssh root@<IP> "journalctl -u paygress-provider -f"
```

### Market Interactions
```bash
# interactive prompt to pick a provider
paygress-cli market list 

# Spawn with specific image (if supported)
paygress-cli market spawn ... --image "ubuntu:24.04"
```
```bash
# interactive prompt to pick a provider
paygress-cli market list 

# Spawn with specific image (if supported)
paygress-cli market spawn ... --image "ubuntu:24.04"
```

### Direct HTTP API (Centralized Mode)
For private/centralized deployments using the HTTP API:
```bash
paygress-cli spawn \
  -s http://my-private-server.com \
  --tier standard \
  --token "cashuA..."
```

---

## 🏗️ Architecture

1.  **Provider Service**: Runs on the compute node. Listens for NIP-04 encrypted messages on Nostr relays.
2.  **Discovery**: Providers publish advertisements (NIP-01) with 'ephemeral' events or specialized kinds to announce availability.
3.  **Negotiation**: Client sends a `spawn` request with a Cashu token.
4.  **Verification**: Provider verifies the Cashu token against the mint (Preventing double-spends).
5.  **Provisioning**:
    *   **LXD**: Creates a container, sets limits, configures SSH port forwarding.
    *   **Proxmox**: Calls Proxmox API to clone a template/container.
6.  **Access**: Provider sends back IP, Port, and Credentials encrypted to the client.

---
**License**: MIT
