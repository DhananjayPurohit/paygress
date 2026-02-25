// Provider CLI Commands
//
// Commands for machine operators to setup and run a Paygress provider.

use anyhow::Result;
use clap::{Args, Subcommand};
use colored::Colorize;
use nostr_sdk::ToBech32;
use std::process::Command;

use paygress::provider::{ProviderConfig, ProviderService, load_config, save_config};
use paygress::nostr::PodSpec;

const CONFIG_PATH: &str = "/etc/paygress/provider-config.json";


#[derive(Args)]
pub struct ProviderArgs {
    #[command(subcommand)]
    pub action: ProviderAction,
}

#[derive(Subcommand)]
pub enum ProviderAction {
    /// Initial setup - configure Proxmox connection and provider settings
    Setup(SetupArgs),

    /// Start the provider service (heartbeats + request handler)
    Start(StartArgs),

    /// Stop the provider service
    Stop,

    /// Show provider status and configuration
    Status,

    /// Edit configuration
    Config(ConfigArgs),

    /// Setup WireGuard VPN tunnel for providers behind NAT
    Tunnel(TunnelArgs),
}

#[derive(Args)]
pub struct SetupArgs {
    /// Proxmox API URL (e.g., https://192.168.1.100:8006/api2/json)
    #[arg(long)]
    pub proxmox_url: String,
    
    /// Proxmox API token ID (e.g., root@pam!paygress)
    #[arg(long)]
    pub token_id: String,
    
    /// Proxmox API token secret
    #[arg(long)]
    pub token_secret: String,
    
    /// Proxmox node name
    #[arg(long, default_value = "pve")]
    pub node: String,
    
    /// Storage pool name
    #[arg(long, default_value = "local-lvm")]
    pub storage: String,
    
    /// LXC template path
    #[arg(long, default_value = "local:vztmpl/ubuntu-22.04-standard.tar.zst")]
    pub template: String,
    
    /// Network bridge
    #[arg(long, default_value = "vmbr0")]
    pub bridge: String,
    
    /// Nostr private key (nsec format, auto-generated if not provided)
    #[arg(long)]
    pub nostr_key: Option<String>,
    
    /// Provider display name
    #[arg(long)]
    pub name: String,
    
    /// Location description (e.g., "US-East", "Germany")
    #[arg(long)]
    pub location: Option<String>,
    
    /// Public IP address (auto-detected if not provided)
    #[arg(long)]
    pub public_ip: Option<String>,

    /// Whitelisted Cashu mints (comma-separated)
    #[arg(long, default_value = "https://mint.minibits.cash")]
    pub mints: String,
}

#[derive(Args)]
pub struct StartArgs {
    /// Path to configuration file
    #[arg(long, default_value = "/etc/paygress/provider-config.json")]
    pub config: String,
    
    /// Run in foreground (don't daemonize)
    #[arg(long, default_value = "true")]
    pub foreground: bool,
}

#[derive(Args)]
pub struct ConfigArgs {
    /// Show current configuration
    #[arg(long)]
    pub show: bool,

    /// Edit a specific setting
    #[arg(long)]
    pub set: Option<String>,

    /// Value for the setting
    #[arg(long)]
    pub value: Option<String>,
}

#[derive(Args)]
pub struct TunnelArgs {
    /// VPN service URL (e.g., https://vpn.cashu.icu)
    #[arg(long)]
    pub vpn_url: String,

    /// Cashu token to pay for VPN access
    #[arg(long)]
    pub token: String,

    /// WireGuard interface name
    #[arg(long, default_value = "wg0")]
    pub interface: String,
}

pub async fn execute(args: ProviderArgs, verbose: bool) -> Result<()> {
    match args.action {
        ProviderAction::Setup(setup_args) => execute_setup(setup_args, verbose).await,
        ProviderAction::Start(start_args) => execute_start(start_args, verbose).await,
        ProviderAction::Stop => execute_stop(verbose).await,
        ProviderAction::Status => execute_status(verbose).await,
        ProviderAction::Config(config_args) => execute_config(config_args, verbose).await,
        ProviderAction::Tunnel(tunnel_args) => execute_tunnel(tunnel_args, verbose).await,
    }
}

