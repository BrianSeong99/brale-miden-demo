//! Deploy an ECDSA K256 issuer (faucet) account.

use std::sync::Arc;

use anyhow::{Context, Result};
use integration::{config::Config, keystore::BraleKeystore, mock_signer::SimulatedMpcSigner, operations};
use miden_client::{
    auth::AuthSecretKey,
    builder::ClientBuilder,
    keystore::FilesystemKeyStore,
    rpc::{Endpoint, GrpcClient},
};
use miden_client_sqlite_store::ClientBuilderSqliteExt;
use miden_protocol::crypto::dsa::ecdsa_k256_keccak::SecretKey;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let config = Config::from_env();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(&config.log_level))
        .init();

    let symbol = std::env::args().nth(1).unwrap_or_else(|| "USDB".into());
    let decimals: u8 = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(6);
    let max_supply: u64 = std::env::args()
        .nth(3)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1_000_000_000_000);

    // Generate ECDSA K256 keypair
    let secret_key = SecretKey::new();
    let public_key = secret_key.public_key();

    // Set up signing backend
    let signer = Arc::new(SimulatedMpcSigner::new());
    signer.register_key(secret_key.clone());

    // Build client with BraleKeystore
    let endpoint = Endpoint::try_from(config.miden_rpc_endpoint.as_str())
        .map_err(|e| anyhow::anyhow!(e))?;
    let rpc = Arc::new(GrpcClient::new(&endpoint, 30_000));
    let fs_keystore = FilesystemKeyStore::new(config.keystore_path)
        .context("failed to create keystore")?;
    let keystore = Arc::new(BraleKeystore::new(fs_keystore, signer));

    keystore.inner().add_key(&AuthSecretKey::EcdsaK256Keccak(secret_key))
        .context("failed to store key")?;

    let mut client = ClientBuilder::new()
        .rpc(rpc)
        .sqlite_store(config.sqlite_store_path)
        .authenticator(keystore.clone())
        .build()
        .await
        .context("failed to build client")?;

    let account = operations::deploy_issuer_account(
        &mut client,
        &public_key,
        &symbol,
        decimals,
        max_supply,
    )
    .await?;

    println!("Issuer account deployed: {}", account.id());
    println!("Symbol: {symbol}, Decimals: {decimals}, Max supply: {max_supply}");
    Ok(())
}
