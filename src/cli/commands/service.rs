// Service command - Manage Paygress service

use anyhow::Result;
use clap::{Args, Subcommand};
use colored::Colorize;
use std::process::Command;

#[derive(Args)]
pub struct ServiceArgs {
    #[command(subcommand)]
    pub command: ServiceCommand,

    /// Server SSH address (user@host)
    #[arg(short, long, global = true)]
    pub target: Option<String>,

    /// SSH port
    #[arg(short, long, default_value = "22", global = true)]
    pub port: u16,
}

#[derive(Subcommand)]
pub enum ServiceCommand {
    /// Check service status
    Status,
    
    /// View service logs
    Logs {
        /// Follow logs in real-time
        #[arg(short, long)]
        follow: bool,
        
        /// Number of lines to show
        #[arg(short, long, default_value = "50")]
        lines: u32,
    },
    
    /// Restart the service
    Restart,
}

pub async fn execute(args: ServiceArgs, verbose: bool) -> Result<()> {
    let target = args.target.clone().unwrap_or_else(|| get_target_from_inventory());
    
    if target.is_empty() {
        return Err(anyhow::anyhow!(
            "No target specified. Use --target user@host or configure inventory.ini"
        ));
    }

    if verbose {
        println!("{} Target: {}", "→".blue(), target);
    }

    match args.command {
        ServiceCommand::Status => {
            println!("{}", "📊 Checking Paygress service status...".bold());
            println!();
            
            run_ssh_command(&target, args.port, "sudo systemctl status paygress --no-pager")?;
            
            println!();
            println!("{}", "━━━ Kubernetes Status ━━━".blue());
            run_ssh_command(&target, args.port, "kubectl get nodes 2>/dev/null || echo 'Kubernetes not accessible'")?;
            
            println!();
            run_ssh_command(&target, args.port, "kubectl get pods -n user-workloads 2>/dev/null || echo 'No pods running'")?;
        }
        
        ServiceCommand::Logs { follow, lines } => {
            println!("{}", "📜 Viewing Paygress logs...".bold());
            if follow {
                println!("{}", "Press Ctrl+C to stop".dimmed());
            }
            println!();
            
            let cmd = if follow {
                "sudo journalctl -u paygress -f".to_string()
            } else {
                format!("sudo journalctl -u paygress -n {} --no-pager", lines)
            };
            
            run_ssh_command(&target, args.port, &cmd)?;
        }
        
        ServiceCommand::Restart => {
            println!("{}", "🔄 Restarting Paygress service...".bold());
            println!();
            
            run_ssh_command(&target, args.port, "sudo systemctl restart paygress")?;
            
            // Wait a moment then show status
            std::thread::sleep(std::time::Duration::from_secs(2));
            
            run_ssh_command(&target, args.port, "sudo systemctl status paygress --no-pager")?;
            
            println!();
            println!("{}", "✅ Service restarted".green());
        }
    }

    Ok(())
}

fn get_target_from_inventory() -> String {
    // Try to read from inventory.ini
    if let Ok(content) = std::fs::read_to_string("inventory.ini") {
        for line in content.lines() {
            if line.contains("ansible_host=") && line.contains("ansible_user=") {
                // Parse: production ansible_host=1.2.3.4 ansible_user=root ...
                let mut host = String::new();
                let mut user = String::new();
                
                for part in line.split_whitespace() {
                    if part.starts_with("ansible_host=") {
                        host = part.replace("ansible_host=", "");
                    } else if part.starts_with("ansible_user=") {
                        user = part.replace("ansible_user=", "");
                    }
                }
                
                if !host.is_empty() && !user.is_empty() {
                    return format!("{}@{}", user, host);
                }
            }
        }
    }
    String::new()
}

fn run_ssh_command(target: &str, port: u16, command: &str) -> Result<()> {
    let status = Command::new("ssh")
        .arg("-p").arg(port.to_string())
        .arg("-o").arg("StrictHostKeyChecking=no")
        .arg(target)
        .arg(command)
        .status()?;

    if !status.success() {
        // Don't fail on SSH command errors, just show the output
        // The command might return non-zero for valid reasons
    }

    Ok(())
}