async fn execute_setup(args: SetupArgs, verbose: bool) -> Result<()> {
    println!("{}", "🔧 Paygress Provider Setup".blue().bold());
    println!("{}", "━".repeat(50).blue());
    println!();

    // Generate Nostr key if not provided
    let nostr_key = match args.nostr_key {
        Some(key) => {
            println!("  {} Using provided Nostr key", "✓".green());
            key
        }
        None => {
            println!("  {} Generating new Nostr keypair...", "⚙".yellow());
            let keys = nostr_sdk::Keys::generate();
            let nsec = keys.secret_key()
                .map_err(|e| anyhow::anyhow!("Failed to get secret key: {}", e))?
                .to_bech32()
                .map_err(|e| anyhow::anyhow!("Failed to encode key: {}", e))?;
            println!("  {} Generated new keypair", "✓".green());
            nsec
        }
    };

    // Create default specs
    let specs = vec![
        PodSpec {
            id: "basic".to_string(),
            name: "Basic".to_string(),
            description: "1 vCPU, 1GB RAM - Great for testing".to_string(),
            cpu_millicores: 1000,
            memory_mb: 1024,
            rate_msats_per_sec: 50,
        },
        PodSpec {
            id: "standard".to_string(),
            name: "Standard".to_string(),
            description: "2 vCPU, 2GB RAM - General purpose".to_string(),
            cpu_millicores: 2000,
            memory_mb: 2048,
            rate_msats_per_sec: 100,
        },
        PodSpec {
            id: "premium".to_string(),
            name: "Premium".to_string(),
            description: "4 vCPU, 4GB RAM - High performance".to_string(),
            cpu_millicores: 4000,
            memory_mb: 4096,
            rate_msats_per_sec: 200,
        },
    ];

    let mints: Vec<String> = args.mints.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    // Determine public IP
    let public_ip = match args.public_ip {
        Some(ip) => ip,
        None => {
            println!("  {} Auto-detecting public IP...", "⚙".yellow());
            match reqwest::get("https://api.ipify.org").await {
                Ok(resp) => match resp.text().await {
                    Ok(ip) => {
                        println!("  {} Detected: {}", "✓".green(), ip.trim());
                        ip.trim().to_string()
                    }
                    Err(_) => {
                        println!("  {} Could not auto-detect IP, using 127.0.0.1", "⚠".yellow());
                        "127.0.0.1".to_string()
                    }
                },
                Err(_) => {
                    println!("  {} Could not auto-detect IP, using 127.0.0.1", "⚠".yellow());
                    "127.0.0.1".to_string()
                }
            }
        }
    };

    // Create configuration
    let config = ProviderConfig {
        backend_type: Default::default(),
        public_ip,
        proxmox_url: args.proxmox_url,
        proxmox_token_id: args.token_id,
        proxmox_token_secret: args.token_secret,
        proxmox_node: args.node,
        proxmox_storage: args.storage,
        proxmox_template: args.template,
        proxmox_bridge: args.bridge,
        vmid_range_start: 1000,
        vmid_range_end: 1999,
        nostr_private_key: nostr_key,
        nostr_relays: vec![
            "wss://relay.damus.io".to_string(),
            "wss://nos.lol".to_string(),
            "wss://relay.nostr.band".to_string(),
        ],
        provider_name: args.name.clone(),
        provider_location: args.location,
        capabilities: vec!["lxc".to_string(), "vm".to_string()],
        specs,
        whitelisted_mints: mints,
        heartbeat_interval_secs: 60,
        minimum_duration_seconds: 60,
        tunnel_enabled: false,
        tunnel_interface: None,
        ssh_port_start: None,
        ssh_port_end: None,
    };

    // Save configuration
    save_config(CONFIG_PATH, &config)?;
    println!("  {} Configuration saved to {}", "✓".green(), CONFIG_PATH);

    // Test Proxmox connection
    println!();
    println!("  {} Testing Proxmox connection...", "⚙".yellow());
    
    match paygress::proxmox::ProxmoxClient::new(
        &config.proxmox_url,
        &config.proxmox_token_id,
        &config.proxmox_token_secret,
        &config.proxmox_node,
    ) {
        Ok(client) => {
            match client.get_node_status().await {
                Ok(status) => {
                    println!("  {} Proxmox connected!", "✓".green());
                    println!("      Node CPU: {:.1}%", status.cpu * 100.0);
                    println!("      Memory: {} MB used", status.memory.used / (1024 * 1024));
                }
                Err(e) => {
                    println!("  {} Proxmox connection failed: {}", "✗".red(), e);
                    println!("      Check your API token and URL");
                }
            }
        }
        Err(e) => {
            println!("  {} Failed to create Proxmox client: {}", "✗".red(), e);
        }
    }

    println!();
    println!("{}", "━".repeat(50).blue());
    println!("{}", "🎉 Setup Complete!".green().bold());
    println!();
    println!("To start your provider, run:");
    println!("  {} provider start", "paygress-cli".cyan());
    println!();
    println!("Your provider name: {}", args.name.yellow());
    
    Ok(())
}

