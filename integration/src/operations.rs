//! Issuer operations — deploy, mint, burn, transfer, balance reads, supply reads.
//!
//! All functions use "issuer" semantics (not "faucet") for Brale's product vocabulary.
//! Under the hood, an "issuer" is a Miden faucet account with ECDSA K256 authentication.

use anyhow::{Context, Result};
use miden_client::{
    account::{
        component::BasicWallet, Account, AccountBuilder, AccountId, AccountStorageMode,
        AccountType,
    },
    asset::{FungibleAsset, TokenSymbol},
    auth::{AuthEcdsaK256Keccak, PublicKeyCommitment},
    note::NoteType,
    store::AccountRecordData,
    transaction::{PaymentNoteDescription, TransactionRequestBuilder},
    Client, Felt,
};
use miden_protocol::account::AccountStorage;
use miden_protocol::crypto::dsa::ecdsa_k256_keccak;
use miden_protocol::note::NoteAttachment;
use miden_protocol::transaction::OutputNote;
use miden_standards::account::faucets::BasicFungibleFaucet;
use miden_standards::note::create_burn_note;
use rand::RngCore;
use tracing::info;

use crate::keystore::BraleKeystore;

// ---------------------------------------------------------------------------
// Account creation
// ---------------------------------------------------------------------------

/// Create a wallet account with ECDSA K256 authentication.
///
/// Returns the created account and its public key commitment.
pub async fn create_wallet_account(
    client: &mut Client<BraleKeystore>,
    pub_key: &ecdsa_k256_keccak::PublicKey,
) -> Result<Account> {
    let mut init_seed = [0u8; 32];
    client.rng().fill_bytes(&mut init_seed);

    let pub_key_commitment = PublicKeyCommitment::from(pub_key.to_commitment());

    let account = AccountBuilder::new(init_seed)
        .account_type(AccountType::RegularAccountImmutableCode)
        .storage_mode(AccountStorageMode::Private)
        .with_component(BasicWallet)
        .with_auth_component(AuthEcdsaK256Keccak::new(pub_key_commitment))
        .build()
        .context("failed to build wallet account")?;

    client
        .add_account(&account, false)
        .await
        .context("failed to add wallet account to client")?;

    info!(account_id = %account.id(), "created wallet account");
    Ok(account)
}

/// Deploy an issuer (faucet) account with ECDSA K256 authentication.
///
/// - `symbol`: Token symbol (e.g. "USDB"), 1-4 uppercase ASCII chars.
/// - `decimals`: Decimal places (max 12).
/// - `max_supply`: Maximum total supply as a raw amount.
pub async fn deploy_issuer_account(
    client: &mut Client<BraleKeystore>,
    pub_key: &ecdsa_k256_keccak::PublicKey,
    symbol: &str,
    decimals: u8,
    max_supply: u64,
) -> Result<Account> {
    let mut init_seed = [0u8; 32];
    client.rng().fill_bytes(&mut init_seed);

    let token_symbol =
        TokenSymbol::new(symbol).context("invalid token symbol")?;
    let pub_key_commitment = PublicKeyCommitment::from(pub_key.to_commitment());

    let faucet_component = BasicFungibleFaucet::new(
        token_symbol,
        decimals,
        Felt::new(max_supply),
    )
    .context("failed to create faucet component")?;

    let account = AccountBuilder::new(init_seed)
        .account_type(AccountType::FungibleFaucet)
        .storage_mode(AccountStorageMode::Public)
        .with_component(faucet_component)
        .with_auth_component(AuthEcdsaK256Keccak::new(pub_key_commitment))
        .build()
        .context("failed to build issuer account")?;

    client
        .add_account(&account, false)
        .await
        .context("failed to add issuer account to client")?;

    info!(
        account_id = %account.id(),
        symbol,
        decimals,
        max_supply,
        "deployed issuer account"
    );
    Ok(account)
}

// ---------------------------------------------------------------------------
// Token operations
// ---------------------------------------------------------------------------

/// Mint tokens from an issuer to a target account.
///
/// Creates a P2ID note sending newly minted tokens to `target_id`.
/// Must be executed against the issuer (faucet) account.
pub async fn mint_tokens(
    client: &mut Client<BraleKeystore>,
    issuer_id: AccountId,
    target_id: AccountId,
    amount: u64,
) -> Result<()> {
    let asset = FungibleAsset::new(issuer_id, amount)
        .context("failed to create fungible asset for minting")?;

    let tx_request = TransactionRequestBuilder::new()
        .build_mint_fungible_asset(asset, target_id, NoteType::Private, client.rng())
        .context("failed to build mint transaction request")?;

    let tx_id = client
        .submit_new_transaction(issuer_id, tx_request)
        .await
        .context("failed to submit mint transaction")?;

    info!(%tx_id, %issuer_id, %target_id, amount, "minted tokens");
    Ok(())
}

