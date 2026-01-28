// Offers command - List available pod tiers

use anyhow::Result;
use clap::Args;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};

use crate::api::PaygressClient;

#[derive(Args)]
pub struct OffersArgs {
    /// Output format (table, json)
    #[arg(short, long, default_value = "table")]
    pub format: String,
}

pub async fn execute(server: &str, args: OffersArgs, verbose: bool) -> Result<()> {
    if verbose {
        println!("{} Fetching offers...", "→".blue());
        println!("  Server: {}", server);
    }

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.blue} {msg}")
            .unwrap()
    );
    spinner.set_message("Fetching available offers...");
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));

    let client = PaygressClient::new(server);
    let response = client.get_offers().await?;
    spinner.finish_and_clear();

    if !response.success {
        let error_msg = response.error.unwrap_or_else(|| "Unknown error".to_string());
        return Err(anyhow::anyhow!("Failed to get offers: {}", error_msg));
    }

    if args.format == "json" {
        println!("{}", serde_json::to_string_pretty(&response.offers)?);
        return Ok(());
    }

    // Table format
    println!("{}", "📦 Available Pod Tiers".bold());
    println!();

    if let Some(offers) = response.offers {
        if offers.is_empty() {
            println!("{}", "  No offers available".dimmed());
        } else {
            // Print header
            println!("  {:<12} {:<20} {:<10} {:<10} {:>15}", 
                "ID".bold().underline(),
                "Name".bold().underline(),
                "CPU".bold().underline(),
                "RAM".bold().underline(),
                "Rate".bold().underline()
            );
            println!();

            for offer in offers {
                let rate_display = format!("{} msats/sec", offer.rate_msats_per_sec);
                let cpu_display = format!("{} cores", offer.cpu_millicores / 1000);
                let ram_display = if offer.memory_mb >= 1024 {
                    format!("{} GB", offer.memory_mb / 1024)
                } else {
                    format!("{} MB", offer.memory_mb)
                };

                println!("  {:<12} {:<20} {:<10} {:<10} {:>15}", 
                    offer.id.cyan(),
                    offer.name,
                    cpu_display,
                    ram_display,
                    rate_display.yellow()
                );
                
                if !offer.description.is_empty() {
                    println!("  {}", format!("  └─ {}", offer.description).dimmed());
                }
            }
        }
    }

    println!();
    
    if let Some(mints) = response.mint_urls {
        println!("{}", "🏦 Accepted Mints".bold());
        for mint in mints {
            println!("  • {}", mint.cyan());
        }
        println!();
    }

    println!("{}", "💡 Tip: Use 'paygress-cli spawn --tier <ID> --token <CASHU_TOKEN>' to spawn a pod".dimmed());

    Ok(())
}
