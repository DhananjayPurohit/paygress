// Cashu Token Utilities
//
// Provides token amount extraction only
// Payment verification handled by ngx_l402 at nginx layer

use std::sync::{Arc, OnceLock};
use std::path::Path;

const MSAT_PER_SAT: u64 = 1000;

// Database singleton (kept for compatibility, may not be needed)
static CASHU_DB: OnceLock<Arc<cdk_redb::wallet::WalletRedbDatabase>> = OnceLock::new();

pub async fn initialize_cashu(db_path: &str) -> Result<(), String> {
    // Initialize database for compatibility
    match cdk_redb::wallet::WalletRedbDatabase::new(Path::new(db_path)) {
        Ok(db) => {
            tracing::debug!("Cashu database initialized at: {}", db_path);
            let _ = CASHU_DB.set(Arc::new(db));
            Ok(())
        },
        Err(e) => {
            let error = format!("Failed to create Cashu database: {:?}", e);
            tracing::error!("{}", error);
            Err(error)
        }
    }
}

// verify_cashu_token removed - ngx_l402 handles all payment verification
// Payment validation now happens at nginx layer before requests reach Paygress
