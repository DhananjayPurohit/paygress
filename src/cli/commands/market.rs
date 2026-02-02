// Market CLI Commands
//
// Commands for end users to discover providers and spawn workloads.

use anyhow::Result;
use clap::{Args, Subcommand};
use colored::Colorize;
use std::io::Write;
use std::path::Path;
use nostr_sdk::{Keys, ToBech32}; // Ensure nostr_sdk is available or use full path

use paygress::discovery::DiscoveryClient;
use paygress::nostr::{
    ProviderFilter, EncryptedSpawnPodRequest, AccessDetailsContent, ErrorResponseContent,
};

const DEFAULT_RELAYS: &[&str] = &[
    "wss://relay.damus.io",
    "wss://nos.lol",
    "wss://relay.primal.net",
];

#[derive(Args)]
pub struct MarketArgs {
    #[command(subcommand)]
    pub action: MarketAction,
}

#[derive(Subcommand)]
pub enum MarketAction {
    /// List available providers
    List(ListArgs),
    
    /// Show details of a specific provider
    Info(InfoArgs),
    
    /// Spawn a VM or container on a provider
    Spawn(SpawnArgs),
}

#[derive(Args)]
pub struct ListArgs {
    /// Filter by capability (lxc, vm)
    #[arg(long)]
    pub capability: Option<String>,
    
    /// Minimum uptime percentage
    #[arg(long)]
    pub min_uptime: Option<f32>,
    
    /// Sort by (price, uptime, capacity, jobs)
    #[arg(long, default_value = "price")]
    pub sort: String,
    
    /// Only show online providers
    #[arg(long)]
    pub online_only: bool,
    
    /// Custom Nostr relays (comma-separated)
    #[arg(long)]
    pub relays: Option<String>,
}

#[derive(Args)]
pub struct InfoArgs {
    /// Provider npub
    #[arg(long)]
    pub provider: String,
    
    /// Custom Nostr relays (comma-separated)
    #[arg(long)]
    pub relays: Option<String>,
}

#[derive(Args)]
pub struct SpawnArgs {
    /// Provider npub to use
    #[arg(long)]
    pub provider: String,
    
    /// Workload type: "lxc" or "vm"
    #[arg(long, default_value = "lxc")]
    pub workload_type: String,
    
    /// Spec tier ID (e.g., "basic", "standard", "premium")
    #[arg(long, default_value = "basic")]
    pub tier: String,
    
    /// Cashu token for payment
    #[arg(long)]
    pub token: String,
    
    /// Container image (for K8s mode, ignored for Proxmox)
    #[arg(long, default_value = "ubuntu:22.04")]
    pub image: Option<String>,
    
    /// SSH username
    #[arg(long, default_value = "user")]
    pub ssh_user: String,
    
    /// SSH password
    #[arg(long)]
    pub ssh_pass: String,
    
    /// Your Nostr private key (nsec) for encrypted communication (optional - will use ~/.paygress/identity if not provided)
    #[arg(long)]
    pub nostr_key: Option<String>,
    
    /// Custom Nostr relays (comma-separated)
    #[arg(long)]
    pub relays: Option<String>,
}

pub async fn execute(args: MarketArgs, verbose: bool) -> Result<()> {
    match args.action {
        MarketAction::List(list_args) => execute_list(list_args, verbose).await,
        MarketAction::Info(info_args) => execute_info(info_args, verbose).await,
        MarketAction::Spawn(spawn_args) => execute_spawn(spawn_args, verbose).await,
    }
}

fn parse_relays(relays: Option<String>) -> Vec<String> {
    match relays {
        Some(r) => r.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        None => DEFAULT_RELAYS.iter().map(|s| s.to_string()).collect(),
    }
}