async fn execute_start(args: StartArgs, verbose: bool) -> Result<()> {
    println!("{}", "🚀 Starting Paygress Provider".blue().bold());
    println!();

    // Load configuration
    let config = load_config(&args.config)?;
    
    println!("  Provider: {}", config.provider_name.yellow());
    
    match config.backend_type {
        paygress::provider::BackendType::Proxmox => {
            println!("  Backend:  Proxmox");
            println!("  URL:      {}", config.proxmox_url);
            println!("  Node:     {}", config.proxmox_node);
        }
        paygress::provider::BackendType::LXD => {
            println!("  Backend:  LXD");
            println!("  Storage:  {}", config.proxmox_storage); // Used as pool name
        }
    }
    println!();

    // Create and run the provider service
    let service = ProviderService::new(config).await?;
    
    println!("  NPUB: {}", service.get_npub().cyan());
    println!();
    println!("{}", "Provider is now live! Press Ctrl+C to stop.".green());
    println!("{}", "━".repeat(50).blue());
    println!();

    // Run the service
    service.run().await?;

    Ok(())
}

async fn execute_stop(_verbose: bool) -> Result<()> {
    println!("{}", "Stopping provider service...".yellow());

    // Try systemctl first (for bootstrapped providers)
    let output = std::process::Command::new("systemctl")
        .args(["stop", "paygress-provider"])
        .output();

    match output {
        Ok(o) if o.status.success() => {
            println!("{}", "Provider stopped via systemctl.".green());
            return Ok(());
        }
        _ => {}
    }

    // Fallback: find and kill the process
    let output = std::process::Command::new("pgrep")
        .args(["-f", "paygress-cli provider start"])
        .output();

    if let Ok(o) = output {
        if o.status.success() {
            let pids = String::from_utf8_lossy(&o.stdout);
            for pid in pids.trim().lines() {
                let _ = std::process::Command::new("kill")
                    .arg(pid.trim())
                    .output();
            }
            println!("{}", "Provider stopped.".green());
            return Ok(());
        }
    }

    println!("{}", "No running provider found.".yellow());
    Ok(())
}

async fn execute_status(_verbose: bool) -> Result<()> {
    println!("{}", "📊 Provider Status".blue().bold());
    println!("{}", "━".repeat(50).blue());
    
    // Try to load config
    match load_config(CONFIG_PATH) {
        Ok(config) => {
            println!();
            println!("  Provider Name:  {}", config.provider_name.yellow());
            println!("  Location:       {}", config.provider_location.as_deref().unwrap_or("Not set"));
            println!("  Proxmox URL:    {}", config.proxmox_url);
            println!("  Node:           {}", config.proxmox_node);
            println!();
            println!("  {} Tiers configured:", "📦".to_string());
            for spec in &config.specs {
                println!("    • {} - {} msat/sec", spec.name, spec.rate_msats_per_sec);
            }
            println!();
            println!("  {} Accepted mints:", "💰".to_string());
            for mint in &config.whitelisted_mints {
                println!("    • {}", mint);
            }
            if config.tunnel_enabled {
                println!();
                println!("  {} Tunnel:", "🔒".to_string());
                println!("    Interface: {}", config.tunnel_interface.as_deref().unwrap_or("wg0"));
                println!("    Public IP: {}", config.public_ip);
                if let (Some(ps), Some(pe)) = (config.ssh_port_start, config.ssh_port_end) {
                    println!("    Port range: {}-{}", ps, pe);
                }
                // Check if WireGuard interface is up
                let iface = config.tunnel_interface.as_deref().unwrap_or("wg0");
                let wg_status = Command::new("wg")
                    .args(["show", iface])
                    .output();
                match wg_status {
                    Ok(o) if o.status.success() => println!("    Status: {}", "UP".green()),
                    _ => println!("    Status: {}", "DOWN".red()),
                }
            }
        }
        Err(_) => {
            println!();
            println!("  {} No configuration found.", "⚠".yellow());
            println!("  Run 'paygress-cli provider setup' first.");
        }
    }
    
    println!();
    Ok(())
}

