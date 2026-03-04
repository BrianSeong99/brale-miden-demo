//! Mint tokens from an issuer to a target account.

use std::sync::Arc;

use anyhow::{Context, Result};
use integration::{config::Config, keystore::BraleKeystore, mock_signer::SimulatedMpcSigner, operations};
use miden_client::{
    account::AccountId,
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

    let issuer_id_str = config
        .issuer_account_id
        .context("ISSUER_ACCOUNT_ID must be set in .env")?;
    let target_id_str = std::env::args()
        .nth(1)
        .context("usage: mint <target_account_id> <amount>")?;
    let amount: u64 = std::env::args()
        .nth(2)
        .context("usage: mint <target_account_id> <amount>")?
        .parse()
        .context("amount must be a number")?;

    let (issuer_id, _) = AccountId::parse(&issuer_id_str).context("invalid issuer account ID")?;
    let (target_id, _) = AccountId::parse(&target_id_str).context("invalid target account ID")?;

    // Set up signing backend (loads issuer key from keystore)
    let signer = Arc::new(SimulatedMpcSigner::new());

    // In a real scenario, the issuer's secret key would be loaded from a secure store
    // and registered with the MPC signer. For the demo, we generate a fresh key.
    let secret_key = SecretKey::new();
    signer.register_key(secret_key.clone());

    let endpoint = Endpoint::try_from(config.miden_rpc_endpoint.as_str())
        .map_err(|e| anyhow::anyhow!(e))?;
    let rpc = Arc::new(GrpcClient::new(&endpoint, 10_000));
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

    client.sync_state().await.context("failed to sync state")?;
    operations::mint_tokens(&mut client, issuer_id, target_id, amount).await?;

    println!("Minted {amount} tokens from {issuer_id} to {target_id}");
    Ok(())
}
