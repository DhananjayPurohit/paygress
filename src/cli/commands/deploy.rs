// Deploy command - Deploy Paygress to server

use anyhow::Result;
use clap::Args;
use colored::Colorize;
use std::process::Command;

#[derive(Args)]
pub struct DeployArgs {
    /// Path to inventory file
    #[arg(short, long, default_value = "inventory.ini")]
    pub inventory: String,

    /// Skip Ansible installation check
    #[arg(long)]
    pub skip_ansible_check: bool,
}

pub async fn execute(args: DeployArgs, verbose: bool) -> Result<()> {
    println!("{}", "🚀 Deploying Paygress...".bold());
    println!();

    // Check if inventory file exists
    if !std::path::Path::new(&args.inventory).exists() {
        return Err(anyhow::anyhow!(
            "Inventory file '{}' not found. Create it from inventory.ini.template",
            args.inventory
        ));
    }

    if verbose {
        println!("  Inventory: {}", args.inventory);
    }

    // Check for ansible-playbook
    if !args.skip_ansible_check {
        print!("  Checking Ansible installation... ");
        let ansible_check = Command::new("which")
            .arg("ansible-playbook")
            .output();

        match ansible_check {
            Ok(output) if output.status.success() => {
                println!("{}", "✓".green());
            }
            _ => {
                println!("{}", "✗".red());
                println!();
                
                #[cfg(target_os = "macos")]
                {
                    println!("  {} Ansible not found. Install with:", "→".yellow());
                    println!("    {}", "brew install ansible".cyan());
                }
                
                #[cfg(not(target_os = "macos"))]
                {
                    println!("  {} Ansible not found. Install with:", "→".yellow());
                    println!("    {}", "sudo apt install ansible".cyan());
                }
                
                return Err(anyhow::anyhow!("Ansible is required for deployment"));
            }
        }
    }

    // Check for ansible-setup.yml
    if !std::path::Path::new("ansible-setup.yml").exists() {
        return Err(anyhow::anyhow!(
            "ansible-setup.yml not found in current directory"
        ));
    }

    println!("  Running Ansible playbook...");
    println!();

    // Run ansible-playbook
    let mut cmd = Command::new("ansible-playbook");
    cmd.arg("-i").arg(&args.inventory)
       .arg("ansible-setup.yml");
    
    if verbose {
        cmd.arg("-v");
    }

    let status = cmd.status()?;

    println!();
    
    if status.success() {
        println!("{}", "╔════════════════════════════════════════════════════════════╗".green());
        println!("{}", "║              🎉 DEPLOYMENT COMPLETE! 🎉                   ║".green());
        println!("{}", "╚════════════════════════════════════════════════════════════╝".green());
        println!();
        println!("  {} Check status: {}", "→".blue(), "paygress-cli service status".cyan());
        println!("  {} View logs:    {}", "→".blue(), "paygress-cli service logs".cyan());
        println!("  {} Test API:     {}", "→".blue(), "paygress-cli offers -s http://<SERVER>:11000".cyan());
    } else {
        println!("{}", "╔════════════════════════════════════════════════════════════╗".yellow());
        println!("{}", "║           Deployment completed with warnings              ║".yellow());
        println!("{}", "╚════════════════════════════════════════════════════════════╝".yellow());
        println!();
        println!("  {} Try fixing: {}", "→".blue(), "paygress-cli fix kubernetes".cyan());
    }

    Ok(())
}
