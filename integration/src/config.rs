//! Environment-based configuration for the Brale Miden integration.

use std::path::PathBuf;

/// Configuration loaded from environment variables or `.env` file.
#[derive(Debug, Clone)]
pub struct Config {
    pub miden_rpc_endpoint: String,
    pub psm_endpoint: String,
    pub sqlite_store_path: PathBuf,
    pub keystore_path: PathBuf,
    pub issuer_account_id: Option<String>,
    pub log_level: String,
}

impl Config {
    /// Load configuration from environment variables, falling back to defaults.
    /// Call `dotenvy::dotenv().ok()` before this to load `.env` if desired.
    pub fn from_env() -> Self {
        Self {
            miden_rpc_endpoint: std::env::var("MIDEN_RPC_ENDPOINT")
                .unwrap_or_else(|_| "https://rpc.testnet.miden.io".into()),
            psm_endpoint: std::env::var("PSM_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:50051".into()),
            sqlite_store_path: std::env::var("SQLITE_STORE_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("./store.sqlite3")),
            keystore_path: std::env::var("KEYSTORE_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("./keystore")),
            issuer_account_id: std::env::var("ISSUER_ACCOUNT_ID").ok(),
            log_level: std::env::var("LOG_LEVEL").unwrap_or_else(|_| "info".into()),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::from_env()
    }
}