/// Burn tokens by creating a burn note and executing it against the issuer.
///
/// The `sender_id` creates a burn note targeting the issuer (faucet), then the
/// issuer consumes the note to reduce total issuance.
///
/// Two-step process:
/// 1. Sender creates burn note (this function handles both steps if sender == issuer)
/// 2. Issuer consumes the burn note
pub async fn burn_tokens(
    client: &mut Client<BraleKeystore>,
    sender_id: AccountId,
    issuer_id: AccountId,
    amount: u64,
) -> Result<()> {
    let asset = FungibleAsset::new(issuer_id, amount)
        .context("failed to create fungible asset for burning")?;

    let burn_note = create_burn_note(
        sender_id,
        issuer_id,
        asset.into(),
        NoteAttachment::default(),
        client.rng(),
    )
    .context("failed to create burn note")?;

    // Step 1: Sender creates the burn note as an output
    let tx_request = TransactionRequestBuilder::new()
        .own_output_notes(vec![OutputNote::Full(burn_note)])
        .build()
        .context("failed to build burn sender transaction")?;

    let tx_id = client
        .submit_new_transaction(sender_id, tx_request)
        .await
        .context("failed to submit burn sender transaction")?;

    info!(%tx_id, %sender_id, %issuer_id, amount, "created burn note");

    // Step 2: Wait for burn note to appear in a block, then have the issuer consume it
    let mut consumable;
    let max_attempts = 120;
    for attempt in 1..=max_attempts {
        client.sync_state().await.context("failed to sync state after burn note creation")?;

        consumable = client
            .get_consumable_notes(Some(issuer_id))
            .await
            .context("failed to get consumable notes for issuer")?;

        if !consumable.is_empty() {
            break;
        }

        if attempt == max_attempts {
            anyhow::bail!("no consumable burn notes found for issuer after {max_attempts} sync attempts");
        }

        info!(%issuer_id, attempt, "burn note not yet in block, waiting…");
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }

    let consumable = client
        .get_consumable_notes(Some(issuer_id))
        .await
        .context("failed to get consumable notes for issuer")?;

    let notes: Vec<_> = consumable
        .into_iter()
        .filter_map(|(record, _)| record.try_into().ok())
        .collect();

    let tx_request = TransactionRequestBuilder::new()
        .build_consume_notes(notes)
        .context("failed to build consume burn notes transaction")?;

    let tx_id = client
        .submit_new_transaction(issuer_id, tx_request)
        .await
        .context("failed to submit burn consumption transaction")?;

    info!(%tx_id, %issuer_id, amount, "burned tokens (issuer consumed burn note)");
    Ok(())
}

/// Transfer tokens from sender to recipient via P2ID note.
///
/// Two-step process:
/// 1. Sender creates P2ID note
/// 2. Recipient syncs and consumes
pub async fn transfer_tokens(
    client: &mut Client<BraleKeystore>,
    sender_id: AccountId,
    recipient_id: AccountId,
    issuer_id: AccountId,
    amount: u64,
) -> Result<()> {
    let asset = FungibleAsset::new(issuer_id, amount)
        .context("failed to create fungible asset for transfer")?;

    let payment = PaymentNoteDescription::new(vec![asset.into()], sender_id, recipient_id);

    let tx_request = TransactionRequestBuilder::new()
        .build_pay_to_id(payment, NoteType::Private, client.rng())
        .context("failed to build P2ID transfer request")?;

    let tx_id = client
        .submit_new_transaction(sender_id, tx_request)
        .await
        .context("failed to submit transfer transaction")?;

    info!(%tx_id, %sender_id, %recipient_id, amount, "transfer sent (P2ID note created)");
    Ok(())
}

