//! Burn tokens by sending them back to the issuer (faucet).

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
    let sender_id_str = std::env::args()
        .nth(1)
        .context("usage: burn <sender_account_id> <amount>")?;
    let amount: u64 = std::env::args()
        .nth(2)
        .context("usage: burn <sender_account_id> <amount>")?
        .parse()
        .context("amount must be a number")?;

    let (issuer_id, _) = AccountId::parse(&issuer_id_str).context("invalid issuer account ID")?;
    let (sender_id, _) = AccountId::parse(&sender_id_str).context("invalid sender account ID")?;

    let signer = Arc::new(SimulatedMpcSigner::new());
    let secret_key = SecretKey::new();
    signer.register_key(secret_key.clone());

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

    client.sync_state().await.context("failed to sync state")?;
    operations::burn_tokens(&mut client, sender_id, issuer_id, amount).await?;

    println!("Burned {amount} tokens (sender: {sender_id}, issuer: {issuer_id})");
    Ok(())
}
