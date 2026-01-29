// Paygress CLI - Command Line Interface
//
// A unified CLI tool for both API interaction and server management.

use clap::{Parser, Subcommand};
use colored::Colorize;

mod api;
mod commands;

use commands::{spawn, topup, status, offers, deploy, service, fix, provider, market, bootstrap, system};

/// Paygress CLI - Cashu Payment Gateway for Compute Provisioning
#[derive(Parser)]
#[command(name = "paygress-cli")]
#[command(author = "Dhananjay Purohit")]
#[command(version = "0.2.0")]
#[command(about = "CLI tool for Paygress - spawn compute with Cashu payments", long_about = None)]
struct Cli {
    /// Paygress server URL (for K8s mode)
    #[arg(short, long, default_value = "http://localhost:8080", global = true)]
    server: String,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    // ============ Marketplace Commands (End Users - Proxmox Mode) ============
    
    /// Discover and use compute providers (Proxmox mode)
    Market(market::MarketArgs),

    // ============ Provider Commands (Machine Operators - Proxmox Mode) ============
    
    /// Provider management - setup, start, stop (Proxmox mode)
    Provider(provider::ProviderArgs),
    
    /// One-click bootstrap - install Proxmox + Paygress on a server
    Bootstrap(bootstrap::BootstrapArgs),

    /// System management - reset, clean up environment
    System(system::SystemArgs),

    // ============ API Commands (End Users - K8s Mode) ============
    
    /// Spawn a new pod with Cashu payment (K8s mode)
    Spawn(spawn::SpawnArgs),

    /// Top up an existing pod with additional payment
    Topup(topup::TopupArgs),

    /// Get status of a pod
    Status(status::StatusArgs),

    /// List available pod offers/tiers
    Offers(offers::OffersArgs),

    // ============ Management Commands (Server Operators - K8s Mode) ============
    
    /// Deploy Paygress to a server (K8s mode)
    Deploy(deploy::DeployArgs),

    /// Service management (status, logs, restart)
    Service(service::ServiceArgs),

    /// Fix issues (Kubernetes, pods)
    Fix(fix::FixArgs),
}

fn print_banner() {
    println!("{}", "╔════════════════════════════════════════════════════════════╗".blue());
    println!("{}", "║                    PAYGRESS CLI                            ║".blue());
    println!("{}", "║     Pay-per-Use Compute with Cashu + Nostr                 ║".blue());
    println!("{}", "╚════════════════════════════════════════════════════════════╝".blue());
    println!();
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")))
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();

    if cli.verbose {
        print_banner();
    }

    let result = match cli.command {
        // Proxmox Mode - Marketplace
        Commands::Market(args) => market::execute(args, cli.verbose).await,
        
        // Proxmox Mode - Provider
        Commands::Provider(args) => provider::execute(args, cli.verbose).await,
        Commands::Bootstrap(args) => bootstrap::execute(args, cli.verbose).await,
        Commands::System(args) => system::execute(args, cli.verbose).await,
        
        // K8s Mode - API Commands
        Commands::Spawn(args) => spawn::execute(&cli.server, args, cli.verbose).await,
        Commands::Topup(args) => topup::execute(&cli.server, args, cli.verbose).await,
        Commands::Status(args) => status::execute(&cli.server, args, cli.verbose).await,
        Commands::Offers(args) => offers::execute(&cli.server, args, cli.verbose).await,
        
        // K8s Mode - Management Commands
        Commands::Deploy(args) => deploy::execute(args, cli.verbose).await,
        Commands::Service(args) => service::execute(args, cli.verbose).await,
        Commands::Fix(args) => fix::execute(args, cli.verbose).await,
    };

    if let Err(e) = result {
        eprintln!("{} {}", "Error:".red().bold(), e);
        std::process::exit(1);
    }
}
