# [Paygress](https://paygress.net)

**Pay-per-use compute with Lightning + Nostr. No accounts, no signups.**

https://github.com/user-attachments/assets/627d2bb1-1a9b-4e66-bc42-7c91a1804fe1

Paygress is a marketplace where anyone can buy or sell compute resources using Cashu ecash tokens. Providers advertise on Nostr, consumers discover and pay - all anonymous, all instant.

## Install

```bash
cargo install --path . --bin paygress-cli
```

---

## For Consumers

### 1. Find a provider

```bash
# Browse all providers on Nostr
paygress-cli list

# Filter and sort
paygress-cli list --online-only --sort price

# Get details on a specific provider
paygress-cli list info <PROVIDER_NPUB>
```

### 2. Spawn a workload

Get a Cashu token from a wallet like [Nutstash](https://nutstash.app/) or [Minibits](https://www.minibits.cash/), then:

```bash
paygress-cli spawn \
  --provider <PROVIDER_NPUB> \
  --tier basic \
  --token "cashuA..." \
  --ssh-pass "my-password"
```

The CLI auto-generates a Nostr identity at `~/.paygress/identity` on first use.

### 3. Connect

```bash
ssh -p <PORT> root@<PROVIDER_IP>
```

### 4. Top up or check status

```bash
# Extend your workload
paygress-cli topup --pod-id <ID> --provider <NPUB> --token "cashuA..."

# Check remaining time
paygress-cli status --pod-id <ID> --provider <NPUB>
```

### HTTP Mode

For centralized deployments (Kubernetes + Nginx L402 paywall), pass `--server` instead of `--provider`:

```bash
paygress-cli list --server http://my-server:8080
paygress-cli spawn --server http://my-server:8080 --tier basic --token "cashuA..." --ssh-pass "pw"
paygress-cli status --server http://my-server:8080 --pod-id <ID>
```

---

## For Providers

### Quick Start: One-Click Bootstrap

Set up any Linux VPS as a provider with a single command:

```bash
paygress-cli bootstrap \
  --host <YOUR_SERVER_IP> \
  --user root \
  --name "My Node" \
  --mints "https://testnut.cashu.space"
```

This will SSH into your server, install LXD (on Ubuntu) or Proxmox (on Debian), compile Paygress, configure a systemd service, and start broadcasting offers to Nostr.

**Requirements:** Linux with systemd, root/sudo access, public IP.

### Manual Setup

```bash
# 1. Setup (generates config at provider-config.json)
paygress-cli provider setup \
  --proxmox-url https://127.0.0.1:8006/api2/json \
  --token-id "root@pam!paygress" \
  --token-secret "<SECRET>" \
  --name "My Provider" \
  --mints "https://testnut.cashu.space"

# 2. Start
paygress-cli provider start --config provider-config.json

# 3. Check status
paygress-cli provider status
```

### Provider Management

```bash
# Stop the service
paygress-cli provider stop

# View live logs
journalctl -u paygress-provider -f

# Reset (remove all Paygress data from a server)
paygress-cli system reset --host <IP> --user root
```

---

## Supported Backends

| Backend | Best For | Status |
|---------|----------|--------|
| **LXD** | Ubuntu VPS, bare metal | Verified |
| **Proxmox** | Home labs, Debian servers | Verified |
| **Kubernetes** | Scalable cloud (HTTP/L402 mode) | Beta |

## Architecture

**Decentralized (Nostr + LXD/Proxmox):**
Provider publishes offers (Kind 38383) and heartbeats (Kind 38384) to Nostr relays. Consumer sends encrypted spawn request with Cashu token. Provider verifies payment, creates container, returns SSH credentials - all via encrypted Nostr DMs.

**Centralized (Kubernetes):**
Nginx with `ngx_l402` validates Cashu tokens. Paygress provisions K8s pods with SSH access. Clients interact via HTTP API.

---