fn get_or_create_identity(explicit_key: Option<String>) -> Result<String> {
    if let Some(key) = explicit_key {
        return Ok(key);
    }

    let home = std::env::var("HOME").map_err(|_| anyhow::anyhow!("Could not determine home directory"))?;
    let paygress_dir = Path::new(&home).join(".paygress");
    if !paygress_dir.exists() {
        std::fs::create_dir_all(&paygress_dir)?;
    }
    
    let identity_file = paygress_dir.join("identity");
    if identity_file.exists() {
        let key = std::fs::read_to_string(&identity_file)?.trim().to_string();
        println!("  Using identity from {}", identity_file.display().to_string().dimmed());
        return Ok(key);
    }
    
    // Generate new key
    println!("{}", "  ℹ No identity found. Generating new Nostr identity...".yellow());
    let keys = Keys::generate();
    let nsec = keys.secret_key()?.to_bech32()?;
    
    // Save to file
    let mut file = std::fs::File::create(&identity_file)?;
    file.write_all(nsec.as_bytes())?;
    
    // Set permissions to 600 (owner read/write only) on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = file.metadata()?.permissions();
        perms.set_mode(0o600);
        file.set_permissions(perms)?;
    }
    
    println!("  {} Created new identity at {}", "✓".green(), identity_file.display());
    println!("  {} {}", "NSEC:".bold(), nsec.red());
    println!("  {}", "Make sure to back up this key!".yellow());
    println!();
    
    Ok(nsec)
}

async fn execute_list(args: ListArgs, verbose: bool) -> Result<()> {
    println!("{}", "🔍 Discovering Providers...".blue().bold());
    println!();

    let relays = parse_relays(args.relays);
    
    if verbose {
        println!("  Connecting to {} relays...", relays.len());
    }
    
    let client = DiscoveryClient::new(relays).await?;

    // Build filter
    let filter = ProviderFilter {
        capability: args.capability,
        min_uptime: args.min_uptime,
        min_memory_mb: None,
        min_cpu: None,
    };

    let mut providers = client.list_providers(Some(filter)).await?;

    // Filter online only if requested
    if args.online_only {
        providers.retain(|p| p.is_online);
    }

    // Sort providers
    DiscoveryClient::sort_providers(&mut providers, &args.sort);

    if providers.is_empty() {
        println!("{}", "No providers found matching your criteria.".yellow());
        println!();
        println!("Try:");
        println!("  • Removing filters");
        println!("  • Checking different relays");
        return Ok(());
    }

    println!("Found {} providers:\n", providers.len().to_string().green());
    println!("{}", DiscoveryClient::format_provider_table(&providers));
    
    println!();
    println!("To see details: {} market info --provider <npub>", "paygress-cli".cyan());
    println!("To spawn:       {} market spawn --provider <npub> --token <cashu-token>", "paygress-cli".cyan());

    Ok(())
}

async fn execute_info(args: InfoArgs, _verbose: bool) -> Result<()> {
    println!("{}", "📋 Provider Details".blue().bold());
    println!();

    let relays = parse_relays(args.relays);
    let client = DiscoveryClient::new(relays).await?;

    match client.get_provider(&args.provider).await? {
        Some(provider) => {
            println!("{}", DiscoveryClient::format_provider_details(&provider));
            
            // Show spawn command example
            println!();
            println!("To spawn on this provider:");
            println!("  {} market spawn \\", "paygress-cli".cyan());
            println!("    --provider {} \\", args.provider);
            println!("    --tier basic \\");
            println!("    --token <your-cashu-token> \\");
            println!("    --ssh-pass <password> \\");
            println!("    --nostr-key <your-nsec>");
        }
        None => {
            println!("{}", "Provider not found.".red());
            println!();
            println!("Make sure the NPUB is correct and the provider is online.");
        }
    }

    Ok(())
}

