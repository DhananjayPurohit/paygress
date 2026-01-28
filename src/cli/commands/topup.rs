// Topup command - Extend pod lifetime with additional payment

use anyhow::Result;
use clap::Args;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};

use crate::api::{PaygressClient, TopupRequest};

#[derive(Args)]
pub struct TopupArgs {
    /// Pod ID to top up
    #[arg(short, long)]
    pub pod_id: String,

    /// Cashu token for payment
    #[arg(short = 'k', long)]
    pub token: String,
}

pub async fn execute(server: &str, args: TopupArgs, verbose: bool) -> Result<()> {
    if verbose {
        println!("{} Topping up pod...", "→".blue());
        println!("  Server: {}", server);
        println!("  Pod ID: {}", args.pod_id);
    }

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.blue} {msg}")
            .unwrap()
    );
    spinner.set_message("Processing top-up payment...");
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));

    let client = PaygressClient::new(server);

    let request = TopupRequest {
        pod_id: args.pod_id.clone(),
        cashu_token: Some(args.token),
    };

    let response = client.topup_pod(request).await?;
    spinner.finish_and_clear();

    if response.success {
        println!("{}", "✅ Pod topped up successfully!".green().bold());
        println!();
        
        if let Some(pod_id) = &response.pod_id {
            println!("  {} {}", "Pod ID:".bold(), pod_id);
        }
        if let Some(expires) = &response.new_expires_at {
            println!("  {} {}", "New Expiry:".bold(), expires);
        }
        if let Some(added) = response.added_seconds {
            let minutes = added / 60;
            let seconds = added % 60;
            println!("  {} +{}m {}s", "Added:".bold(), minutes, seconds);
        }
        if let Some(msg) = &response.message {
            println!("  {} {}", "Message:".bold(), msg);
        }
    } else {
        let error_msg = response.error.unwrap_or_else(|| "Unknown error".to_string());
        return Err(anyhow::anyhow!("Failed to top up pod: {}", error_msg));
    }

    Ok(())
}
