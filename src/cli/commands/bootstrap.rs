// Bootstrap CLI Command
//
// One-click setup for a fresh VPS/machine.
// Installs Proxmox VE + Paygress and configures everything.

use anyhow::{Context, Result};
use clap::Args;
use colored::Colorize;
use nostr_sdk::ToBech32;
use std::io::Write;
use std::process::{Command, Stdio};


#[derive(Args)]
pub struct BootstrapArgs {
    /// Target server IP or hostname
    #[arg(long)]
    pub host: String,
    
    /// SSH user (must have sudo privileges)
    #[arg(long, default_value = "root")]
    pub user: String,
    
    /// SSH password (use --key for key-based auth)
    #[arg(long)]
    pub password: Option<String>,
    
    /// SSH private key path
    #[arg(long)]
    pub key: Option<String>,
    
    /// SSH port
    #[arg(long, default_value = "22")]
    pub port: u16,
    
    /// Provider display name
    #[arg(long)]
    pub name: String,
    
    /// Location description (e.g., "US-East", "Germany")
    #[arg(long)]
    pub location: Option<String>,
    
    /// Nostr private key (nsec format, auto-generated if not provided)
    #[arg(long)]
    pub nostr_key: Option<String>,
    
    /// Whitelisted Cashu mints (comma-separated)
    #[arg(long, default_value = "https://mint.minibits.cash")]
    pub mints: String,
    
    /// Skip Proxmox installation (assumes already installed)
    #[arg(long)]
    pub skip_proxmox: bool,
    
    /// Dry run - show commands without executing
    #[arg(long)]
    pub dry_run: bool,

    /// Install WireGuard for tunnel support (for machines behind NAT)
    #[arg(long)]
    pub tunnel: bool,
}

