// Paygress CLI - Command Line Interface
//
// A unified CLI tool for both API interaction and server management.

use clap::{Parser, Subcommand};
use colored::Colorize;

mod api;
mod commands;

use commands::{spawn, topup, status, offers, deploy, service, fix};

/// Paygress CLI - Cashu Payment Gateway for Kubernetes Pod Provisioning
#[derive(Parser)]
#[command(name = "paygress-cli")]
#[command(author = "Dhananjay Purohit")]
#[command(version = "0.1.0")]
#[command(about = "CLI tool for Paygress - spawn pods with Cashu payments", long_about = None)]
struct Cli {
    /// Paygress server URL
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
    // ============ API Commands (End Users) ============
    
    /// Spawn a new pod with Cashu payment
    Spawn(spawn::SpawnArgs),

    /// Top up an existing pod with additional payment
    Topup(topup::TopupArgs),

    /// Get status of a pod
    Status(status::StatusArgs),

    /// List available pod offers/tiers
    Offers(offers::OffersArgs),

    // ============ Management Commands (Server Operators) ============
    
    /// Deploy Paygress to a server
    Deploy(deploy::DeployArgs),

    /// Service management (status, logs, restart)
    Service(service::ServiceArgs),

    /// Fix issues (Kubernetes, pods)
    Fix(fix::FixArgs),
}

fn print_banner() {
    println!("{}", "╔════════════════════════════════════════════════════════════╗".blue());
    println!("{}", "║                    PAYGRESS CLI                            ║".blue());
    println!("{}", "║  Cashu Payment Gateway for Kubernetes Pod Provisioning    ║".blue());
    println!("{}", "╚════════════════════════════════════════════════════════════╝".blue());
    println!();
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if cli.verbose {
        print_banner();
    }

    let result = match cli.command {
        // API Commands
        Commands::Spawn(args) => spawn::execute(&cli.server, args, cli.verbose).await,
        Commands::Topup(args) => topup::execute(&cli.server, args, cli.verbose).await,
        Commands::Status(args) => status::execute(&cli.server, args, cli.verbose).await,
        Commands::Offers(args) => offers::execute(&cli.server, args, cli.verbose).await,
        
        // Management Commands
        Commands::Deploy(args) => deploy::execute(args, cli.verbose).await,
        Commands::Service(args) => service::execute(args, cli.verbose).await,
        Commands::Fix(args) => fix::execute(args, cli.verbose).await,
    };

    if let Err(e) = result {
        eprintln!("{} {}", "Error:".red().bold(), e);
        std::process::exit(1);
    }
}
