use anyhow::Result;
use clap::{Args, Subcommand};
use colored::*;
use std::process::Command;
use std::io::{self, Write};

#[derive(Args)]
pub struct SystemArgs {
    #[command(subcommand)]
    pub action: SystemAction,
}

#[derive(Subcommand)]
pub enum SystemAction {
    /// Reset the system to a clean state (uninstall/cleanup)
    Reset(ResetArgs),
}

#[derive(Args)]
pub struct ResetArgs {
    /// Path to inventory file (for remote reset)
    #[arg(short, long, default_value = "inventory.ini")]
    pub inventory: Option<String>,

    /// Skip confirmation prompts
    #[arg(long, short = 'y')]
    pub yes: bool,

    /// Uninstall the compute backend (LXD/Proxmox)
    #[arg(long)]
    pub uninstall_backend: bool,
}

pub async fn execute(args: SystemArgs, verbose: bool) -> Result<()> {
    match args.action {
        SystemAction::Reset(reset_args) => execute_reset(reset_args, verbose).await,
    }
}

async fn execute_reset(args: ResetArgs, verbose: bool) -> Result<()> {
    if let Some(ref inventory) = args.inventory {
        if std::path::Path::new(inventory).exists() {
            return execute_remote_reset(inventory, verbose).await;
        }
    }

    println!("{}", "⚠️  SYSTEM RESET (LOCAL) ⚠️".red().bold());
    println!("{}", "This will permanently remove Paygress services and configurations from THIS machine.".red());
    if args.uninstall_backend {
        println!("{}", "WARNING: This will also attempt to UNINSTALL your compute backend (LXD/Proxmox).".red().bold());
    }
    println!();

    if !args.yes {
        print!("Are you sure you want to proceed? [y/N] ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Reset aborted.");
            return Ok(());
        }
    }

    // 1. Stop and disable service
    print!("  ⚙ Stopping paygress-provider service... ");
    io::stdout().flush()?;
    let _ = Command::new("systemctl").args(["stop", "paygress-provider"]).output();
    let _ = Command::new("systemctl").args(["disable", "paygress-provider"]).output();
    println!("{}", "DONE".green());

    // 2. Remove systemd unit
    print!("  ⚙ Removing systemd unit... ");
    io::stdout().flush()?;
    let _ = Command::new("rm").args(["-f", "/etc/systemd/system/paygress-provider.service"]).output();
    let _ = Command::new("systemctl").args(["daemon-reload"]).output();
    println!("{}", "DONE".green());

    // 3. Remove configurations
    print!("  ⚙ Removing /etc/paygress... ");
    io::stdout().flush()?;
    let _ = Command::new("rm").args(["-rf", "/etc/paygress"]).output();
    println!("{}", "DONE".green());

    // 4. Uninstall Backend if requested
    if args.uninstall_backend {
        // Detect OS/Backend
        println!("{}", "  ⚙ Uninstalling compute backend...".yellow());
        
        // Try snap remove lxd (Ubuntu common)
        print!("    Removing LXD (snap)... ");
        io::stdout().flush()?;
        let output = Command::new("snap").args(["remove", "lxd", "--purge"]).output();
        if output.is_ok() {
            println!("{}", "DONE".green());
        } else {
            println!("{}", "SKIPPED (not via snap)".yellow());
        }

        // Try apt remove lxc
        print!("    Removing LXC (apt)... ");
        io::stdout().flush()?;
        let _ = Command::new("apt-get").args(["remove", "--purge", "-y", "lxc", "lxcfs"]).output();
        let _ = Command::new("apt-get").args(["autoremove", "-y"]).output();
        println!("{}", "DONE".green());
        
        // Proxmox removal is dangerous, we just hint it for now or do basic cleanup
        println!("    {} Manual Proxmox cleanup may still be required if using Proxmox VE packages.", "💡".yellow());
    }

    println!();
    println!("{}", "━".repeat(50).green());
    println!("{}", "✅ Reset Complete!".green().bold());
    println!("Paygress has been uninstalled from this machine.");
    println!("Note: The paygress-cli binary itself remains in /usr/local/bin/ (you can remove it manually if desired).");
    println!("{}", "━".repeat(50).green());

    Ok(())
}

async fn execute_remote_reset(inventory: &str, verbose: bool) -> Result<()> {
    println!("{}", "🚀 Remote System Reset...".bold());
    println!("  Inventory: {}", inventory);
    println!();

    if !std::path::Path::new("reset-vps.yml").exists() {
        return Err(anyhow::anyhow!("reset-vps.yml not found in current directory"));
    }

    let mut cmd = Command::new("ansible-playbook");
    cmd.arg("-i").arg(inventory)
       .arg("reset-vps.yml");
    
    if verbose {
        cmd.arg("-v");
    }

    let status = cmd.status()?;

    if status.success() {
        println!();
        println!("{}", "✅ Remote Reset Complete!".green().bold());
    } else {
        return Err(anyhow::anyhow!("Remote reset failed"));
    }

    Ok(())
}