async fn execute_config(args: ConfigArgs, _verbose: bool) -> Result<()> {
    if args.show {
        let config = load_config(CONFIG_PATH)?;
        let json = serde_json::to_string_pretty(&config)?;
        println!("{}", json);
        return Ok(());
    }

    if let (Some(key), Some(value)) = (args.set, args.value) {
        println!("Setting {} = {}", key, value);
        // TODO: Implement config editing
        println!("{}", "Config editing not yet implemented".yellow());
    }

    Ok(())
}

async fn execute_tunnel(args: TunnelArgs, _verbose: bool) -> Result<()> {
    println!("{}", "🔒 WireGuard Tunnel Setup".blue().bold());
    println!("{}", "━".repeat(50).blue());
    println!();

    let wg_conf_path = format!("/etc/wireguard/{}.conf", args.interface);

    // Check if config already exists
    if std::path::Path::new(&wg_conf_path).exists() {
        println!("  {} WireGuard config already exists at {}", "⚠".yellow(), wg_conf_path);
        println!("  Delete it first if you want to re-provision.");
        println!();

        // Still try to extract info and update provider config
        let config_content = std::fs::read_to_string(&wg_conf_path)?;
        if let Some((public_ip, port_start, port_end)) = parse_wg_config(&config_content) {
            update_provider_tunnel_config(&args.interface, &public_ip, port_start, port_end)?;
        }
        return Ok(());
    }

    // 1. Ensure WireGuard is installed
    print!("  Checking WireGuard installation... ");
    let wg_check = Command::new("which").arg("wg-quick").output();
    match wg_check {
        Ok(o) if o.status.success() => {
            println!("{}", "OK".green());
        }
        _ => {
            println!("{}", "not found, installing...".yellow());
            let install = Command::new("apt-get")
                .args(["install", "-y", "wireguard", "wireguard-tools"])
                .env("DEBIAN_FRONTEND", "noninteractive")
                .output();
            match install {
                Ok(o) if o.status.success() => {
                    println!("  {} WireGuard installed", "✓".green());
                }
                _ => {
                    return Err(anyhow::anyhow!(
                        "Failed to install WireGuard. Install manually: apt install wireguard wireguard-tools"
                    ));
                }
            }
        }
    }

    // 2. Download WireGuard config from VPN service
    print!("  Requesting VPN config from {}... ", args.vpn_url);
    let client = reqwest::Client::new();
    let response = client.get(&args.vpn_url)
        .header("Authorization", format!("Cashu {}", args.token))
        .header("User-Agent", "Paygress-CLI/0.3.0")
        .send()
        .await?;

    if !response.status().is_success() {
        println!("{}", "FAILED".red());
        return Err(anyhow::anyhow!(
            "VPN service returned {}: {}",
            response.status(),
            response.text().await.unwrap_or_default()
        ));
    }

    let wg_config = response.text().await?;
    println!("{}", "OK".green());

    // 2. Validate config
    if !wg_config.contains("[Interface]") {
        println!("  {} Received invalid config (no [Interface] section)", "✗".red());
        return Err(anyhow::anyhow!("Invalid WireGuard config received from VPN service"));
    }
    println!("  {} Config validated", "✓".green());

    // 3. Save config
    std::fs::create_dir_all("/etc/wireguard")?;
    std::fs::write(&wg_conf_path, &wg_config)?;

    // Set permissions to 600
    Command::new("chmod")
        .args(["600", &wg_conf_path])
        .output()?;
    println!("  {} Saved to {}", "✓".green(), wg_conf_path);

    // 4. Parse tunnel details
    let (public_ip, port_start, port_end) = parse_wg_config(&wg_config)
        .ok_or_else(|| anyhow::anyhow!("Could not extract tunnel IP from WireGuard config"))?;

    println!("  {} Tunnel public IP: {}", "✓".green(), public_ip.cyan());
    if let (Some(ps), Some(pe)) = (port_start, port_end) {
        println!("  {} Port range: {}-{}", "✓".green(), ps, pe);
    }

    // 5. Start WireGuard interface
    print!("  Starting WireGuard interface {}... ", args.interface);
    let output = Command::new("wg-quick")
        .args(["up", &args.interface])
        .output()?;

    if output.status.success() {
        println!("{}", "UP".green());
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("already exists") {
            println!("{}", "ALREADY UP".yellow());
        } else {
            println!("{}", "FAILED".red());
            println!("  {}", stderr.trim());
            return Err(anyhow::anyhow!("Failed to start WireGuard interface"));
        }
    }

    // 6. Enable on boot
    let _ = Command::new("systemctl")
        .args(["enable", &format!("wg-quick@{}", args.interface)])
        .output();
    println!("  {} Enabled on boot", "✓".green());

    // 7. Update provider config
    update_provider_tunnel_config(&args.interface, &public_ip, port_start, port_end)?;

    println!();
    println!("{}", "━".repeat(50).blue());
    println!("{}", "🎉 Tunnel Active!".green().bold());
    println!();
    println!("  Public IP:  {}", public_ip.cyan());
    println!("  Interface:  {}", args.interface);
    if let (Some(ps), Some(pe)) = (port_start, port_end) {
        println!("  Port range: {}-{}", ps, pe);
    }
    println!();
    println!("  Your provider will now be reachable through the VPN tunnel.");
    println!("  Restart the provider service to apply: {} provider start", "paygress-cli".cyan());

    Ok(())
}

