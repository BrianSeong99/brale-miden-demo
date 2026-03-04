//! End-to-end demo: deploy issuer → create wallets → mint → transfer → read balances → burn.

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
    let rpc = Arc::new(GrpcClient::new(&endpoint, 10_000));
    let fs_keystore = FilesystemKeyStore::new(config.keystore_path)
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
        .sqlite_store(config.sqlite_store_path)
        .authenticator(keystore.clone())
        .build()
        .await
        .context("failed to build client")?;

    // Sync with network to populate local store with block data
    client.sync_state().await.context("failed to sync state")?;

    // Step 1: Deploy issuer
    println!("=== Step 1: Deploy Issuer ===");
    let issuer = operations::deploy_issuer_account(
        &mut client, &issuer_pk, "USDB", 6, 1_000_000_000_000,
    ).await?;
    println!("Issuer deployed: {}", issuer.id());

    // Step 2: Create wallets
    println!("\n=== Step 2: Create Wallets ===");
    let wallet_a = operations::create_wallet_account(&mut client, &wallet_a_pk).await?;
    println!("Wallet A: {}", wallet_a.id());
    let wallet_b = operations::create_wallet_account(&mut client, &wallet_b_pk).await?;
    println!("Wallet B: {}", wallet_b.id());

    // Step 3: Mint 1000 tokens to Wallet A
    println!("\n=== Step 3: Mint Tokens ===");
    operations::mint_tokens(&mut client, issuer.id(), wallet_a.id(), 1000).await?;
    println!("Minted 1000 tokens to Wallet A");

    // Sync and consume at Wallet A
    operations::consume_notes(&mut client, wallet_a.id()).await?;
    println!("Wallet A consumed mint note");

    // Step 4: Transfer 300 from Wallet A to Wallet B
    println!("\n=== Step 4: Transfer Tokens ===");
    operations::transfer_tokens(
        &mut client, wallet_a.id(), wallet_b.id(), issuer.id(), 300,
    ).await?;
    println!("Transferred 300 tokens A → B");

    // Sync and consume at Wallet B
    operations::consume_notes(&mut client, wallet_b.id()).await?;
    println!("Wallet B consumed transfer note");

    // Step 5: Read balances
    println!("\n=== Step 5: Read Balances ===");
    let balance_a = operations::read_balance(&client, wallet_a.id(), issuer.id()).await?;
    let balance_b = operations::read_balance(&client, wallet_b.id(), issuer.id()).await?;
    println!("Wallet A balance: {balance_a}");
    println!("Wallet B balance: {balance_b}");

    // Step 6: Read total supply
    println!("\n=== Step 6: Read Total Supply ===");
    let supply = operations::read_total_supply(&client, issuer.id()).await?;
    println!("Total supply: {supply}");

    // Step 7: Burn 100 tokens from Wallet B
    println!("\n=== Step 7: Burn Tokens ===");
    operations::burn_tokens(&mut client, wallet_b.id(), issuer.id(), 100).await?;
    println!("Burned 100 tokens from Wallet B");

    // Final state
    println!("\n=== Final State ===");
    let final_balance_a = operations::read_balance(&client, wallet_a.id(), issuer.id()).await?;
    let final_balance_b = operations::read_balance(&client, wallet_b.id(), issuer.id()).await?;
    let final_supply = operations::read_total_supply(&client, issuer.id()).await?;
    println!("Wallet A: {final_balance_a}");
    println!("Wallet B: {final_balance_b}");
    println!("Total supply: {final_supply}");

    println!("\nFull demo complete!");
    Ok(())
}
