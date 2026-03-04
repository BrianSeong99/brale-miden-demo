# Brale ↔ Miden Integration Mapping

[Back to Index](../WALKTHROUGH.md) | [Previous: What's Next](07-whats-next.md)

---

This document maps Brale's existing platform concepts to Miden primitives. If you
work with Brale's API (accounts, addresses, transfers, status lifecycles), this
shows exactly how each pattern translates.

## Architecture: Where Brale Meets Miden

```
┌─────────────────────────────────────────────────────────┐
│                    Brale Platform                        │
│                                                         │
│  ┌──────────┐  ┌──────────┐  ┌────────────────────┐    │
│  │ KYB/KYC  │  │ REST API │  │ Idempotency / Dedup│    │
│  │ (off-chain)│ │ Gateway  │  │ (PostgreSQL)       │    │
│  └──────────┘  └────┬─────┘  └────────────────────┘    │
│                     │                                    │
│  ┌──────────────────┴──────────────────────────────┐    │
│  │              Brale Miden Service                  │    │
│  │  ┌─────────────────────────────────────────┐     │    │
│  │  │  integration crate (this demo)          │     │    │
│  │  │  ┌──────────┐  ┌────────────────────┐   │     │    │
│  │  │  │ BraleKey-│  │ operations.rs       │   │     │    │
│  │  │  │ store    │  │ mint / burn / xfer  │   │     │    │
│  │  │  └────┬─────┘  └────────────────────┘   │     │    │
│  │  │       │                                   │     │    │
│  │  │       ▼ ExternalSigner::sign_prehash()   │     │    │
│  │  └───────┬──────────────────────────────────┘     │    │
│  │          │                                         │    │
│  │  ┌───────┴──────────┐                              │    │
│  │  │ Blockdaemon BV   │  (MPC signing)               │    │
│  │  │ or Fireblocks    │                              │    │
│  │  └──────────────────┘                              │    │
│  └─────────────────────────────────────────────────┘    │
└───────────────────────────┬─────────────────────────────┘
                            │ gRPC
                            ▼
                ┌───────────────────────┐
                │     Miden Network     │
                │  Accounts · Notes     │
                │  STARK Verification   │
                └───────────────────────┘
```

**What Brale builds** (application layer):
- REST API gateway, authentication, rate limiting
- KYB/KYC verification (off-chain, business-to-account mapping)
- Idempotency key deduplication (PostgreSQL)
- Transfer status tracking and webhooks to customers
- OFAC/compliance screening before transaction submission

**What this demo provides** (Miden SDK integration layer):
- Account creation and token deployment
- Mint, burn, transfer, balance/supply reads
- Signer injection for MPC/HSM backends
- Deposit detection via `sync_state()` polling

**What Miden provides** (protocol layer):
- Account state management (code, storage, vault, nonce)
- Note creation, commitment, and consumption
- STARK proof generation and verification
- Max supply enforcement at protocol level

---

## Concept Mapping

| Brale Concept | Miden Equivalent | Notes |
|---|---|---|
| **Account** (`account_id`, KSUID) | App-layer entity | KYB is off-chain. Miden accounts are wallets, not business entities. Brale maintains its own `account_id → [miden_account_ids]` mapping. |
| **Address** (`type=internal`) | `Account` with `BasicWallet` + `AuthEcdsaK256Keccak` | One Miden account per custodial wallet. Created via `create_wallet_account()`. |
| **Address** (`type=external`) | P2ID note recipient via `AccountId` | External wallets receive tokens via P2ID notes. No Miden account creation needed — just the recipient's `AccountId`. |
| **Transfer** (mint) | `mint_tokens()` → P2ID note → `consume_notes()` | Two-step: faucet creates P2ID note, target consumes it. See [§3 Mint](05-rfi-requirements.md#3-mint-tokens). |
| **Transfer** (burn/redeem) | `burn_tokens()` → burn note → faucet consumes | Two-step: sender creates burn note, faucet consumes to reduce supply. See [§4 Burn](05-rfi-requirements.md#4-burn-tokens). |
| **Transfer** (payout/send) | `transfer_tokens()` → P2ID note → `consume_notes()` | Same P2ID pattern as mint. See [§5 Transfer](05-rfi-requirements.md#5-transfer-tokens). |
| **Transfer status** | See [Transaction Lifecycle](#transaction-lifecycle) below | Miden's note model maps cleanly to Brale's `pending → processing → complete` status. |
| **Idempotency key** | App-layer dedup | Not a protocol concept. Brale stores `idempotency_key → tx_id` in its backend database. Check before calling `submit_new_transaction()`. |
| **value_type** (e.g. `SBC`) | `TokenSymbol` on faucet | 1-4 uppercase ASCII chars, set at faucet creation via `deploy_issuer_account()`. |
| **decimals** | `decimals` param on `BasicFungibleFaucet` | Set at faucet creation. Max 12. Brale uses 6 for stablecoins. |
| **chain** | Always `miden` | Single-chain. Miden is the settlement layer. |
| **transfer_type** (e.g. `base`) | N/A | Miden has one chain — no cross-chain transfer types. All transfers are P2ID notes. |
| **Webhook** (deposit notification) | `sync_state()` polling | No push notifications yet. Poll every ~3 seconds (matching block time). See [Deposit Detection Pattern](#deposit-detection-pattern). |

---

## Transaction Lifecycle

Brale's API tracks transfer status as `pending → processing → complete`. Here is how
each status maps to Miden's transaction and note model.

### Status Mapping

| Brale Status | Miden State | What Happened | How to Detect |
|---|---|---|---|
| `pending` | Transaction submitted | `submit_new_transaction()` returned successfully. The STARK proof has been submitted to the Miden node via gRPC. | Return value from `submit_new_transaction()` (returns `TransactionId`). |
| `processing` | Note committed in block | The output note (P2ID or burn) has been included in a block. The recipient has not yet consumed it. | `sync_state()` returns the note in `committed_notes`. |
| `complete` | Note consumed by recipient | The recipient executed `consume_notes()`, consuming the note and updating their vault balance. | `sync_state()` returns the note in `consumed_notes`. |
| `failed` | Execution error | Transaction failed during local execution (e.g. insufficient balance, auth failure) or proof verification failed at the node. | Error returned from `submit_new_transaction()`. |
| `canceled` | Note expired | The note was not consumed before its expiration block height. | Note no longer appears in `get_consumable_notes()` after expiration. |

### Timing

| Event | Latency |
|---|---|
| `pending` → `processing` | ~3 seconds (one block) |
| `processing` → `complete` | Depends on recipient. For custodial wallets (Brale-controlled), immediate via `consume_notes()`. |
| Block proof generated | ~30 seconds after block inclusion |
| L1 publication | ~1 hour (batched) |

### Implementation Pattern

```rust
/// Track a transfer through its lifecycle.
async fn track_transfer_status(
    client: &mut Client<BraleKeystore>,
    sender_id: AccountId,
    recipient_id: AccountId,
    issuer_id: AccountId,
    amount: u64,
) -> Result<TransferStatus> {
    // Status: PENDING
    // Submit the transfer transaction (creates P2ID note)
    let tx_id = transfer_tokens(client, sender_id, recipient_id, issuer_id, amount).await?;
    // Store in DB: (idempotency_key, tx_id, status="pending")

    // Status: PROCESSING
    // Sync to detect note commitment in a block
    let summary = client.sync_state().await?;
    // When note appears in summary.committed_notes → update status to "processing"

    // Status: COMPLETE
    // For custodial wallets, consume immediately
    consume_notes(client, recipient_id).await?;
    // Update status to "complete"
    // For external wallets, poll until note appears in summary.consumed_notes

    Ok(TransferStatus::Complete)
}
```

### Idempotency Pattern

Brale's API requires an `Idempotency-Key` header on all POST requests. Since Miden
has no protocol-level idempotency, implement it at the application layer:

```rust
/// Before submitting any transaction, check the idempotency store.
async fn idempotent_mint(
    db: &Database,
    client: &mut Client<BraleKeystore>,
    idempotency_key: &str,
    issuer_id: AccountId,
    target_id: AccountId,
    amount: u64,
) -> Result<TransactionId> {
    // 1. Check if this key was already processed
    if let Some(existing_tx_id) = db.get_by_idempotency_key(idempotency_key).await? {
        return Ok(existing_tx_id);  // Return cached result
    }

    // 2. Execute the mint
    let tx_id = mint_tokens(client, issuer_id, target_id, amount).await?;

    // 3. Store the mapping
    db.insert_idempotency(idempotency_key, tx_id).await?;

    Ok(tx_id)
}
```

---

## Deposit Detection Pattern

Brale's platform detects incoming deposits to trigger downstream processing (crediting
user accounts, sending webhooks). On Miden, this is done by polling `sync_state()`.

```rust
/// Background loop: poll for new deposits every block (~3 seconds).
async fn deposit_detection_loop(
    client: &mut Client<BraleKeystore>,
    tracked_accounts: &[AccountId],
) {
    loop {
        let summary = client.sync_state().await.unwrap();

        for note in &summary.committed_notes {
            // A new note has been committed — check if it targets one of our accounts
            // In production: look up the note's target account, match against
            // tracked_accounts, and trigger deposit processing
        }

        // For custodial accounts, auto-consume to credit the balance
        for &account_id in tracked_accounts {
            let _ = consume_notes(client, account_id).await;
        }

        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }
}
```

**Key difference from Ethereum**: On Ethereum, deposit detection is passive (watch
for `Transfer` events). On Miden, the recipient must actively consume notes. For
custodial wallets (Brale-controlled), this is automatic. For external wallets, the
recipient handles consumption.

---

## Address Lifecycle

Brale's address model maps to Miden accounts:

### Internal Addresses (Custodial)

```
Brale: POST /addresses { type: "internal", account_id: "..." }
Miden: create_wallet_account(client, &pub_key) → Account

Brale stores: address_id → miden_account_id mapping
```

Each internal address is a Miden account with:
- `BasicWallet` component (send/receive)
- `AuthEcdsaK256Keccak` authentication (MPC-compatible)
- Private storage mode (balance not visible to network)

### External Addresses

```
Brale: POST /addresses { type: "external", address: "0x...", chain: "miden" }
Miden: No account creation needed — just store the AccountId
```

External addresses are just `AccountId` values used as P2ID note recipients. The
external party manages their own Miden account and note consumption.

---

## What Changes vs. Other Chains

Brale currently supports 21+ chains. Here is what is different about Miden:

| Aspect | Typical Chain (Ethereum, Solana) | Miden |
|---|---|---|
| Transaction execution | On-chain (validators re-execute) | Client-side (local execution + STARK proof) |
| Transfer model | Single atomic transaction | Two-step: create note → consume note |
| Deposit detection | Event logs / webhooks | Polling via `sync_state()` |
| Privacy | Public by default | Private accounts: only commitment on-chain |
| Signing | Standard ECDSA/EdDSA | Same ECDSA K256 — **no MPC changes needed** |
| Token identity | Contract address | Faucet account ID |
| Fees | Gas market | Currently zero; future: VM cycle-based |
| Finality | Probabilistic (confirmations) | Deterministic (~3s block, ~30s proof) |
| Account activation | Gas for deploy / rent | None required |
| Compliance | Contract-level (Chainalysis) | In development (faucet callbacks) |

---

[Back to Index](../WALKTHROUGH.md)
