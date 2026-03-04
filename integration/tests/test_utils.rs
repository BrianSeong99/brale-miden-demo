//! Shared test helpers for the Brale Miden integration tests.

use miden_testing::{Auth, MockChain};
use miden_protocol::account::Account;
use miden_protocol::asset::FungibleAsset;
use miden_protocol::note::{Note, NoteType};

/// Test environment with a faucet, wallet, and P2ID mint note.
pub struct TestSetup {
    pub mock_chain: MockChain,
    pub faucet: Account,
    pub wallet: Account,
    pub mint_note: Note,
}

/// Build a test environment with:
/// - An ECDSA K256 faucet (symbol: "TST", max supply: 1_000_000)
/// - An ECDSA K256 wallet
/// - A P2ID mint note of `amount` tokens from faucet to wallet
pub fn setup_mint_test(amount: u64) -> anyhow::Result<TestSetup> {
    let mut builder = MockChain::builder();

    let faucet = builder.add_existing_basic_faucet(
        Auth::EcdsaK256KeccakAuth,
        "TST",
        1_000_000,
        None,
    )?;
    let wallet = builder.add_existing_wallet(Auth::EcdsaK256KeccakAuth)?;

    let asset = FungibleAsset::new(faucet.id(), amount)?;
    let note = builder.add_p2id_note(
        faucet.id(),
        wallet.id(),
        &[asset.into()],
        NoteType::Private,
    )?;

    let mock_chain = builder.build()?;

    Ok(TestSetup {
        mock_chain,
        faucet,
        wallet,
        mint_note: note,
    })
}

/// Assert that an account's vault contains exactly `expected` of the given faucet's token.
pub fn assert_balance(account: &Account, faucet_id: miden_protocol::account::AccountId, expected: u64) {
    let balance = account
        .vault()
        .get_balance(faucet_id)
        .expect("failed to read balance");
    assert_eq!(balance, expected, "expected balance {expected}, got {balance}");
}
