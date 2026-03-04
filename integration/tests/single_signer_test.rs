//! Single-signer tests using MockChain with ECDSA K256 Keccak authentication.
//!
//! Tests cover: account creation, minting, consuming notes, balance reads, supply reads.

mod test_utils;

use miden_protocol::account::AccountStorage;
use miden_protocol::asset::{FungibleAsset, TokenSymbol};
use miden_protocol::note::NoteType;
use miden_standards::account::faucets::BasicFungibleFaucet;
use miden_testing::{Auth, MockChain};

// ---------------------------------------------------------------------------
// Happy paths
// ---------------------------------------------------------------------------

#[tokio::test]
async fn create_ecdsa_wallet() -> anyhow::Result<()> {
    let mut builder = MockChain::builder();
    let wallet = builder.add_existing_wallet(Auth::EcdsaK256KeccakAuth)?;
    let _chain = builder.build()?;

    assert_ne!(wallet.nonce(), miden_client::Felt::new(0));
    Ok(())
}

#[tokio::test]
async fn deploy_issuer_account() -> anyhow::Result<()> {
    let mut builder = MockChain::builder();
    let faucet = builder.add_existing_basic_faucet(
        Auth::EcdsaK256KeccakAuth,
        "USDB",
        1_000_000_000,
        None,
    )?;
    let _chain = builder.build()?;

    assert!(faucet.id().is_faucet());
    Ok(())
}

#[tokio::test]
async fn mint_and_consume() -> anyhow::Result<()> {
    let test = test_utils::setup_mint_test(100)?;
    let mock_chain = test.mock_chain;
    let mut wallet = test.wallet;

    let executed_tx = mock_chain
        .build_tx_context(wallet.id(), &[test.mint_note.id()], &[])?
        .build()?
        .execute()
        .await?;

    wallet.apply_delta(executed_tx.account_delta())?;
    test_utils::assert_balance(&wallet, test.faucet.id(), 100);
    Ok(())
}

#[tokio::test]
async fn read_balance_after_mint() -> anyhow::Result<()> {
    let test = test_utils::setup_mint_test(500)?;
    let mock_chain = test.mock_chain;
    let mut wallet = test.wallet;

    let executed_tx = mock_chain
        .build_tx_context(wallet.id(), &[test.mint_note.id()], &[])?
        .build()?
        .execute()
        .await?;

    wallet.apply_delta(executed_tx.account_delta())?;
    let balance = wallet.vault().get_balance(test.faucet.id())?;
    assert_eq!(balance, 500);
    Ok(())
}

#[tokio::test]
async fn faucet_sysdata_slot_readable() -> anyhow::Result<()> {
    // Verify that the faucet's sysdata slot is accessible and starts at 0 for a fresh faucet.
    let mut builder = MockChain::builder();
    let faucet = builder.add_existing_basic_faucet(
        Auth::EcdsaK256KeccakAuth,
        "TST",
        1_000_000,
        None,
    )?;
    let _chain = builder.build()?;

    let issuance_slot = faucet
        .storage()
        .get_item(AccountStorage::faucet_sysdata_slot())?;
    // Fresh faucet starts with 0 total issuance
    let total_supply = issuance_slot[0].as_int();
    assert_eq!(total_supply, 0);
    Ok(())
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

#[tokio::test]
async fn faucet_is_configured_correctly() -> anyhow::Result<()> {
    // Verify that the faucet account type is correct and storage is initialized.
    let mut builder = MockChain::builder();
    let faucet = builder.add_existing_basic_faucet(
        Auth::EcdsaK256KeccakAuth,
        "TST",
        1000,
        None,
    )?;
    let _chain = builder.build()?;

    assert!(faucet.id().is_faucet());
    // Sysdata slot should be accessible
    let issuance_slot = faucet
        .storage()
        .get_item(AccountStorage::faucet_sysdata_slot())?;
    assert_eq!(issuance_slot[0].as_int(), 0); // no tokens minted yet
    Ok(())
}

#[tokio::test]
async fn consume_multiple_notes() -> anyhow::Result<()> {
    let mut builder = MockChain::builder();
    let faucet = builder.add_existing_basic_faucet(
        Auth::EcdsaK256KeccakAuth,
        "TST",
        1_000_000,
        None,
    )?;
    let wallet = builder.add_existing_wallet(Auth::EcdsaK256KeccakAuth)?;

    let asset1 = FungibleAsset::new(faucet.id(), 100)?;
    let asset2 = FungibleAsset::new(faucet.id(), 50)?;

    let note1 = builder.add_p2id_note(
        faucet.id(),
        wallet.id(),
        &[asset1.into()],
        NoteType::Private,
    )?;
    let note2 = builder.add_p2id_note(
        faucet.id(),
        wallet.id(),
        &[asset2.into()],
        NoteType::Private,
    )?;

    let mock_chain = builder.build()?;

    let executed_tx = mock_chain
        .build_tx_context(wallet.id(), &[note1.id(), note2.id()], &[])?
        .build()?
        .execute()
        .await?;

    let mut wallet = wallet;
    wallet.apply_delta(executed_tx.account_delta())?;
    test_utils::assert_balance(&wallet, faucet.id(), 150);

    Ok(())
}

#[tokio::test]
async fn read_token_metadata_from_faucet() -> anyhow::Result<()> {
    let mut builder = MockChain::builder();
    let faucet = builder.add_existing_basic_faucet(
        Auth::EcdsaK256KeccakAuth,
        "USDB",
        1_000_000_000,
        None,
    )?;
    let _chain = builder.build()?;

    // Read metadata slot directly (same approach as read_token_metadata in operations.rs)
    let metadata_word = faucet
        .storage()
        .get_item(BasicFungibleFaucet::metadata_slot())?;

    let max_supply = metadata_word[0].as_int();
    let decimals = metadata_word[1].as_int() as u8;
    let symbol = TokenSymbol::try_from(metadata_word[2])?;

    assert_eq!(symbol, TokenSymbol::new("USDB")?);
    assert_eq!(max_supply, 1_000_000_000);
    // MockChain default decimals
    assert!(decimals <= 12, "decimals should be within valid range");

    Ok(())
}
