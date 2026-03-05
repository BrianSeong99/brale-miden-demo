//! Multisig demo: 2-of-3 ECDSA K256 multisig wallet creation and signing flow.
//!
//! Requires a running PSM server (default: http://localhost:50051).
//! See README.md for PSM server setup instructions.

use anyhow::{Context, Result};
use integration::config::Config;
use integration::display;
use integration::multisig::SimulatedMpcKeyManager;
use miden_multisig_client::{Endpoint, KeyManager, MultisigClient};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let config = Config::from_env();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(&config.log_level))
        .init();

    // Step 1: Generate 3 ECDSA K256 keypairs
    println!("=== Step 1: Generate 3 ECDSA K256 Keypairs ===");
    let km1 = SimulatedMpcKeyManager::generate();
    let km2 = SimulatedMpcKeyManager::generate();
    let km3 = SimulatedMpcKeyManager::generate();

    let c1 = km1.commitment_hex();
    let c2 = km2.commitment_hex();
    let c3 = km3.commitment_hex();

    display::print_signer_table(
        &[
            ("Signer 1", &c1, false),
            ("Signer 2", &c2, false),
            ("Signer 3", &c3, false),
        ],
        2,
    );

    // Step 2: Build MultisigClient for signer 1
    println!("\n=== Step 2: Build MultisigClient ===");
    let miden_endpoint = Endpoint::try_from(config.miden_rpc_endpoint.as_str())
        .map_err(|e| anyhow::anyhow!(e))?;

    let mut client = MultisigClient::builder()
        .miden_endpoint(miden_endpoint)
        .psm_endpoint(&config.psm_endpoint)
        .account_dir("./multisig-accounts")
        .with_ecdsa_secret_key(km1.clone_secret_key())
        .build()
        .await
        .context("failed to build MultisigClient")?;

    println!("MultisigClient built for signer 1");

    // Step 3: Create 2-of-3 multisig account
    println!("\n=== Step 3: Create 2-of-3 Multisig Account ===");
    let signer_commitments = vec![
        km1.commitment(),
        km2.commitment(),
        km3.commitment(),
    ];

    let account = client
        .create_account(2, signer_commitments)
        .await
        .context("failed to create multisig account")?;

    println!("Multisig account: {}", account.id());
    println!("Threshold: 2-of-3");

    // Step 4: Demonstrate signing capability
    println!("\n=== Step 4: Demonstrate Signing ===");
    let test_message = km1.commitment(); // use commitment as test message

    let sig1 = km1.sign_hex(test_message);
    println!(
        "Signer 1 signed: {}...{}",
        &sig1[..10],
        &sig1[sig1.len() - 6..]
    );

    display::print_signer_table(
        &[
            ("Signer 1", &c1, true),
            ("Signer 2", &c2, false),
            ("Signer 3", &c3, false),
        ],
        2,
    );

    let sig2 = km2.sign_hex(test_message);
    println!(
        "\nSigner 2 signed: {}...{}",
        &sig2[..10],
        &sig2[sig2.len() - 6..]
    );

    display::print_signer_table(
        &[
            ("Signer 1", &c1, true),
            ("Signer 2", &c2, true),
            ("Signer 3", &c3, false),
        ],
        2,
    );

    // Step 5: Summary
    println!("\n=== Multisig Demo Complete ===");
    println!("Account: {}", account.id());
    println!("Scheme: ECDSA K256 Keccak");
    println!("Signers: 3 generated, 2 signed");
    println!("\nTo execute a full multisig transaction:");
    println!("  1. Propose a transaction via client.propose(...)");
    println!("  2. Each signer calls client.sign(proposal_id)");
    println!("  3. Once threshold met, client.execute(proposal_id)");

    Ok(())
}
