// Spawn command - Create a new pod with Cashu payment

use anyhow::Result;
use clap::Args;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};

use crate::api::{PaygressClient, SpawnRequest};

#[derive(Args)]
pub struct SpawnArgs {
    /// Pod tier/specification ID (e.g., basic, standard, premium)
    #[arg(short, long)]
    pub tier: String,

    /// Cashu token for payment (required)
    #[arg(short = 'k', long)]
    pub token: String,

    /// Container image to use
    #[arg(short, long, default_value = "linuxserver/openssh-server:latest")]
    pub image: String,

    /// SSH username for the pod
    #[arg(short = 'u', long, default_value = "user")]
    pub ssh_user: String,

    /// SSH password for the pod
    #[arg(short = 'p', long, default_value = "password")]
    pub ssh_pass: String,
}

pub async fn execute(server: &str, args: SpawnArgs, verbose: bool) -> Result<()> {
    if verbose {
        println!("{} Spawning pod...", "→".blue());
        println!("  Server: {}", server);
        println!("  Tier: {}", args.tier);
        println!("  Image: {}", args.image);
    }

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.blue} {msg}")
            .unwrap()
    );
    spinner.set_message("Connecting to Paygress server...");
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));

    let client = PaygressClient::new(server);

    // First check server health
    spinner.set_message("Checking server health...");
    match client.health().await {
        Ok(_) => {
            if verbose {
                spinner.set_message("Server is healthy");
            }
        }
        Err(e) => {
            spinner.finish_and_clear();
            return Err(anyhow::anyhow!("Server health check failed: {}", e));
        }
    }

    // Spawn the pod
    spinner.set_message("Spawning pod with Cashu payment...");
    
    let request = SpawnRequest {
        pod_spec_id: args.tier,
        pod_image: args.image,
        ssh_username: args.ssh_user,
        ssh_password: args.ssh_pass,
        cashu_token: Some(args.token),
    };

    let response = client.spawn_pod(request).await?;
    spinner.finish_and_clear();

    if response.success {
        println!("{}", "✅ Pod spawned successfully!".green().bold());
        println!();
        
        if let Some(pod_id) = &response.pod_id {
            println!("  {} {}", "Pod ID:".bold(), pod_id);
        }
        if let Some(host) = &response.ssh_host {
            if let Some(port) = response.ssh_port {
                println!("  {} ssh {}@{} -p {}", 
                    "SSH:".bold(), 
                    response.ssh_username.as_deref().unwrap_or("user"),
                    host, 
                    port
                );
            }
        }
        if let Some(expires) = &response.expires_at {
            println!("  {} {}", "Expires:".bold(), expires);
        }
        if let Some(duration) = response.duration_seconds {
            let minutes = duration / 60;
            let seconds = duration % 60;
            println!("  {} {}m {}s", "Duration:".bold(), minutes, seconds);
        }
        
        println!();
        println!("{}", "💡 Tip: Use 'paygress-cli status --pod-id <POD_ID>' to check status".dimmed());
        println!("{}", "💡 Tip: Use 'paygress-cli topup --pod-id <POD_ID> --token <TOKEN>' to extend".dimmed());
    } else {
        let error_msg = response.error.unwrap_or_else(|| "Unknown error".to_string());
        return Err(anyhow::anyhow!("Failed to spawn pod: {}", error_msg));
    }

    Ok(())
}
