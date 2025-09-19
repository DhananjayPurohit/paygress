# Paygress Testing Script

This README provides instructions for using the `test-paygress.sh` script to test your Paygress setup with NIP-17 private direct messages. NIP-17 handles encryption/decryption automatically - no manual encryption required.

## 📋 Prerequisites

Before running the testing script, ensure you have:

1. **Built the Cashu CDK CLI**:
   ```bash
   cd ../cdk && cargo build --bin cdk-cli --release
   ```

2. **Installed required tools**:
   - `nak` (Nostr CLI tool)
   - `jq` (JSON processor)
   - `ssh` (SSH client)
   - `ansible` (for running the setup playbook)

3. **Built the Cashu CDK CLI**:
   ```bash
   cd ../cdk && cargo build --bin cdk-cli --release
   ```

4. **Made CDK CLI Accessible System-Wide** (Optional but recommended):
   The CDK CLI binary is built at `../cdk/target/release/cdk-cli`. To make it accessible from anywhere, you can either:
   
   **Option A: Add to PATH** (Recommended):
   ```bash
   # Add to your shell profile (.bashrc, .zshrc, etc.)
   echo 'export PATH="$HOME/cdk/target/release:$PATH"' >> ~/.bashrc
   source ~/.bashrc
   ```
   
   **Option B: Create symbolic link**:
   ```bash
   # Create a symbolic link in /usr/local/bin (requires sudo)
   sudo ln -s $HOME/cdk/target/release/cdk-cli /usr/local/bin/cdk-cli
   ```

5. **Note on Cashu Mint**:
   The script uses the `nofees.testnut.cashu.space` mint for testing, which is a test mint with no fees.
   You can replace this with any other Cashu mint by modifying the script.

## 🚀 Usage

### Method 1: Command Line Argument (Recommended)

Pass the service public key as a command-line argument:

```bash
./test-paygress.sh npub1your_service_public_key_here
```

Example:
```bash
./test-paygress.sh npub17fhydww6c2pxqmrqe4xtefgwkm59uxw2upt5xp6dla67u78q6efq3ecqkh
```

### Method 2: Interactive Prompt

Run the script without arguments and enter the service public key when prompted:

```bash
./test-paygress.sh
```

The script will prompt you to enter the service public key:
```
📡 Paygress Service Public Key Required
=====================================
Please provide the service public key (npub1... format)
You can get this by running: kubectl logs -n ingress-system -l app=paygress-sidecar

Enter service public key (npub1...): 
```

## 🛠️ Script Workflow

The testing script automatically performs the following steps:

1. **Generates User Keypair**: Creates a new Nostr keypair for testing
2. **Configures Service Key**: Uses the provided service public key
3. **Generates Cashu Tokens**: Creates payment tokens using the CDK CLI
4. **Sends Encrypted Request**: Creates and sends an encrypted Nostr provisioning request
5. **Listens for Response**: Monitors Nostr relays for the service response
6. **Decrypts Credentials**: Extracts SSH access details from the response
7. **Provides Connection Info**: Displays SSH connection details for the pod

## 📝 Example Output

```
🚀 Paygress Testing Script
==========================

🔐 Step 1: Generating User Keypair
----------------------------------
Generated private key (hex): 8d3f031a8b870c967a50011a1cb2cd2aeab63a24cc3459346d6a8cfa4a1257ee
User private key (bech32/nsec): nsec135lsxx5tsuxfv7jsqydpevkd9t4tvw3yes69jdrdd2x05jsj2lhqkq4dnk
User public key (hex): 1d9257727bd2b6734f9044a72db2584ea77d44a6e762079e5da4bf6a3b89c511
User public key (bech32/npub): npub1rkf9wunm62m8xnusgjnjmvjcf6nh639xua3q08ja5jlk5wufc5gscletnp
✅ User keypair generated and exported

📡 Step 2: Configuring Service Public Key
-----------------------------------------
Service public key (bech32): npub1w9xusq8ueyh0f2szrhrdzk8xq4hw72kzvgm25dp72kr9qkmpx4ps2u323e
Service public key (hex): 1d9257727bd2b6734f9044a72db2584ea77d44a6e762079e5da4bf6a3b89c511
✅ Service public key configured
```

## ⚠️ Troubleshooting

### Common Issues:

1. **"CDK CLI binary not found"**:
   - Ensure you've built the CDK CLI: `cd ../cdk && cargo build --release`
   - Verify the binary exists: `ls -la ../cdk/target/release/cdk-cli`

2. **"Failed to generate Cashu token"**:
   - Check internet connectivity
   - Verify the test mint is accessible: `curl https://mint.cashu.space/info`
   - Ensure protobuf-compiler is installed: `sudo apt-get install protobuf-compiler`

3. **"Incorrect Usage: flag provided but not defined: -hex"**:
   - This indicates an issue with the `nak` command version
   - The script has been updated to use `nak decode` instead

4. **Invalid service public key format**:
   - Ensure the key starts with `npub1` followed by valid base32 characters
   - Example: `npub1w9xusq8ueyh0f2szrhrdzk8xq4hw72kzvgm25dp72kr9qkmpx4ps2u323e`

## 🔧 Making CDK CLI Accessible

If you encounter issues with the `cdk-cli` command not being found, follow these steps to make it accessible system-wide:

**Option 1: Add to PATH** (Recommended for current user)
```bash
# Add the CDK release directory to your PATH
echo 'export PATH="$HOME/cdk/target/release:$PATH"' >> ~/.bashrc
source ~/.bashrc

# Verify it works
cdk-cli --help
```

**Option 2: Create symbolic link** (System-wide access)
```bash
# Create a symbolic link in /usr/local/bin (requires sudo)
sudo ln -s $HOME/cdk/target/release/cdk-cli /usr/local/bin/cdk-cli

# Verify it works
cdk-cli --help
```

**Option 3: Use full path** (No changes needed)
You can always use the full path to the binary:
```bash
../cdk/target/release/cdk-cli mint 1000 --url https://nofees.testnut.cashu.space
```

## 🎮 Running the Ansible Setup Script

If you want to automate the setup process or reproduce the environment on another machine, you can use the provided Ansible playbook:

```bash
# Run the Ansible playbook to set up the environment
ansible-playbook paygress-setup.yml

# Run specific tasks using tags
ansible-playbook paygress-setup.yml --tags "setup,install"
ansible-playbook paygress-setup.yml --tags "build" --skip-tags "install"
```

The playbook includes the following tags:
- `setup`: General setup tasks
- `install`: Package installation
- `clone`: Repository cloning
- `build`: Building CDK CLI
- `verify`: Verification steps

## 📋 Usage Instructions

1. Make the script executable:
   ```bash
   chmod +x test-paygress.sh
   ```

2. Run the script with your service public key:
   ```bash
   ./test-paygress.sh npub1your_service_key_here
   ```

3. Follow the prompts and monitor the Nostr relays for responses

4. When you receive the response, extract the SSH connection details to access your pod

## 🔧 Important Notes

- **Pod Duration**: Based on payment (1 sat = 1 minute)
- **Ensure CDK CLI**: Built before running: `cd ../cdk && cargo build --release`
- **Sufficient Tokens**: Make sure you have sufficient Cashu tokens for the requested duration
- **Response Monitoring**: Keep the script running while waiting for the service response

## 🎉 Success

When the script completes successfully, you'll receive SSH connection details that look like:
```
📋 SSH Access Details:
   Pod Name: ssh-pod-abc12345
   SSH Username: alice
   SSH Password: your_generated_password
   Node Port: 31234
```

Use these details to connect to your provisioned pod via SSH.