pub async fn execute(args: BootstrapArgs, verbose: bool) -> Result<()> {
    println!("{}", "╔════════════════════════════════════════════════════════════╗".blue());
    println!("{}", "║              🚀 PAYGRESS BOOTSTRAP                         ║".blue());
    println!("{}", "║     One-Click Proxmox + Provider Setup                     ║".blue());
    println!("{}", "╚════════════════════════════════════════════════════════════╝".blue());
    println!();

    if args.dry_run {
        println!("{}", "🔍 DRY RUN MODE - Commands will be shown but not executed".yellow());
        println!();
    }

    let target = format!("{}@{}", args.user, args.host);
    let is_root = args.user == "root";
    let sudo = if is_root { "" } else { "sudo " };
    
    println!("Target: {}", target.cyan());
    println!("Name:   {}", args.name.yellow());
    if let Some(ref loc) = args.location {
        println!("Location: {}", loc);
    }
    println!();

    // Step 1: Test SSH connection
    println!("{}", "Step 1: Testing SSH Connection".blue().bold());
    println!("{}", "─".repeat(50));
    
    let ssh_test = build_ssh_command(&args, "echo 'SSH connection successful'");
    
    if args.dry_run {
        println!("  Would run: {}", ssh_test.cyan());
    } else {
        print!("  Connecting to {}... ", args.host);
        std::io::stdout().flush()?;
        
        if !run_ssh_command(&args, "echo 'Connected'")? {
            println!("{}", "FAILED".red());
            return Err(anyhow::anyhow!("SSH connection failed"));
        }
        println!("{}", "OK".green());
    }
    println!();

    // Step 2: Check OS & Install Backend
    println!("{}", "Step 2: Checking OS & Installing Backend".blue().bold());
    println!("{}", "─".repeat(50));
    
    let os_id = if args.dry_run {
        println!("  Would detect OS (assuming debian for dry-run)");
        "debian".to_string()
    } else {
        let output = run_ssh_command_output(&args, "cat /etc/os-release | grep ^ID= | cut -d= -f2 | tr -d '\"'")?;
        output.trim().to_string()
    };
    
    println!("  Detected OS: {}", os_id.cyan());
    
    let use_lxd = os_id == "ubuntu";
    
    if use_lxd {
        println!("{}", "  -> Installing LXD backend (Ubuntu detected)".green());
        
        if args.dry_run {
            println!("  Would run: snap install lxd && lxd init --auto");
        } else {
            // Check if LXD is installed
            let check = run_ssh_command_output(&args, "which lxd >/dev/null 2>&1 && echo 'installed' || echo 'not_installed'")?;
            if check.trim() == "installed" {
                println!("  LXD is already installed.");
            } else {
                println!("  Installing LXD...");
                let install_cmd = format!("{}snap install lxd && lxd init --auto", sudo);
                run_ssh_command(&args, &install_cmd)?;
                println!("  LXD installed and initialized!");
            }
            
            // Allow root to use lxc
            // run_ssh_command(&args, "usermod -aG lxd root")?; 
            // Note: snap lxd usually allows root by default or uses lxd group. Root is always allowed usually.
        }
    } else if !args.skip_proxmox {
        // Proxmox (Debian) path
        println!("{}", "  -> Installing Proxmox backend (Debian assumed)".green());
        
        if os_id != "debian" && !args.dry_run {
             println!("{}", format!("⚠️  Warning: OS is not Debian (detected: {}). Proxmox install may fail.", os_id).yellow());
        }

        let proxmox_check = "which pvesh >/dev/null 2>&1 && echo 'installed' || echo 'not_installed'";
        
        if args.dry_run {
            println!("  Would check: {}", proxmox_check.cyan());
        } else {
            print!("  Checking for existing Proxmox... ");
            std::io::stdout().flush()?;
            
            let output = run_ssh_command_output(&args, proxmox_check)?;
            
            if output.trim() == "installed" {
                println!("{}", "Already installed".green());
            } else {
                println!("{}", "Not found".yellow());
                println!();
                println!("  {} Installing Proxmox VE...", "⚙".yellow());
                println!("  {} This may take 10-15 minutes", "⏳".to_string());
                println!();
                
                // Run Proxmox installation script
                let install_script = get_proxmox_install_script();
                // If not root, run with sudo bash
                let cmd = if is_root {
                    install_script.to_string()
                } else {
                    format!("sudo bash -c '{}'", install_script.replace("'", "'\\''"))
                };
                
                run_ssh_command(&args, &cmd)?;
                
                println!("  {} Proxmox VE installed!", "✓".green());
            }
        }
    } else {
        println!("  Skipping Proxmox installation (--skip-proxmox)");
    }
    println!();

    // Step 3: Create API Token
    println!("{}", "Step 3: Creating Proxmox API Token".blue().bold());
    println!("{}", "─".repeat(50));
    
    let token_name = "paygress";
    let create_token_cmd = format!(
        "pveum user token add root@pam {} --privsep=0 2>/dev/null || pveum user token list root@pam 2>/dev/null | grep {}",
        token_name, token_name
    );
    

    
    // Only check for token if we are using Proxmox (skipping for LXD)
    if !use_lxd {
        if args.dry_run {
            println!("  Would run: {}", create_token_cmd.cyan());
        } else {
            print!("  Creating API token... ");
            std::io::stdout().flush()?;
            
            let token_output = run_ssh_command_output(&args, &format!(
                "{}pveum user token add root@pam {} --privsep=0 2>&1 || echo 'exists'",
                sudo, token_name
            ))?;
            
            if token_output.contains("exists") || token_output.contains("already exists") {
                println!("{}", "Already exists".green());
            } else {
                println!("{}", "Created".green());
                if verbose {
                    println!("    Token output: {}", token_output);
                }
            }
        }
    } else {
         println!("  Skipping Proxmox API token creation (LXD mode)");
    }
    println!();

    // Step 4: Install Dependencies & Sync Source
    println!("{}", "Step 4: Installing Dependencies & Syncing Source".blue().bold());
    println!("{}", "─".repeat(50));
    
    let install_deps = format!(r#"
        echo "Checking for Rust environment..."
        if ! command -v cargo &> /dev/null; then
             if [ -f "$HOME/.cargo/env" ]; then source "$HOME/.cargo/env"; fi
        fi
        if ! command -v cargo &> /dev/null; then
             echo "Installing Rust..."
             curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
             source "$HOME/.cargo/env"
        fi
        
        if command -v apt-get &> /dev/null; then
            export DEBIAN_FRONTEND=noninteractive
            {0}apt-get update -q && {0}apt-get install -y build-essential pkg-config libssl-dev rsync
        fi
        
        # Clean build dir
        mkdir -p /tmp/paygress-src
    "#, sudo);
    
    if args.dry_run {
        println!("  Would install deps and sync source");
    } else {
        print!("  Installing system dependencies... ");
        std::io::stdout().flush()?;
        run_ssh_command(&args, &install_deps)?;
        println!("{}", "OK".green());

        if args.tunnel {
            print!("  Installing WireGuard for tunnel support... ");
            std::io::stdout().flush()?;
            let wg_install = format!(
                "export DEBIAN_FRONTEND=noninteractive && {}apt-get install -y wireguard wireguard-tools",
                sudo
            );
            run_ssh_command(&args, &wg_install)?;
            println!("{}", "OK".green());
        }

        println!("  Syncing source code... ");
        
        let mut rsync_args = vec![
             "-az".to_string(),
             "--exclude=target".to_string(),
             "--exclude=.git".to_string(),
             "--exclude=.idea".to_string(),
             "--delete".to_string(),
             ".".to_string(),
        ];
        
        // SSH options
        let ssh_opt = if let Some(ref key) = args.key {
             format!("ssh -o StrictHostKeyChecking=no -p {} -i {}", args.port, key)
        } else {
             format!("ssh -o StrictHostKeyChecking=no -p {}", args.port)
        };
        
        rsync_args.push("-e".to_string());
        rsync_args.push(ssh_opt);
        
        rsync_args.push(format!("{}@{}:/tmp/paygress-src", args.user, args.host));
        
        let status = Command::new("rsync")
            .args(&rsync_args)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .context("Failed to execute rsync")?;
            
        if !status.success() {
             // Fallback hint
             println!("{}", "Rsync failed. Ensure rsync is installed locally.".yellow());
             return Err(anyhow::anyhow!("Failed to sync source code"));
        }
        
        println!("  Compiling Paygress from source (this may take 2-5 mins)...");
        // Install to user cargo bin then copy with sudo
        let build_cmd = format!("source $HOME/.cargo/env 2>/dev/null || true; cargo install --path /tmp/paygress-src --bin paygress-cli --force && {}cp $HOME/.cargo/bin/paygress-cli /usr/local/bin/paygress-cli", sudo);
        
        if !run_ssh_command(&args, &build_cmd)? {
             return Err(anyhow::anyhow!("Compilation/Installation failed"));
        }
        
        println!("{}", "OK".green());
    }
    println!();

    // Step 5: Generate Nostr Key
    println!("{}", "Step 5: Configuring Nostr".blue().bold());
    println!("{}", "─".repeat(50));
    
    let nostr_key = match args.nostr_key {
        Some(ref key) => {
            println!("  Using provided Nostr key");
            key.clone()
        }
        None => {
            print!("  Generating Nostr keypair... ");
            std::io::stdout().flush()?;
            
            let keys = nostr_sdk::Keys::generate();
            let nsec = keys.secret_key()
                .map_err(|e| anyhow::anyhow!("Failed to get secret key: {}", e))?
                .to_bech32()
                .map_err(|e| anyhow::anyhow!("Failed to encode key: {}", e))?;
            let npub = keys.public_key().to_bech32()
                .map_err(|e| anyhow::anyhow!("Failed to encode public key: {}", e))?;
            
            println!("{}", "Done".green());
            println!("  NPUB: {}", npub.cyan());
            nsec
        }
    };
    println!();

    // Step 6: Create Configuration
    println!("{}", "Step 6: Creating Provider Configuration".blue().bold());
    println!("{}", "─".repeat(50));
    
    // Explicitly set backend type based on OS detection, otherwise it defaults to Proxmox
    let backend_type = if use_lxd { "LXD" } else { "Proxmox" };
    let proxmox_template = if use_lxd { "images:ubuntu/22.04" } else { "local:vztmpl/ubuntu-22.04-standard.tar.zst" };
    let storage = if use_lxd { "default" } else { "local-lvm" }; // LXD default pool is usually 'default'
    let bridge = if use_lxd { "lxdbr0" } else { "vmbr0" }; // LXD default bridge is lxdbr0
    
    let config = format!(r#"{{
  "backend_type": "{}",
  "proxmox_url": "https://127.0.0.1:8006/api2/json",
  "proxmox_token_id": "root@pam!paygress",
  "proxmox_token_secret": "REPLACE_WITH_TOKEN",
  "proxmox_node": "pve",
  "proxmox_storage": "{}",
  "proxmox_template": "{}",
  "proxmox_bridge": "{}",
  "vmid_range_start": 1000,
  "vmid_range_end": 1999,
  "nostr_private_key": "{}",
  "nostr_relays": ["wss://relay.damus.io", "wss://nos.lol"],
  "provider_name": "{}",
  "provider_location": {},
  "public_ip": "{}",
  "capabilities": ["lxc", "vm"],
  "specs": [
    {{"id": "basic", "name": "Basic", "description": "1 vCPU, 1GB RAM", "cpu_millicores": 1000, "memory_mb": 1024, "rate_msats_per_sec": 50}},
    {{"id": "standard", "name": "Standard", "description": "2 vCPU, 2GB RAM", "cpu_millicores": 2000, "memory_mb": 2048, "rate_msats_per_sec": 100}}
  ],
  "whitelisted_mints": ["{}"],
  "heartbeat_interval_secs": 60,
  "minimum_duration_seconds": 60
}}"#,
        backend_type,
        storage,
        proxmox_template,
        bridge,
        nostr_key,
        args.name,
        args.location.as_ref().map(|l| format!("\"{}\"", l)).unwrap_or("null".to_string()),
        args.host, // <--- Added arg
        args.mints
    );

    if args.dry_run {
        println!("  Would create /etc/paygress/provider-config.json");
    } else {
        let create_config = if is_root {
            format!(
                "mkdir -p /etc/paygress && cat > /etc/paygress/provider-config.json << 'EOF'\n{}\nEOF",
                config
            )
        } else {
             format!(
                "{}mkdir -p /etc/paygress && echo '{}' | {}tee /etc/paygress/provider-config.json > /dev/null",
                sudo, config.replace("'", "'\\''"), sudo
            )
        };
        run_ssh_command(&args, &create_config)?;
        println!("  {} Created /etc/paygress/provider-config.json", "✓".green());
    }
    println!();

    // Step 7: Create Systemd Service
    println!("{}", "Step 7: Setting Up Systemd Service".blue().bold());
    println!("{}", "─".repeat(50));
    
    let systemd_service = r#"[Unit]
