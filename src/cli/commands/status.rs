// Status command - Get pod status

use anyhow::Result;
use clap::Args;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};

use crate::api::PaygressClient;

#[derive(Args)]
pub struct StatusArgs {
    /// Pod ID to check status
    #[arg(short, long)]
    pub pod_id: String,
}

pub async fn execute(server: &str, args: StatusArgs, verbose: bool) -> Result<()> {
    if verbose {
        println!("{} Checking pod status...", "→".blue());
        println!("  Server: {}", server);
        println!("  Pod ID: {}", args.pod_id);
    }

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.blue} {msg}")
            .unwrap()
    );
    spinner.set_message("Fetching pod status...");
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));

    let client = PaygressClient::new(server);
    let response = client.get_pod_status(&args.pod_id).await?;
    spinner.finish_and_clear();

    if response.success {
        println!("{}", "📊 Pod Status".bold());
        println!();
        
        if let Some(pod_id) = &response.pod_id {
            println!("  {} {}", "Pod ID:".bold(), pod_id);
        }
        
        if let Some(status) = &response.status {
            let status_colored = match status.as_str() {
                "Running" => status.green().to_string(),
                "Pending" => status.yellow().to_string(),
                "Failed" | "Error" => status.red().to_string(),
                "Terminated" | "Expired" => status.dimmed().to_string(),
                _ => status.clone(),
            };
            println!("  {} {}", "Status:".bold(), status_colored);
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
        
        if let Some(remaining) = response.time_remaining_seconds {
            if remaining > 0 {
                let hours = remaining / 3600;
                let minutes = (remaining % 3600) / 60;
                let seconds = remaining % 60;
                
                let time_str = if hours > 0 {
                    format!("{}h {}m {}s", hours, minutes, seconds)
                } else if minutes > 0 {
                    format!("{}m {}s", minutes, seconds)
                } else {
                    format!("{}s", seconds)
                };
                
                let time_colored = if remaining < 300 {
                    time_str.red().to_string()
                } else if remaining < 600 {
                    time_str.yellow().to_string()
                } else {
                    time_str.green().to_string()
                };
                
                println!("  {} {}", "Remaining:".bold(), time_colored);
            } else {
                println!("  {} {}", "Remaining:".bold(), "Expired".red());
            }
        }
    } else {
        let error_msg = response.error.unwrap_or_else(|| "Unknown error".to_string());
        return Err(anyhow::anyhow!("Failed to get pod status: {}", error_msg));
    }

    Ok(())
}