/// Consume all available notes for the given account.
///
/// Syncs state first, then consumes any notes addressed to this account.
pub async fn consume_notes(
    client: &mut Client<BraleKeystore>,
    account_id: AccountId,
) -> Result<()> {
    // Poll until notes appear — the node may not have included the
    // preceding transaction in a block yet.
    let mut consumable;
    let max_attempts = 120;
    for attempt in 1..=max_attempts {
        client.sync_state().await.context("failed to sync state")?;

        consumable = client
            .get_consumable_notes(Some(account_id))
            .await
            .context("failed to get consumable notes")?;

        if !consumable.is_empty() {
            break;
        }

        if attempt == max_attempts {
            anyhow::bail!("no consumable notes found for {account_id} after {max_attempts} sync attempts");
        }

        info!(%account_id, attempt, "no consumable notes yet, waiting for next block…");
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }

    let consumable = client
        .get_consumable_notes(Some(account_id))
        .await
        .context("failed to get consumable notes")?;

    let notes: Vec<_> = consumable
        .into_iter()
        .filter_map(|(record, _)| record.try_into().ok())
        .collect();

    let count = notes.len();
    let tx_request = TransactionRequestBuilder::new()
        .build_consume_notes(notes)
        .context("failed to build consume notes transaction")?;

    let tx_id = client
        .submit_new_transaction(account_id, tx_request)
        .await
        .context("failed to submit consume notes transaction")?;

    info!(%tx_id, %account_id, count, "consumed notes");
    Ok(())
}

// ---------------------------------------------------------------------------
// State reads
// ---------------------------------------------------------------------------

/// Read the balance of a specific token (issuer) held by an account.
pub async fn read_balance(
    client: &Client<BraleKeystore>,
    account_id: AccountId,
    issuer_id: AccountId,
) -> Result<u64> {
    let record = client
        .get_account(account_id)
        .await
        .context("failed to get account")?
        .context("account not found")?;

    let account = match record.account_data() {
        AccountRecordData::Full(acc) => acc,
        AccountRecordData::Partial(_) => anyhow::bail!("account {account_id} is only partially tracked"),
    };

    let balance = account
        .vault()
        .get_balance(issuer_id)
        .context("failed to read balance from vault")?;

    info!(%account_id, %issuer_id, balance, "read balance");
    Ok(balance)
}

/// Read the total token issuance from the issuer (faucet) account.
///
/// This reads the faucet's sysdata storage slot which tracks total distributed supply.
pub async fn read_total_supply(
    client: &Client<BraleKeystore>,
    issuer_id: AccountId,
) -> Result<u64> {
    let record = client
        .get_account(issuer_id)
        .await
        .context("failed to get issuer account")?
        .context("issuer account not found")?;

    let account = match record.account_data() {
        AccountRecordData::Full(acc) => acc,
        AccountRecordData::Partial(_) => anyhow::bail!("issuer {issuer_id} is only partially tracked"),
    };

    // The total issuance is stored in the faucet's reserved sysdata slot
    // ("miden::protocol::faucet::sysdata") with [total_issuance, 0, 0, 0].
    let issuance_slot = account
        .storage()
        .get_item(AccountStorage::faucet_sysdata_slot())
        .context("failed to read issuance storage slot")?;

    let total_supply = issuance_slot[0].as_int();

    info!(%issuer_id, total_supply, "read total supply");
    Ok(total_supply)
}

/// Token metadata read from a faucet's storage.
#[derive(Debug, Clone)]
pub struct TokenMetadata {
    pub symbol: TokenSymbol,
    pub decimals: u8,
    pub max_supply: u64,
}

/// Read token metadata (symbol, decimals, max supply) from a faucet account.
///
/// The metadata is stored in the faucet's reserved metadata slot by `BasicFungibleFaucet`.
pub async fn read_token_metadata(
    client: &Client<BraleKeystore>,
    issuer_id: AccountId,
) -> Result<TokenMetadata> {
    let record = client
        .get_account(issuer_id)
        .await
        .context("failed to get issuer account")?
        .context("issuer account not found")?;

    let account = match record.account_data() {
        AccountRecordData::Full(acc) => acc,
        AccountRecordData::Partial(_) => anyhow::bail!("issuer {issuer_id} is only partially tracked"),
    };

    let metadata_word = account
        .storage()
        .get_item(BasicFungibleFaucet::metadata_slot())
        .context("failed to read metadata storage slot")?;

    // Metadata layout: [max_supply, decimals, token_symbol, 0]
    let max_supply = metadata_word[0].as_int();
    let decimals = metadata_word[1].as_int() as u8;
    let symbol = TokenSymbol::try_from(metadata_word[2])
        .context("invalid token symbol in faucet metadata")?;

    info!(%issuer_id, ?symbol, decimals, max_supply, "read token metadata");
    Ok(TokenMetadata { symbol, decimals, max_supply })
}

/// Sync client state with the network.
pub async fn sync_and_track(client: &mut Client<BraleKeystore>) -> Result<()> {
    let summary = client.sync_state().await.context("failed to sync state")?;
    info!(
        block_num = %summary.block_num,
        committed_notes = summary.committed_notes.len(),
        consumed_notes = summary.consumed_notes.len(),
        "synced state"
    );
    Ok(())
}
