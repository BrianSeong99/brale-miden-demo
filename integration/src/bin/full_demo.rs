//! End-to-end demo: deploy issuer → create wallets → mint → transfer → read balances → burn.

use std::sync::Arc;

use anyhow::{Context, Result};
use integration::{
    config::Config, display, keystore::BraleKeystore, mock_signer::SimulatedMpcSigner, operations,
};
use miden_client::{
    account::AccountStorageMode,
    auth::AuthSecretKey,
    builder::ClientBuilder,
    keystore::FilesystemKeyStore,
    rpc::{Endpoint, GrpcClient},
    Client,
};
use miden_client::account::AccountId;
use miden_client_sqlite_store::ClientBuilderSqliteExt;
use miden_protocol::crypto::dsa::ecdsa_k256_keccak::SecretKey;
use tracing_subscriber::EnvFilter;

/// Read all three balances and print the state table.
async fn print_state(
    client: &Client<BraleKeystore>,
    issuer_id: AccountId,
    wallet_a_id: AccountId,
    wallet_b_id: AccountId,
) -> Result<()> {
    let supply = operations::read_total_supply(client, issuer_id).await?;
    let bal_a = operations::read_balance(client, wallet_a_id, issuer_id).await?;
    let bal_b = operations::read_balance(client, wallet_b_id, issuer_id).await?;
    display::print_balance_table("USDB", supply, bal_a, bal_b);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let config = Config::from_env();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(&config.log_level))
        .init();

    let private = std::env::args().any(|a| a == "--private");
    let storage_mode = if private {
        AccountStorageMode::Private
    } else {
        AccountStorageMode::Public
    };
    println!(
        "Wallet storage mode: {}",
        if private { "Private" } else { "Public" }
    );

    // Use a fresh temp directory for store and keystore so stale state from
    // previous runs doesn't interfere.
    let tmp_dir = tempfile::tempdir().context("failed to create temp dir")?;
    let store_path = tmp_dir.path().join("store.sqlite3");
    let keystore_path = tmp_dir.path().join("keystore");

    // Generate keys for issuer and two wallets
    let issuer_sk = SecretKey::new();
    let issuer_pk = issuer_sk.public_key();
    let wallet_a_sk = SecretKey::new();
    let wallet_a_pk = wallet_a_sk.public_key();
    let wallet_b_sk = SecretKey::new();
    let wallet_b_pk = wallet_b_sk.public_key();

    // Set up signing backend with all keys
    let signer = Arc::new(SimulatedMpcSigner::new());
    signer.register_key(issuer_sk.clone());
    signer.register_key(wallet_a_sk.clone());
    signer.register_key(wallet_b_sk.clone());

    // Build client
    let endpoint = Endpoint::try_from(config.miden_rpc_endpoint.as_str())
        .map_err(|e| anyhow::anyhow!(e))?;
    let rpc = Arc::new(GrpcClient::new(&endpoint, 30_000));
    let fs_keystore = FilesystemKeyStore::new(keystore_path)
        .context("failed to create keystore")?;
    let keystore = Arc::new(BraleKeystore::new(fs_keystore, signer));

    keystore.inner().add_key(&AuthSecretKey::EcdsaK256Keccak(issuer_sk))
        .context("failed to store issuer key")?;
    keystore.inner().add_key(&AuthSecretKey::EcdsaK256Keccak(wallet_a_sk))
        .context("failed to store wallet A key")?;
    keystore.inner().add_key(&AuthSecretKey::EcdsaK256Keccak(wallet_b_sk))
        .context("failed to store wallet B key")?;

    let mut client = ClientBuilder::new()
        .rpc(rpc)
        .sqlite_store(store_path)
        .authenticator(keystore.clone())
        .build()
        .await
        .context("failed to build client")?;

    // Sync with network to populate local store with block data
    client.sync_state().await.context("failed to sync state")?;

    // Step 1: Deploy issuer
    println!("\n=== Step 1: Deploy Issuer ===");
    let issuer = operations::deploy_issuer_account(
        &mut client, &issuer_pk, "USDB", 6, 1_000_000_000_000,
    ).await?;
    println!("Issuer deployed: {}", issuer.id());

    // Step 2: Create wallets
    println!("\n=== Step 2: Create Wallets ===");
    let wallet_a = operations::create_wallet_account(&mut client, &wallet_a_pk, storage_mode).await?;
    println!("Wallet A: {}", wallet_a.id());
    let wallet_b = operations::create_wallet_account(&mut client, &wallet_b_pk, storage_mode).await?;
    println!("Wallet B: {}", wallet_b.id());

    // Sync again to register note tags for newly created accounts
    client.sync_state().await.context("failed to sync state after account creation")?;

    // Step 3: Mint 1000 tokens to Wallet A
    println!("\n=== Step 3: Mint Tokens ===");
    operations::mint_tokens(&mut client, issuer.id(), wallet_a.id(), 1000).await?;
    println!("Minted 1000 tokens to Wallet A");

    // Sync and consume at Wallet A
    operations::consume_notes(&mut client, wallet_a.id()).await?;
    println!("Wallet A consumed mint note");
    print_state(&client, issuer.id(), wallet_a.id(), wallet_b.id()).await?;

    // Step 4: Transfer 300 from Wallet A to Wallet B
    println!("\n=== Step 4: Transfer Tokens ===");
    operations::transfer_tokens(
        &mut client, wallet_a.id(), wallet_b.id(), issuer.id(), 300,
    ).await?;
    println!("Transferred 300 tokens A → B");

    // Sync and consume at Wallet B
    operations::consume_notes(&mut client, wallet_b.id()).await?;
    println!("Wallet B consumed transfer note");
    print_state(&client, issuer.id(), wallet_a.id(), wallet_b.id()).await?;

    // Step 5: Burn 100 tokens from Wallet B
    println!("\n=== Step 5: Burn Tokens ===");
    operations::burn_tokens(&mut client, wallet_b.id(), issuer.id(), 100).await?;
    println!("Burned 100 tokens from Wallet B");
    print_state(&client, issuer.id(), wallet_a.id(), wallet_b.id()).await?;

    println!("\nFull demo complete!");
    Ok(())
}