async fn execute_spawn(args: SpawnArgs, verbose: bool) -> Result<()> {
    println!("{}", "🚀 Spawning Workload".blue().bold());
    println!("{}", "━".repeat(50).blue());
    println!();

    let relays = parse_relays(args.relays);
    
    // Get or create identity
    let nostr_key = get_or_create_identity(args.nostr_key)?;
    
    // Create discovery client with user's key for encrypted messaging
    let client = DiscoveryClient::new_with_key(relays.clone(), nostr_key).await?;
    
    println!("  Your NPUB: {}", client.get_npub().cyan());
    println!();

    // Check if provider is online
    print!("  {} Checking provider status... ", "⚙".yellow());
    if !client.is_provider_online(&args.provider).await {
        println!("{}", "OFFLINE".red());
        println!();
        println!("{}", "Provider appears to be offline.".red());
        println!("Try a different provider or wait for this one to come online.");
        return Ok(());
    }
    println!("{}", "ONLINE".green());

    // Get provider info
    let provider = client.get_provider(&args.provider).await?
        .ok_or_else(|| anyhow::anyhow!("Provider not found"))?;

    // Verify tier exists
    let spec = provider.specs.iter()
        .find(|s| s.id == args.tier)
        .ok_or_else(|| anyhow::anyhow!("Tier '{}' not available on this provider", args.tier))?;

    println!("  {} Found tier: {} ({} msat/sec)", "✓".green(), spec.name, spec.rate_msats_per_sec);

    // Build spawn request
    let request = EncryptedSpawnPodRequest {
        cashu_token: args.token.clone(),
        pod_spec_id: Some(args.tier.clone()),
        pod_image: args.image.unwrap_or_else(|| "ubuntu:22.04".to_string()),
        ssh_username: args.ssh_user.clone(),
        ssh_password: args.ssh_pass.clone(),
    };

    println!();
    print!("  {} Sending spawn request... ", "⚙".yellow());

    let request_json = serde_json::to_string(&request)?;
    let _event_id = client.nostr().send_encrypted_private_message(
        &provider.npub,
        request_json,
        "nip04",
    ).await?;

    // Actually send spawn request
    // Note: We need to send the actual request, not access details
    // This is a simplified version - real implementation would use proper request flow
    
    println!("{}", "SENT".green());
    println!();
    
    println!("  {} Waiting for provider response (timeout: 60s)...", "⏳".yellow());
    
    // Wait for response from provider
    match client.nostr().wait_for_decrypted_message(&provider.npub, 60).await {
        Ok(response) => {
            println!();
            println!("{}", "━".repeat(50).blue());
            
            // Try to parse as AccessDetailsContent
            if let Ok(access) = serde_json::from_str::<AccessDetailsContent>(&response.content) {
                println!("{}", "🎉 Workload Provisioned Successfully!".green().bold());
                println!();
                println!("  {}   {}", "Pod ID:".bold(), access.pod_npub.cyan());
                println!("  {}   {}", "Expires:".bold(), access.expires_at.yellow());
                println!("  {}   {} vCPU, {} MB RAM", "Spec:".bold(), access.cpu_millicores / 1000, access.memory_mb);
                println!();
                println!("{}", "Connection Instructions:".bold());
                for inst in access.instructions {
                    println!("  • {}", inst);
                }
            } 
            // Try to parse as ErrorResponseContent
            else if let Ok(err) = serde_json::from_str::<ErrorResponseContent>(&response.content) {
                println!("{}", "❌ Provider Error".red().bold());
                println!();
                println!("  Type:    {}", err.error_type);
                println!("  Message: {}", err.message);
                if let Some(details) = err.details {
                    println!("  Details: {}", details);
                }
            } 
            // Unknown response
            else {
                println!("{}", "❓ Received Unknown Response".yellow().bold());
                println!();
                println!("Content: {}", response.content);
            }
        }
        Err(e) => {
            println!();
            println!("{}", "━".repeat(50).blue());
            println!("  {} {}", "⚠️".yellow(), e.to_string().yellow());
            println!();
            println!("The request was sent, but the provider didn't respond in time.");
            println!("You may check your status later with event ID: {}", _event_id);
        }
    }

    Ok(())
}