Description=Paygress Provider Service
After=network.target pve-cluster.service

[Service]
Type=simple
ExecStart=/usr/local/bin/paygress-cli provider start --config /etc/paygress/provider-config.json
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
"#;

    if args.dry_run {
        println!("  Would create /etc/systemd/system/paygress-provider.service");
    } else {
        let create_service = if is_root {
            format!(
                "cat > /etc/systemd/system/paygress-provider.service << 'EOF'\n{}\nEOF\nsystemctl daemon-reload",
                systemd_service
            )
        } else {
             format!(
                "echo '{}' | {}tee /etc/systemd/system/paygress-provider.service > /dev/null && {}systemctl daemon-reload",
                systemd_service.replace("'", "'\\''"), sudo, sudo
            )
        };
        run_ssh_command(&args, &create_service)?;
        println!("  {} Created systemd service", "✓".green());
    }
    println!();

    // Step 8: Start Service
    println!("{}", "Step 8: Starting Provider Service".blue().bold());
    println!("{}", "─".repeat(50));
    
    if args.dry_run {
        println!("  Would run: systemctl enable --now paygress-provider");
    } else {
        if use_lxd {
             let start_cmd = format!("{}systemctl enable paygress-provider && {}systemctl restart paygress-provider", sudo, sudo);
             run_ssh_command(&args, &start_cmd)?;
             println!("  {} Service started successfully!", "✓".green());
        } else {
            // Don't actually start yet since config needs token
            println!("  {} Service configured (not started - needs API token)", "✓".green());
            println!();
            println!("  To complete setup, SSH into the server and:");
            println!("    1. Get your API token: pveum user token list root@pam");
            println!("    2. Update /etc/paygress/provider-config.json");
            println!("    3. Start: systemctl enable --now paygress-provider");
        }
    }
    println!();

    // Summary
    println!("{}", "═".repeat(60).green());
    println!("{}", "🎉 BOOTSTRAP COMPLETE!".green().bold());
    println!("{}", "═".repeat(60).green());
    println!();
    println!("  Provider Name: {}", args.name.yellow());
    println!("  Server:        {}", args.host.cyan());
    
    if !use_lxd {
        println!("  Proxmox UI:    https://{}:8006", args.host);
        println!();
        println!("  {} Next Steps:", "📋".to_string());
        println!("    1. SSH into {} and get your API token", args.host);
        println!("    2. Update the config with the token secret");
        println!("    3. Start the service: systemctl start paygress-provider");
    } else {
        println!("  Backend:       LXD (Native)");
        println!("  Status:        Running 🟢");
    }
    
    println!();
    println!("  Users can discover you with:");
    println!("    {} market list", "paygress-cli".cyan());
    println!();

    Ok(())
}