/// Parse WireGuard config to extract public IP and port range.
/// Returns (public_ip, optional_port_start, optional_port_end)
fn parse_wg_config(config: &str) -> Option<(String, Option<u16>, Option<u16>)> {
    // Extract public IP from Endpoint field (e.g., "Endpoint = 1.2.3.4:51820")
    let public_ip = config.lines()
        .find(|l| l.trim().starts_with("Endpoint"))
        .and_then(|l| l.split('=').nth(1))
        .map(|v| v.trim().split(':').next().unwrap_or("").to_string())
        .filter(|s| !s.is_empty())?;

    // Try to extract port range from comments (e.g., "# Public Ports: 1.2.3.4:11000-11999")
    let (port_start, port_end) = config.lines()
        .find(|l| l.contains("Public Ports:") || l.contains("Port Range:"))
        .and_then(|l| {
            // Extract "11000-11999" from the line
            let re_part = l.split(':').last()?;
            let range_str = re_part.trim().split(':').last()?.trim();
            let mut parts = range_str.split('-');
            let start: u16 = parts.next()?.trim().parse().ok()?;
            let end: u16 = parts.next()?.trim().parse().ok()?;
            Some((Some(start), Some(end)))
        })
        .unwrap_or((None, None));

    Some((public_ip, port_start, port_end))
}

/// Update provider config with tunnel settings
fn update_provider_tunnel_config(
    interface: &str,
    public_ip: &str,
    port_start: Option<u16>,
    port_end: Option<u16>,
) -> Result<()> {
    match load_config(CONFIG_PATH) {
        Ok(mut config) => {
            config.tunnel_enabled = true;
            config.tunnel_interface = Some(interface.to_string());
            config.public_ip = public_ip.to_string();
            config.ssh_port_start = port_start;
            config.ssh_port_end = port_end;
            save_config(CONFIG_PATH, &config)?;
            println!("  {} Provider config updated (public_ip={}, tunnel=enabled)", "✓".green(), public_ip);
        }
        Err(_) => {
            println!("  {} No provider config found at {}. Run 'provider setup' first.", "⚠".yellow(), CONFIG_PATH);
            println!("  Tunnel is active but provider config not updated.");
        }
    }
    Ok(())
}
