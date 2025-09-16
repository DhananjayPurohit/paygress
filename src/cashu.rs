use cdk;
// rand::Rng not needed, using rand::random directly
use std::cell::RefCell;
use std::collections::HashSet;
use std::str::FromStr;
use std::sync::{Arc, OnceLock};
// WalletDatabase import removed as it's unused
use std::path::Path;
// Runtime import removed - using async/await instead

// Thread-local storage to track processed tokens
thread_local! {
    static PROCESSED_TOKENS: RefCell<Option<HashSet<String>>> = RefCell::new(None);
}

const MSAT_PER_SAT: u64 = 1000;

// Database singleton
static CASHU_DB: OnceLock<Arc<cdk_redb::wallet::WalletRedbDatabase>> = OnceLock::new();

// Function to validate if a mint URL is whitelisted
pub fn is_mint_whitelisted(mint_url: &str, whitelisted_mints: &[String]) -> bool {
    // Normalize the mint URL for comparison
    let normalized_mint = mint_url.trim_end_matches('/').to_lowercase();
    
    whitelisted_mints.iter().any(|whitelisted| {
        let normalized_whitelisted = whitelisted.trim_end_matches('/').to_lowercase();
        normalized_mint == normalized_whitelisted || 
        normalized_mint.starts_with(&normalized_whitelisted)
    })
}

pub async fn initialize_cashu(db_path: &str) -> Result<(), String> {
    // Initialize PROCESSED_TOKENS with empty HashSet
    PROCESSED_TOKENS.with(|tokens| {
        *tokens.borrow_mut() = Some(HashSet::new());
    });
    
    // Initialize database (no nested runtime needed)
    match cdk_redb::wallet::WalletRedbDatabase::new(Path::new(db_path)) {
        Ok(db) => {
            println!("Cashu database initialized successfully");
            let _ = CASHU_DB.set(Arc::new(db));
            Ok(())
        },
        Err(e) => {
            let error = format!("Failed to create Cashu database: {:?}", e);
            println!("{}", error);
            Err(error)
        }
    }
}

pub async fn verify_cashu_token(token: &str, amount_msat: i64, whitelisted_mints: &[String]) -> Result<bool, String> {
    // Check if token was already processed
    let token_already_processed = PROCESSED_TOKENS.with(|tokens| {
        if let Some(set) = tokens.borrow().as_ref() {
            set.contains(token)
        } else {
            false
        }
    });

    if token_already_processed {
        println!("Token already processed");
        return Ok(true);
    }

    // Decode the token from string
    let token_decoded = match cdk::nuts::Token::from_str(token) {
        Ok(token) => token,
        Err(e) => {
            eprintln!("Failed to decode Cashu token: {}", e);
            return Err(format!("Failed to decode Cashu token: {}", e));
        }
    };
    
    // Check if the token is valid
    if token_decoded.proofs().is_empty() {
        return Ok(false);
    }
    
    // Calculate total token amount in millisatoshis
    let total_amount = token_decoded.value()
        .map_err(|e| format!("Failed to get token value: {}", e))?;

    // Check if the token unit is in millisatoshis or satoshis
    let total_amount_msat: u64 = if token_decoded.unit().unwrap() == cdk::nuts::CurrencyUnit::Sat {
        u64::from(total_amount) * MSAT_PER_SAT
    } else if token_decoded.unit().unwrap() == cdk::nuts::CurrencyUnit::Msat {
        u64::from(total_amount)
    } else {
        // Other units not supported
        return Err(format!("Unsupported token unit: {:?}", token_decoded.unit().unwrap()));
    };
    
    // Check if the token amount is sufficient
    if total_amount_msat < amount_msat as u64 {
        eprintln!("Cashu token amount insufficient: {} msat (required: {} msat)", 
            total_amount_msat, amount_msat);
        return Ok(false);
    }
    
    println!("Successfully decoded Cashu token with {} proofs and {} msat (required: {} msat)", 
        token_decoded.proofs().len(),
        total_amount_msat,
        amount_msat);
    
    // Extract mint URL from the token
    let mint_url = token_decoded.mint_url()
        .map_err(|e| format!("Failed to get mint URL: {}", e))?;

    // Validate mint URL against whitelist
    if !is_mint_whitelisted(&mint_url.to_string(), whitelisted_mints) {
        eprintln!("Mint URL not whitelisted: {}", mint_url);
        return Err(format!("Mint URL not whitelisted: {}", mint_url));
    }

    println!("Mint URL validated: {}", mint_url);

    let unit = token_decoded.unit().unwrap();
    
    // Use the shared database instance
    let db = CASHU_DB.get()
        .ok_or_else(|| "Cashu database not initialized".to_string())?
        .clone();
        
    let seed = rand::random::<[u8; 32]>();
    let wallet = cdk::wallet::Wallet::new(&mint_url.to_string(), unit, db, &seed, None)
        .map_err(|e| format!("Failed to create wallet: {}", e))?;

    match wallet.receive(token, cdk::wallet::ReceiveOptions::default()).await {
        Ok(_) => {
            println!("Cashu token received successful");
            // Add token to processed set after successful receive
            PROCESSED_TOKENS.with(|tokens| {
                if let Some(set) = tokens.borrow_mut().as_mut() {
                    set.insert(token.to_string());
                }
            });
            Ok(true)
        },
        Err(e) => {
            eprintln!("Cashu token receive failed: {}", e);
            Ok(false)
        }
    }
}