fn build_ssh_command(args: &BootstrapArgs, cmd: &str) -> String {
    let mut ssh = format!("ssh -o StrictHostKeyChecking=no -p {} ", args.port);
    
    if let Some(ref key) = args.key {
        ssh.push_str(&format!("-i {} ", key));
    }
    
    ssh.push_str(&format!("{}@{} '{}'", args.user, args.host, cmd));
    ssh
}

fn run_ssh_command(args: &BootstrapArgs, cmd: &str) -> Result<bool> {
    let mut ssh_args = vec![
        "-o".to_string(),
        "StrictHostKeyChecking=no".to_string(),
        "-t".to_string(),
        "-p".to_string(),
        args.port.to_string(),
    ];
    
    if let Some(ref key) = args.key {
        ssh_args.push("-i".to_string());
        ssh_args.push(key.clone());
    }
    
    ssh_args.push(format!("{}@{}", args.user, args.host));
    ssh_args.push(cmd.to_string());
    
    let status = Command::new("ssh")
        .args(&ssh_args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .context("Failed to execute SSH command")?;
    
    Ok(status.success())
}

fn run_ssh_command_output(args: &BootstrapArgs, cmd: &str) -> Result<String> {
    let mut ssh_args = vec![
        "-o".to_string(),
        "StrictHostKeyChecking=no".to_string(),
        "-p".to_string(),
        args.port.to_string(),
    ];
    
    if let Some(ref key) = args.key {
        ssh_args.push("-i".to_string());
        ssh_args.push(key.clone());
    }
    
    ssh_args.push(format!("{}@{}", args.user, args.host));
    ssh_args.push(cmd.to_string());
    
    let output = Command::new("ssh")
        .args(&ssh_args)
        .output()
        .context("Failed to execute SSH command")?;
    
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn get_proxmox_install_script() -> &'static str {
    r#"
# Proxmox VE Installation Script
set -e

# Check OS information
if [ -f /etc/os-release ]; then
    . /etc/os-release
    OS=$ID
    VERSION=$VERSION_ID
else
    echo "ERROR: Cannot detect OS"
    exit 1
fi

echo "Detected OS: $OS $VERSION"

# Proxmox VE 8.x requires Debian 12 (Bookworm)
if [ "$OS" != "debian" ] || [ "$VERSION" != "12" ]; then
    echo "ERROR: Proxmox VE installation requires Debian 12 (Bookworm)."
    echo "Current OS is $PRETTY_NAME."
    echo "Please rebuild this server with Debian 12 and try again."
    exit 1
fi

# Add Proxmox repository
echo "Adding Proxmox repository..."
echo "deb [arch=amd64] http://download.proxmox.com/debian/pve bookworm pve-no-subscription" > /etc/apt/sources.list.d/pve-install-repo.list

# Add repository key
wget https://enterprise.proxmox.com/debian/proxmox-release-bookworm.gpg -O /etc/apt/trusted.gpg.d/proxmox-release-bookworm.gpg

# Add /etc/hosts entry for itself if missing (required for Proxmox request)
IP=$(hostname -I | awk '{print $1}')
HOSTNAME=$(hostname)
if ! grep -q "$IP $HOSTNAME" /etc/hosts; then
    echo "Adding host entry to /etc/hosts..."
    echo "$IP $HOSTNAME.local $HOSTNAME" >> /etc/hosts
fi

# Update and install
export DEBIAN_FRONTEND=noninteractive
apt-get update
apt-get full-upgrade -y
apt-get install -y proxmox-ve postfix open-iscsi chrony

# Remove os-prober (conflicts with Proxmox)
apt-get remove -y os-prober 2>/dev/null || true

echo "Proxmox VE installation complete!"
echo "A reboot may be required."
"#
}
