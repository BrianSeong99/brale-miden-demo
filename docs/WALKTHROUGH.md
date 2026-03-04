# Miden + Brale: Regulated Stablecoin Issuance on Miden

Technical walkthrough for the Brale integration demo. This document explains how Miden
works (for engineers coming from Ethereum), how each RFI requirement is implemented in
working code, and how PSM enables institutional multisig custody.

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Miden for Ethereum Engineers](#2-miden-for-ethereum-engineers)
3. [Architecture](#3-architecture)
4. [Signer Injection: MPC/HSM Bridge](#4-signer-injection-mpchsm-bridge)
5. [PSM: Private Multisig Orchestration](#5-psm-private-multisig-orchestration)
6. [RFI Requirements Walkthrough](#6-rfi-requirements-walkthrough)
7. [Testing](#7-testing)
8. [What's Next](#8-whats-next)

---

## 1. Executive Summary

This demo proves that Brale's stablecoin operations — account custody, token issuance,
mint/burn/transfer, balance reads, and MPC signing — work on Miden today. The signing
layer uses ECDSA secp256k1 (the same curve as Ethereum), so Brale's existing
Blockdaemon BV / Sepior MPC infrastructure plugs in directly.

### RFI Coverage

| # | Requirement | Status | Code |
|---|-------------|--------|------|
| 1 | Signer injection (MPC/HSM) | **Working** | `keystore.rs` — `ExternalSigner` trait |
| 2 | Create wallet / account | **Working** | `operations.rs` — `create_wallet_account()` |
| 3 | Read token balance | **Working** | `operations.rs` — `read_balance()` |
| 4 | Read total supply | **Working** | `operations.rs` — `read_total_supply()` |
| 5 | Deploy / register fungible token | **Working** | `operations.rs` — `deploy_issuer_account()` |
| 6 | Mint (increase supply) | **Working** | `operations.rs` — `mint_tokens()` |
| 7 | Burn (reduce supply) | **Working** | `operations.rs` — `burn_tokens()` |
| 8 | Transfer (holder to recipient) | **Working** | `operations.rs` — `transfer_tokens()` |
| 9 | Compliance controls (denylist/freeze) | **Roadmap** | `docs/COMPLIANCE_ROADMAP.md` |
| 10 | Finality signal | **Documented** | Section 6.9 |
| 11 | Deposit detection | **Working** | `operations.rs` — `consume_notes()` + `sync_state()` |
| 12 | Fee estimation | **Roadmap** | Section 6.10 |
| 13 | Account activation / opt-in | **None required** | Section 6.11 |

### Quick Start

```bash
cp .env.example .env          # configure endpoints
make build                    # compile everything
make test                     # run 16 tests (no network needed)
make full-demo                # E2E: deploy → mint → transfer → burn
```

---

## 2. Miden for Ethereum Engineers

### 2.1 The 60-Second Mental Model

On Ethereum, every transaction re-executes on every validator. On Miden, the user
executes the transaction locally on their own machine, generates a zero-knowledge proof
(STARK) that the execution was correct, and submits the proof to the network. The
network verifies the proof without re-executing anything. This is what makes privacy
and parallel execution possible.

Think of it this way: on Ethereum, the network runs your code. On Miden, you run your
code and prove you did it correctly.

### 2.2 Accounts

On Ethereum, there are two kinds of accounts: EOAs (externally owned, just a key) and
smart contracts (code + storage). On Miden, every account is both — it always has code,
storage, a vault (for holding assets), and a nonce. There is no EOA/contract split.

An account has four components:

```
Account
├── ID         120-bit unique identifier (encodes account type + storage mode)
├── Code       Immutable procedures (the account's API)
├── Storage    Up to 256 key-value slots (sparse Merkle tree)
├── Vault      Holds fungible and non-fungible assets (up to 256 different tokens)
└── Nonce      Monotonically increasing counter (replay protection)
```

**Account types** relevant to Brale:

- **Regular Account** — a wallet that can hold and send tokens. Equivalent to an
  EOA + smart wallet on Ethereum.
- **Fungible Faucet** — an issuer account that can mint and burn a fungible token.
  Equivalent to deploying an ERC-20 contract on Ethereum, except the token is a
  native protocol asset rather than a storage mapping in a contract.

**Storage modes:**

- **Public** — full account state stored on-chain (like Ethereum). Required for
  faucets / issuers so anyone can verify supply.
- **Private** — only a 40-byte commitment stored on-chain. The actual state lives
  with the account owner. This is how Miden achieves privacy.

### 2.3 Notes (Miden's UTXO Model)

Ethereum uses an account-balance model: `balanceOf[address] += amount` in one
atomic transaction. Miden uses a note-based model (similar to Bitcoin's UTXOs, but
programmable):

1. Sender creates a **note** containing assets and a script
2. Note is recorded on the network
3. Recipient discovers the note via `sync_state()`
4. Recipient **consumes** the note in a separate transaction, executing the note's
   script which transfers assets into the recipient's vault

This two-step model is fundamental. Every operation that moves tokens — mint, transfer,
burn — creates a note that must be consumed.

**Note types used in this demo:**

| Note Type | What It Does | Ethereum Analog |
|-----------|-------------|-----------------|
| **P2ID** (Pay-to-ID) | Sends assets to a specific account ID. Only that account can consume it. | `ERC20.transfer(to, amount)` |
| **Burn** | Sends assets back to the issuing faucet. The faucet's burn procedure decrements total supply. | `ERC20.burn(amount)` |

**Why notes, not direct transfers?** Notes enable privacy (the network only sees
commitments, not amounts or recipients for private notes), parallel execution (notes
are independent), and programmability (note scripts can enforce arbitrary conditions).

### 2.4 Client-Side Execution and STARK Proofs

On Ethereum:
```
User signs tx → submits to mempool → every validator re-executes → consensus
```

On Miden:
```
User builds tx locally → executes on Miden VM → generates STARK proof → submits
proof to network → network verifies proof (no re-execution) → block inclusion
```

The STARK proof is a cryptographic guarantee that the transaction was executed
correctly. The network never sees the transaction's inputs, execution trace, or
private state — only the proof and the resulting state commitment.

**What this means for Brale:**
- Transaction execution happens on your infrastructure (Rust SDK, ~1-2 seconds)
- Signing happens during execution via the `TransactionAuthenticator` callback
- The proof is generated automatically by the SDK
- Submission is a single gRPC call to the Miden node

### 2.5 Faucets: Native Token Issuance

On Ethereum, a stablecoin is a smart contract with a `mapping(address => uint256)`.
On Miden, a stablecoin is a **faucet account** — a special account type that can
create and destroy fungible assets.

Key properties:
- **Token identity = faucet account ID**. The asset carries the issuer's ID, not a
  contract address.
- **Max supply** is set at faucet creation and enforced at the protocol level.
- **Total issuance** is tracked in the faucet's reserved `sysdata` storage slot,
  updated automatically by the kernel on every mint/burn.
- **No ERC-20 interface needed**. Mint, burn, transfer, and balance reads are
  protocol-native operations.

### 2.6 Privacy Model

| Component | Public Mode | Private Mode |
|-----------|-------------|--------------|
| Account state | Full state on-chain | Only commitment hash on-chain (40 bytes) |
| Notes | Full note data on-chain | Only commitment on-chain |
| Transaction | Proof + state delta visible | Proof + state delta visible |

Faucets (issuers) should be **public** — total supply must be auditable.
User wallets can be **private** — balances are not visible to the network.

### 2.7 Concept Mapping: Ethereum to Miden

| Ethereum | Miden | Key Difference |
|----------|-------|----------------|
| EOA / Smart Wallet | Account | All accounts have code; no EOA/contract split |
| ERC-20 Contract | Faucet Account | Token is a native asset, not a contract mapping |
| Contract deployment | Account creation | Params set at build time via `AccountBuilder` |
| `transfer(to, amount)` | P2ID note creation + consumption | Two-step: sender creates note, recipient consumes |
| `balanceOf(address)` | `vault().get_balance(faucet_id)` | Local state read, not a contract call |
| `totalSupply()` | Faucet sysdata storage slot | Read from faucet's reserved slot |
| Events / Logs | Notes | Notes ARE the transfer, not a side-effect of it |
| `msg.sender` | `TransactionAuthenticator` | Proven via STARK proof, not checked at runtime |
| Gas / `estimateGas` | Client-side proving cost | No gas market; computation cost is local |
| Block confirmations | Proof verification | Proof verified = included (no reorgs currently) |
| Nonce | Account nonce | Same: monotonically incrementing, prevents replay |
| `keccak256` | Poseidon2 (internal) | External signers still receive a keccak256 digest |
| MPC / Fireblocks | `ExternalSigner` trait | Same secp256k1 curve; identical key material |
| `approve` + `transferFrom` | Direct P2ID note | No approval step needed |
| Mempool | Local execution | Transactions built and proven locally before submission |

---

## 3. Architecture

### 3.1 System Diagram

```
                         ┌──────────────────────┐
                         │  Brale Application   │
                         └──────────┬───────────┘
                                    │
                 ┌──────────────────┼──────────────────┐
                 │                                     │
    ┌────────────▼────────────┐          ┌─────────────▼────────────┐
    │  Client<BraleKeystore>  │          │   PSM MultisigClient     │
    │  (single-signer path)   │          │   (multisig path)        │
    └────────────┬────────────┘          └─────────────┬────────────┘
                 │                                     │
    ┌────────────▼────────────┐          ┌─────────────▼────────────┐
    │     BraleKeystore       │          │   BraleMpcKeyManager     │
    │  TransactionAuthenticator│         │   PSM KeyManager trait   │
    └────────┬────────────────┘          └─────────────┬────────────┘
             │                                         │
             └──────────────┬──────────────────────────┘
                            │
               ┌────────────▼────────────┐
               │     ExternalSigner      │
               │  sign_prehash(digest)   │
               │  → [u8; 65] ECDSA sig   │
               └────────────┬────────────┘
                            │
              ┌─────────────┼─────────────┐
              │             │             │
    ┌─────────▼──┐  ┌──────▼──────┐  ┌───▼──────────┐
    │ Simulated  │  │ Blockdaemon │  │  Fireblocks  │
    │ MpcSigner  │  │ BV (prod)   │  │  HSM (prod)  │
    │ (testing)  │  │             │  │              │
    └────────────┘  └─────────────┘  └──────────────┘
```

Both the single-signer path (`Client<BraleKeystore>`) and the multisig path
(`PSM MultisigClient`) route signing through the same `ExternalSigner` interface.
This means a single MPC backend serves both patterns.

### 3.2 Component Inventory

| File | Purpose | Lines |
|------|---------|-------|
| `src/keystore.rs` | `ExternalSigner` trait + `BraleKeystore` (signing bridge) | 181 |
| `src/mock_signer.rs` | `SimulatedMpcSigner` (test double for MPC) | 75 |
| `src/operations.rs` | All token operations (deploy, mint, burn, transfer, reads) | 351 |
| `src/multisig.rs` | PSM `KeyManager` bridge (`BraleMpcKeyManager`) | 165 |
| `src/config.rs` | Environment-based configuration | 42 |
| `src/helpers.rs` | Template helpers (do not modify) | 364 |
| `src/bin/*.rs` | 9 executable binaries | ~600 |
| `tests/*.rs` | 16 passing tests (integration + unit) | ~250 |

### 3.3 Signing Patterns

| Pattern | Use Case | Auth Component | Client Type |
|---------|----------|----------------|-------------|
| **Single signer** | Brale-custodied wallet, single MPC key | `AuthEcdsaK256Keccak` | `Client<BraleKeystore>` |
| **PSM Multisig** | Treasury, governance, multi-party custody | PSM `multisig_ecdsa` | `MultisigClient` |

Both use ECDSA secp256k1 (K256). Both are compatible with Blockdaemon BV / Sepior.

---

## 4. Signer Injection: MPC/HSM Bridge

This is the core integration point for Brale's existing custody infrastructure.
The design goal: **Blockdaemon BV should work without understanding Miden internals.**

### 4.1 The ExternalSigner Trait

The entire MPC integration surface is one async method:

```rust
// integration/src/keystore.rs

#[async_trait]
pub trait ExternalSigner: Send + Sync {
    async fn sign_prehash(
        &self,
        pub_key_commitment: PublicKeyCommitment,  // which key to use
        digest: [u8; 32],                         // Keccak256 hash to sign
    ) -> Result<[u8; 65], ExternalSignerError>;   // r[32] || s[32] || v[1]
}
```

The MPC provider receives:
- A key identifier (`PublicKeyCommitment` — a Poseidon2 hash of the public key)
- A standard 32-byte Keccak256 digest (identical to what Ethereum signers handle)

It returns:
- A 65-byte signature: `r` (32 bytes, big-endian) + `s` (32 bytes, big-endian) +
  `v` (1 byte, recovery ID 0 or 1)

This is the same format as Ethereum `eth_sign`. No Miden-specific encoding is needed
from the MPC provider.

### 4.2 BraleKeystore: How It Plugs Into the Miden SDK

The Miden SDK's `Client<K>` is generic over an authenticator type `K`. The SDK
requires `K` to implement `TransactionAuthenticator + From<FilesystemKeyStore>`.

`BraleKeystore` satisfies both:

```rust
// integration/src/keystore.rs

pub struct BraleKeystore {
    inner: FilesystemKeyStore,              // key storage (delegated)
    signer: Option<Arc<dyn ExternalSigner>>, // signing (overridden)
}

// Required by ClientBuilder
impl From<FilesystemKeyStore> for BraleKeystore {
    fn from(inner: FilesystemKeyStore) -> Self {
        Self::passthrough(inner)  // no external signer
    }
}

// Transaction signing
impl TransactionAuthenticator for BraleKeystore {
    fn get_signature(&self, pub_key_commitment, signing_inputs) -> ... {
        // When external signer is configured:
        //   1. Extract commitment from signing inputs
        //   2. Hash to Keccak256
        //   3. Call ExternalSigner::sign_prehash
        //   4. Parse response into Miden signature type
        //
        // When no external signer:
        //   Delegate to inner FilesystemKeyStore (local signing)
    }
}
```

The `BraleKeystore` wraps the SDK's built-in `FilesystemKeyStore`:
- **Key management** (storage, lookup, account mappings) is fully delegated
- **Signing** is overridden to route through the external backend when configured
- **Fallback** to local signing when no external signer is set

### 4.3 The Signing Chain (Byte-Level)

When the Miden SDK needs a signature during transaction execution, this is what
happens inside `BraleKeystore::get_signature()`:

```
TransactionAuthenticator::get_signature(commitment, signing_inputs)
│
├─ 1. COMMITMENT EXTRACTION
│     signing_inputs.to_commitment()
│     → Word (4 field elements, Poseidon2 hash of tx summary)
│     This is the canonical "what you're signing"
│
├─ 2. KECCAK256 HASHING
│     word_bytes: [u8; 32] = Word::into()
│     → Each field element → u64 → 8 bytes little-endian → 32 bytes total
│     digest: [u8; 32] = Keccak256(word_bytes)
│     → Standard 32-byte hash (same algorithm as Ethereum)
│
├─ 3. EXTERNAL SIGNING (sent to MPC/HSM)
│     signer.sign_prehash(pub_key_commitment, digest)
│     → MPC provider signs the 32-byte digest
│     → Returns [u8; 65] = r[32] || s[32] || v[1]
│
├─ 4. SIGNATURE RECONSTRUCTION
│     parse_ecdsa_signature(&sig_bytes)
│     → Signature::from_sec1_bytes_and_recovery_id(r||s, v)
│     → ecdsa_k256_keccak::Signature
│
└─ 5. RETURN TO SDK
      Signature::EcdsaK256Keccak(sig)
      → SDK encodes as 26 packed-u32 felts for VM advice stack
      → VM verifies signature against the account's public key
```

The critical insight: **the MPC provider never handles Miden field elements,
Poseidon2 hashes, or VM-specific encoding.** It receives a standard Keccak256
digest and returns a standard ECDSA signature. This is intentional — it means
Brale's existing Blockdaemon BV integration works unchanged.

### 4.4 SimulatedMpcSigner (Test Double)

For testing, `SimulatedMpcSigner` implements `ExternalSigner` using local keys:

```rust
// integration/src/mock_signer.rs

pub struct SimulatedMpcSigner {
    keys: Mutex<BTreeMap<PublicKeyCommitment, SecretKey>>,
}
```

It holds a mapping of key commitments to secret keys. When `sign_prehash` is called,
it looks up the secret key and signs locally using ECDSA K256 with RFC 6979
deterministic nonce generation (same as Ethereum).

In production, replace `SimulatedMpcSigner` with an HTTP client that calls
Blockdaemon BV's signing API. The trait interface is the same.

### 4.5 Production Integration: What Changes

| Component | This Demo | Production |
|-----------|-----------|------------|
| `ExternalSigner` impl | `SimulatedMpcSigner` (local keys) | HTTP client to Blockdaemon BV |
| Key storage | `FilesystemKeyStore` (local disk) | HSM-backed key storage |
| Key generation | `SecretKey::new()` (random) | MPC key generation ceremony |
| Client state | SQLite file | PostgreSQL / managed database |
| RPC endpoint | Testnet (`rpc.testnet.miden.io`) | Production node |

The `ExternalSigner` trait, `BraleKeystore`, and all operations code remain
unchanged. Only the signer implementation and infrastructure config change.

---

## 5. PSM: Private Multisig Orchestration

### 5.1 What PSM Solves

PSM (Private State Manager) is an off-chain coordination layer built by OpenZeppelin
for Miden multisig operations. It solves the problem: "How do N parties coordinate
to sign a single Miden transaction when each party has their own key?"

On Ethereum, multisig is typically an on-chain contract (like Gnosis Safe) that
collects signatures and executes when threshold is met. On Miden, the account's
authentication procedure verifies a threshold of ECDSA signatures, but the
coordination of collecting those signatures happens off-chain via PSM.

PSM provides:
- **Delta proposals** — one signer proposes a transaction, others review and sign
- **Signature collection** — PSM server collects signatures until threshold is met
- **Advice construction** — aggregates signatures into the format the Miden VM expects
- **Execution** — any party can submit the final transaction once threshold is met

### 5.2 KeyManager Bridge

PSM has its own signing abstraction — the `KeyManager` trait — separate from
Miden's `TransactionAuthenticator`. `BraleMpcKeyManager` bridges the two so that
the same MPC backend serves both single-signer and multisig paths:

```rust
// integration/src/multisig.rs

pub struct BraleMpcKeyManager {
    secret_key: ecdsa_k256_keccak::SecretKey,
    public_key: ecdsa_k256_keccak::PublicKey,
    commitment: Word,
    signer: Arc<dyn ExternalSigner>,  // same MPC backend
}

impl KeyManager for BraleMpcKeyManager {
    fn scheme(&self) -> SignatureScheme { SignatureScheme::Ecdsa }

    fn sign_hex(&self, message: Word) -> String {
        // Bridge: PSM's sync sign_hex → our async ExternalSigner
        let digest = keccak256_hash_word(message);
        let sig = block_in_place(|| {
            signer.sign_prehash(commitment, digest).await
        });
        hex::encode(sig)
    }

    fn public_key_hex(&self) -> Option<String> {
        Some(hex::encode(self.public_key.to_bytes()))
    }
}
```

The `sign_hex` method bridges PSM's synchronous API to our async `ExternalSigner`
using `tokio::task::block_in_place`. This is necessary because PSM's `KeyManager`
trait is synchronous, but MPC backends are inherently async (network calls).

### 5.3 2-of-3 ECDSA Multisig Flow

```
Signer 1 (Proposer)         PSM Server            Signer 2            Signer 3
────────────────────         ──────────            ────────            ────────
       │                          │                    │                   │
 Generate 3 ECDSA K256            │                    │                   │
 keypairs (secp256k1)             │                    │                   │
       │                          │                    │                   │
 create_account(                  │                    │                   │
   threshold=2,                   │                    │                   │
   signers=[pk1,pk2,pk3]         │                    │                   │
 )                                │                    │                   │
       │                          │                    │                   │
 propose(P2ID transfer) ──────►  Store proposal        │                   │
       │                     Broadcast ──────────► Receive                 │
       │                          │        ──────────────────────────► Receive
       │                          │                    │                   │
 sign(proposal) ──────────────►  Collect sig #1        │                   │
       │                          │                    │                   │
       │                          │     sign(proposal) ──► Collect sig #2  │
       │                          │                    │                   │
       │                     Threshold met (2/3)       │                   │
       │                     Aggregate signatures      │                   │
       │                          │                    │                   │
 execute() ◄──────────────── Build final tx            │                   │
       │                     Submit to Miden           │                   │
       │                          │                    │                   │
 Transaction included             │                    │                   │
```

### 5.4 Running the Multisig Demo

The multisig demo requires a running PSM server:

```bash
# Terminal 1: Start PSM server
git clone https://github.com/OpenZeppelin/private-state-manager
cd private-state-manager
cargo run -p private-state-manager-server
# Default: http://localhost:50051

# Terminal 2: Run demo
make multisig-demo
```

The demo (`integration/src/bin/multisig_demo.rs`):
1. Generates 3 ECDSA K256 keypairs
2. Builds a `MultisigClient` connected to both Miden RPC and PSM
3. Creates a 2-of-3 multisig account
4. Demonstrates that 2 signers can produce valid signatures

---

## 6. RFI Requirements Walkthrough

Each subsection maps one Brale RFI requirement to working code.

---

### 6.1 Create Wallet / Account

**RFI**: "Create wallet / account (Brale-custodied keys)"

**Ethereum equivalent**: EOA creation via key derivation, or `CREATE2` for smart
wallets.

**How it works on Miden**: Generate an ECDSA K256 keypair, derive the public key
commitment (Poseidon2 hash), and build an account with `AccountBuilder`. The account
is added to the client's local store. No on-chain transaction is needed — the account
exists once it has an ID derived from its initial state.

**Code** (`integration/src/operations.rs`):

```rust
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
        .build()?;

    client.add_account(&account, false).await?;
    Ok(account)
}
```

**What each line does:**
1. Generate a random 32-byte seed (deterministically derives the account ID)
2. Derive a `PublicKeyCommitment` from the ECDSA public key
3. Build the account with:
   - `RegularAccountImmutableCode` — standard wallet, code cannot change after creation
   - `Private` storage — only a commitment stored on-chain (balance not visible)
   - `BasicWallet` component — standard send/receive procedures
   - `AuthEcdsaK256Keccak` — ECDSA secp256k1 signature verification
4. Add the account to the client's local SQLite store

**Test**: `create_ecdsa_wallet` in `tests/single_signer_test.rs` — verifies account
creation with ECDSA K256 auth, asserts nonce is non-zero.

**CLI**:
```bash
make create-account
```

**Production notes**: In production, the ECDSA keypair would be generated by
Blockdaemon BV's MPC ceremony. Only the public key is needed for account creation.

---

### 6.2 Deploy / Register Fungible Token

**RFI**: "Deploy / register a new fungible token"

**Ethereum equivalent**: Deploying an ERC-20 contract with constructor args
(name, symbol, decimals, initial supply).

**How it works on Miden**: Create a faucet account with `BasicFungibleFaucet` component.
The faucet's account ID becomes the token's identity. Token symbol, decimals, and max
supply are set at creation.

**Code** (`integration/src/operations.rs`):

```rust
pub async fn deploy_issuer_account(
    client: &mut Client<BraleKeystore>,
    pub_key: &ecdsa_k256_keccak::PublicKey,
    symbol: &str,        // e.g. "USDB"
    decimals: u8,        // e.g. 6
    max_supply: u64,     // e.g. 1_000_000_000_000
) -> Result<Account> {
    let token_symbol = TokenSymbol::new(symbol)?;
    let pub_key_commitment = PublicKeyCommitment::from(pub_key.to_commitment());

    let faucet_component = BasicFungibleFaucet::new(
        token_symbol, decimals, Felt::new(max_supply),
    )?;

    let account = AccountBuilder::new(init_seed)
        .account_type(AccountType::FungibleFaucet)
        .storage_mode(AccountStorageMode::Public)  // supply must be auditable
        .with_component(faucet_component)
        .with_auth_component(AuthEcdsaK256Keccak::new(pub_key_commitment))
        .build()?;

    client.add_account(&account, false).await?;
    Ok(account)
}
```

**Key differences from ERC-20 deployment:**
- `AccountType::FungibleFaucet` — the account type encodes that this is a token issuer
- `StorageMode::Public` — the faucet's state (including total supply) is on-chain and auditable
- `max_supply` is enforced at the protocol level — minting beyond it fails with
  `ERR_FUNGIBLE_ASSET_DISTRIBUTE_WOULD_CAUSE_MAX_SUPPLY_TO_BE_EXCEEDED`
- Token symbol is 1-4 uppercase ASCII characters
- Decimals can be up to 12 (Brale recommends 6)

**Test**: `deploy_issuer_account` in `tests/single_signer_test.rs` — verifies faucet
creation with ECDSA auth.

**CLI**:
```bash
make deploy-issuer
# Optional args: cargo run -p integration --bin deploy_issuer -- USDB 6 1000000000000
```

---

### 6.3 Mint Tokens

**RFI**: "Mint (increase supply, deliver to address)"

**Ethereum equivalent**: `ERC20.mint(to, amount)` — increases total supply and credits
the target address.

**How it works on Miden**: The issuer (faucet) executes a transaction that creates a
P2ID (pay-to-id) note containing the newly minted tokens. The target account then
consumes the note to receive the tokens. Minting is a two-step process.

```
Issuer (Faucet)              Miden Network               Target Wallet
───────────────              ────────────                 ─────────────
      │                            │                            │
 build_mint_fungible_asset()       │                            │
 → creates P2ID note               │                            │
 → local execution + proof         │                            │
      │                            │                            │
 submit_new_transaction() ──────►  Verify proof                 │
                                   Supply += amount             │
                                   Note recorded                │
                                       │                        │
                                   sync_state() ─────────►  Discover note
                                                                │
                                                         consume_notes()
                                                         → local execution
                                                                │
                                   Verify proof  ◄────────  Submit tx
                                   Note consumed                │
                                                         Vault += amount
```

**Code** (`integration/src/operations.rs`):

```rust
pub async fn mint_tokens(
    client: &mut Client<BraleKeystore>,
    issuer_id: AccountId,
    target_id: AccountId,
    amount: u64,
) -> Result<()> {
    let asset = FungibleAsset::new(issuer_id, amount)?;

    let tx_request = TransactionRequestBuilder::new()
        .build_mint_fungible_asset(asset, target_id, NoteType::Private, client.rng())?;

    client.submit_new_transaction(issuer_id, tx_request).await?;
    Ok(())
}
```

**What happens inside `submit_new_transaction`:**
1. The SDK builds a transaction program for the faucet account
2. The faucet's `distribute` procedure is called (mints new tokens)
3. A P2ID note is created as output (containing the minted tokens)
4. The `TransactionAuthenticator` callback fires — `BraleKeystore::get_signature()`
5. The ECDSA signature is generated via `ExternalSigner::sign_prehash()`
6. A STARK proof is generated
7. The transaction + proof are submitted to the Miden node via gRPC

**After minting, the target must consume the note:**

```rust
operations::consume_notes(&mut client, target_id).await?;
```

**Test**: `mint_and_consume` in `tests/single_signer_test.rs` — mints 100 tokens,
consumes at target, verifies balance is 100.

**CLI**:
```bash
ISSUER_ACCOUNT_ID=<id> cargo run -p integration --bin mint -- <target_id> 1000
```

---

### 6.4 Burn Tokens

**RFI**: "Burn (reduce supply from an address)"

**Ethereum equivalent**: `ERC20.burn(amount)` or `ERC20.burnFrom(address, amount)`.

**How it works on Miden**: Burning uses a dedicated **burn note** (not a simple
transfer to the faucet). The burn note's script calls the faucet's `burn` procedure,
which validates the asset and decrements total issuance. This is a two-transaction
flow:

```
Sender (Holder)              Miden Network               Issuer (Faucet)
───────────────              ────────────                 ───────────────
      │                            │                            │
 create_burn_note()                │                            │
 → output note targeting faucet    │                            │
 → local execution + proof         │                            │
      │                            │                            │
 submit_new_transaction() ──────►  Verify proof                 │
                                   Note recorded                │
                                       │                        │
                                   sync_state() ─────────►  Discover note
                                                                │
                                                         consume_notes()
                                                         → burn procedure
                                                         → supply -= amount
                                                                │
                                   Verify proof  ◄────────  Submit tx
                                   Note consumed                │
                                                         Supply decreased
```

**Code** (`integration/src/operations.rs`):

```rust
pub async fn burn_tokens(
    client: &mut Client<BraleKeystore>,
    sender_id: AccountId,
    issuer_id: AccountId,
    amount: u64,
) -> Result<()> {
    let asset = FungibleAsset::new(issuer_id, amount)?;

    // Step 1: Sender creates a burn note targeting the issuer
    let burn_note = create_burn_note(
        sender_id, issuer_id, asset.into(),
        NoteAttachment::default(), client.rng(),
    )?;

    let tx_request = TransactionRequestBuilder::new()
        .own_output_notes(vec![OutputNote::Full(burn_note)])
        .build()?;
    client.submit_new_transaction(sender_id, tx_request).await?;

    // Step 2: Issuer consumes the burn note (reduces supply)
    client.sync_state().await?;
    let consumable = client.get_consumable_notes(Some(issuer_id)).await?;
    let notes: Vec<_> = consumable.into_iter()
        .filter_map(|(record, _)| record.try_into().ok()).collect();

    let tx_request = TransactionRequestBuilder::new()
        .build_consume_notes(notes)?;
    client.submit_new_transaction(issuer_id, tx_request).await?;

    Ok(())
}
```

**Why a special burn note?** The burn note's script calls `faucets::burn`, which:
- Validates the asset was issued by the executing faucet
- Decrements the faucet's `TOTAL_ISSUANCE` in the sysdata storage slot
- Removes the asset from the input vault (tokens are destroyed)
- Creates no output notes

This is different from Ethereum where `burn()` is just a contract function call.
On Miden, the note-based model means the holder creates a note that, when consumed
by the faucet, triggers the supply reduction.

**Test**: `faucet_sysdata_slot_readable` in `tests/single_signer_test.rs` — verifies
that the sysdata slot (where total issuance is stored) is accessible and starts at 0.

**CLI**:
```bash
ISSUER_ACCOUNT_ID=<id> cargo run -p integration --bin burn -- <sender_id> 100
```

---

### 6.5 Transfer Tokens

**RFI**: "Transfer (holder to recipient)"

**Ethereum equivalent**: `ERC20.transfer(to, amount)`

**How it works on Miden**: Sender creates a P2ID (pay-to-id) note containing the
tokens. The recipient syncs, discovers the note, and consumes it in a separate
transaction. Two transactions, two STARK proofs.

**Code** (`integration/src/operations.rs`):

```rust
pub async fn transfer_tokens(
    client: &mut Client<BraleKeystore>,
    sender_id: AccountId,
    recipient_id: AccountId,
    issuer_id: AccountId,
    amount: u64,
) -> Result<()> {
    let asset = FungibleAsset::new(issuer_id, amount)?;
    let payment = PaymentNoteDescription::new(
        vec![asset.into()], sender_id, recipient_id,
    );

    let tx_request = TransactionRequestBuilder::new()
        .build_pay_to_id(payment, NoteType::Private, client.rng())?;

    client.submit_new_transaction(sender_id, tx_request).await?;
    Ok(())
}
```

**After the sender submits, the recipient must consume:**

```rust
// Recipient side
operations::consume_notes(&mut client, recipient_id).await?;
```

**P2ID note enforcement**: The P2ID note script checks that the consuming account's
ID matches the target. If account B tries to consume a note addressed to account A,
execution fails with `ERR_P2ID_TARGET_ACCT_MISMATCH`.

**Test**: `consume_multiple_notes` in `tests/single_signer_test.rs` — verifies that
two P2ID notes (100 + 50 tokens) can be consumed in a single transaction, resulting
in balance 150.

**CLI**:
```bash
ISSUER_ACCOUNT_ID=<id> cargo run -p integration --bin transfer -- <sender> <recipient> 300
```

---

### 6.6 Read Token Balance

**RFI**: "Read token balance for an address"

**Ethereum equivalent**: `ERC20.balanceOf(address)` — reads the balance mapping in
the contract's storage.

**How it works on Miden**: Read the account's vault, which stores balances for all
tokens held by the account. The vault is keyed by faucet ID (token identity).

**Code** (`integration/src/operations.rs`):

```rust
pub async fn read_balance(
    client: &Client<BraleKeystore>,
    account_id: AccountId,
    issuer_id: AccountId,
) -> Result<u64> {
    let record = client.get_account(account_id).await?
        .context("account not found")?;

    let account = match record.account_data() {
        AccountRecordData::Full(acc) => acc,
        AccountRecordData::Partial(_) => bail!("account is only partially tracked"),
    };

    let balance = account.vault().get_balance(issuer_id)?;
    Ok(balance)
}
```

**Important caveat**: For **private accounts**, the balance is only readable by the
account owner (who has the full account state locally). The network only stores a
commitment hash. For **public accounts**, balance can be read by querying the node.

This demo reads balances from the local client state. In production, Brale would track
its custodied accounts locally and read balances from its own client instance.

**Test**: `read_balance_after_mint` in `tests/single_signer_test.rs` — mints 500,
consumes, reads balance via `vault().get_balance()`, asserts equals 500.

**CLI**:
```bash
ISSUER_ACCOUNT_ID=<id> cargo run -p integration --bin read_balance -- <account_id>
```

---

### 6.7 Read Total Supply

**RFI**: "Read total supply of a token"

**Ethereum equivalent**: `ERC20.totalSupply()` — reads the contract's total supply
variable.

**How it works on Miden**: The faucet's total issuance is stored in a reserved
storage slot (`faucet_sysdata_slot`). Since faucets are public accounts, this
data is on-chain and auditable by anyone.

**Code** (`integration/src/operations.rs`):

```rust
pub async fn read_total_supply(
    client: &Client<BraleKeystore>,
    issuer_id: AccountId,
) -> Result<u64> {
    let record = client.get_account(issuer_id).await?
        .context("issuer account not found")?;

    let account = match record.account_data() {
        AccountRecordData::Full(acc) => acc,
        AccountRecordData::Partial(_) => bail!("issuer is only partially tracked"),
    };

    let issuance_slot = account.storage()
        .get_item(AccountStorage::faucet_sysdata_slot())?;

    Ok(issuance_slot[0].as_int())
}
```

The sysdata slot stores a `Word` (4 field elements). Element `[0]` is the total
issuance amount. Elements `[1..3]` are reserved.

**Test**: `faucet_sysdata_slot_readable` in `tests/single_signer_test.rs` — deploys
a fresh faucet, reads sysdata slot, asserts total supply starts at 0.

**CLI**:
```bash
ISSUER_ACCOUNT_ID=<id> make read-supply
```

---

### 6.8 Deposit Detection / Note Consumption

**RFI**: "Deposit detection / eventing: subscribe to inbound transfers for a
whitelisted set of tokens + addresses"

**Ethereum equivalent**: Listening for `Transfer` events via `eth_getLogs` or
WebSocket subscriptions.

**How it works on Miden**: The client calls `sync_state()` to poll the Miden node
for new notes addressed to tracked accounts. Discovered notes are then consumed
in a follow-up transaction.

**Code** (`integration/src/operations.rs`):

```rust
pub async fn consume_notes(
    client: &mut Client<BraleKeystore>,
    account_id: AccountId,
) -> Result<()> {
    // 1. Sync with network — discover new notes
    client.sync_state().await?;

    // 2. Get consumable notes for this account
    let consumable = client.get_consumable_notes(Some(account_id)).await?;
    if consumable.is_empty() { return Ok(()); }

    // 3. Build and submit consume transaction
    let notes: Vec<_> = consumable.into_iter()
        .filter_map(|(record, _)| record.try_into().ok()).collect();
    let tx_request = TransactionRequestBuilder::new()
        .build_consume_notes(notes)?;
    client.submit_new_transaction(account_id, tx_request).await?;

    Ok(())
}
```

**Polling model**: Miden does not currently support WebSocket push notifications
for new notes. The client polls via `sync_state()`. Each sync returns:
- `committed_notes` — notes that were committed in new blocks
- `consumed_notes` — notes that were spent
- `block_num` — the latest synced block number

**Production approach**: Run a background loop calling `sync_state()` on an
interval (e.g. every 3 seconds, matching block time). When new notes are
discovered, trigger deposit processing.

```rust
pub async fn sync_and_track(client: &mut Client<BraleKeystore>) -> Result<()> {
    let summary = client.sync_state().await?;
    // summary.committed_notes — new deposits
    // summary.consumed_notes — completed transactions
    // summary.block_num — latest block
    Ok(())
}
```

**Production notes**: Webhook-based deposit notifications are not yet available.
The polling model works but requires Brale to run a sync loop.

---

### 6.9 Finality Model

**RFI**: "Finality: deterministic finality signal & rule of thumb"

**Current model** (centralized, pre-mainnet):

| Stage | Timing | What It Means |
|-------|--------|---------------|
| Transaction submitted | Immediate | Proof sent to node via gRPC |
| Block inclusion | ~3 seconds | Node validates proof, includes in block |
| Block proof generated | ~30 seconds | STARK proof of block correctness |
| Proof published to Ethereum L1 | ~1 hour | Sliding window; batched with other blocks |

**Recommended policies:**

- **UI confirmation**: On block inclusion (~3 seconds). The transaction is validated
  and the proof has been verified by the Miden operator. This is the earliest point
  at which Brale should show "confirmed" to users.

- **Accounting finality**: After block proof generation (~30 seconds). At this point,
  the block itself has been cryptographically proven correct and is extremely unlikely
  to be reverted. Alternatively, wait for L1 publication (~1 hour) for the strongest
  guarantee.

**No reorgs**: Miden currently operates with a centralized operator. There are no
competing block producers, so reorgs do not occur. This will evolve as
decentralization progresses.

**Transaction failure detection**: If a transaction is not included by its
expiration block height, it is considered dropped. The Miden client handles this
via configurable timeout.

---

### 6.10 Fee Estimation / Simulation

**RFI**: "Fee estimation / simulation — pre-flight for tx construction"

**Current status**: Fees are in development. Currently, only the number of VM cycles
is used to compute the fee, and the fee parameters are set such that **fees are
effectively zero** for the time being.

The transaction kernel computes the fee automatically at the end of execution based
on cycle count and deducts it from the account's vault. There is no need to set
fees explicitly or estimate gas.

**When fees become material:**
- Fee will be computed as a logarithmic function of VM cycle count
- "Pay in any token" is planned but not live yet
- The error for insufficient fees is `TransactionExecutorError::InsufficientFee`

**Production notes**: No action needed from Brale in the current phase. When fees
become non-trivial, the SDK will provide estimation APIs.

---

### 6.11 Account Activation / Token Opt-In

**RFI**: "Account activation / token opt-in requirements (association / trustlines /
rent / min-balance / etc)"

**Answer: None required.**

- Tokens can be sent to any account ID, whether the account exists or not
- No rent or minimum balance is enforced by the protocol
- No token "opt-in" or "association" is needed (unlike Stellar trustlines or
  Solana token accounts)
- The only requirement is that the recipient must eventually consume the note
  to receive the tokens. This is automatic via `sync_state()` + `consume_notes()`

---

### 6.12 Compliance Controls (Denylist, Freeze, Wipe)

**RFI**: "Compliance controls: per-address denylist, freeze, or wipe (block from/to)"

**Current status: In development at OpenZeppelin.** This demo does not implement
compliance controls. This is stated explicitly because honest disclosure builds
trust.

**What is NOT enforced:**
- No on-chain denylist for accounts or addresses
- No ability to freeze individual account balances
- No compliance oracle integration
- No OFAC/SDN list screening at the protocol level
- No on-chain geographic restrictions

**How it will work (design):**

The compliance model uses **callbacks on the faucet account**. When a transaction
involves assets from a specific faucet (issuer), the transaction kernel calls the
faucet's compliance procedures:

- `create_note` and `consume_note` involving the faucet's asset will check
  the executing account against the faucet's denylist
- If the account is on the denylist, the transaction fails
- Freeze capabilities are implemented via pause/unpause on the faucet

**Design references:**
- [OZ Miden Confidential Contracts Discussion #39](https://github.com/OpenZeppelin/miden-confidential-contracts/discussions/39)
- [Protocol Issue #2432: Callbacks](https://github.com/0xMiden/protocol/issues/2432)

**Timeline**: Expected in Miden v0.14 (end of March 2025). Audit follows, estimated
2 additional weeks.

**Recommendation**: Implement OFAC screening at the application layer before
submitting transactions. This is independent of on-chain controls and can be done
today with existing infrastructure.

See [docs/COMPLIANCE_ROADMAP.md](COMPLIANCE_ROADMAP.md) for full details.

---

## 7. Testing

### 7.1 Running Tests

```bash
make test
# Runs all 16 tests — no network connection required
```

Tests use `MockChain` from `miden-testing`, a deterministic offline Miden execution
environment. It simulates the full transaction lifecycle (build, execute, prove,
apply delta) without connecting to any network.

### 7.2 How MockChain Works

```rust
// Build a test environment
let mut builder = MockChain::builder();
let faucet = builder.add_existing_basic_faucet(
    Auth::EcdsaK256KeccakAuth, "TST", 1_000_000, None,
)?;
let wallet = builder.add_existing_wallet(Auth::EcdsaK256KeccakAuth)?;
let note = builder.add_p2id_note(
    faucet.id(), wallet.id(), &[asset.into()], NoteType::Private,
)?;
let mock_chain = builder.build()?;

// Execute a transaction
let executed_tx = mock_chain
    .build_tx_context(wallet.id(), &[note.id()], &[])?
    .build()?
    .execute()
    .await?;

// Apply state changes and verify
wallet.apply_delta(executed_tx.account_delta())?;
assert_eq!(wallet.vault().get_balance(faucet.id())?, 100);
```

`MockChain` provides:
- Deterministic block production (no timing dependencies)
- Built-in ECDSA K256 authentication (via `Auth::EcdsaK256KeccakAuth`)
- P2ID note creation via `add_p2id_note()`
- Transaction context building with input notes

### 7.3 Test Inventory

#### Integration Tests (`tests/single_signer_test.rs`)

| Test | What It Verifies |
|------|-----------------|
| `create_ecdsa_wallet` | Account creation with ECDSA K256 auth succeeds |
| `deploy_issuer_account` | Faucet account created with correct type |
| `mint_and_consume` | Mint 100 tokens, consume at target, balance = 100 |
| `read_balance_after_mint` | Balance reads correctly via vault after consumption |
| `faucet_sysdata_slot_readable` | Total issuance slot accessible, starts at 0 |
| `faucet_is_configured_correctly` | Faucet structure and storage initialized properly |
| `consume_multiple_notes` | Two notes consumed in one tx, balance aggregated |

#### Unit Tests (`tests/operations_unit_test.rs`)

| Test | What It Verifies |
|------|-----------------|
| `config_custom_values` | Environment variable overrides work |
| `config_has_expected_field_names` | Config struct has all required fields |
| `ecdsa_key_generation` | ECDSA K256 keypair can be generated |
| `commitment_from_public_key_deterministic` | Same key produces same commitment |
| `mock_signer_sign_returns_65_bytes` | Signature is exactly 65 bytes (r+s+v) |
| `mock_signer_deterministic` | Same key + message produces same signature (RFC 6979) |
| `mock_signer_unknown_key_fails` | Signing with unregistered key returns error |
| `keccak256_hash_word_deterministic` | Same input produces same hash |
| `keccak256_hash_word_different_inputs` | Different inputs produce different hashes |

---

## 8. What's Next

### 8.1 Compliance Controls

The highest priority gap. OpenZeppelin is building denylist/freeze/wipe capabilities
as part of the Miden Confidential Contracts project:

| Control | Expected Timeline | Reference |
|---------|-------------------|-----------|
| Denylist / freeze | Miden v0.14 (end of March 2025) | [OZ Discussion #39](https://github.com/OpenZeppelin/miden-confidential-contracts/discussions/39) |
| Compliance callbacks | Miden v0.14 | [Protocol #2432](https://github.com/0xMiden/protocol/issues/2432) |
| Audit of compliance contracts | ~2 weeks after v0.14 | OpenZeppelin, Trail of Bits |

Until on-chain controls are available, OFAC screening should be implemented at the
application layer before submitting transactions.

### 8.2 Production Readiness

| Gap | Status | Workaround |
|-----|--------|------------|
| Fee estimation API | In development | Fees are effectively zero currently |
| Deposit webhooks | Not yet available | Poll via `sync_state()` every ~3 seconds |
| Transaction status polling | Limited | Use expiration block + timeout |
| "Pay in any token" fees | Planned | Not needed while fees are zero |
| Mainnet | July 2025 | Testnet and devnet available now |

### 8.3 Frontend Demo

A web-based coordinator UI (forked from the
[0xMiden/MultiSig coordinator frontend](https://github.com/0xMiden/MultiSig)) is
planned. It will demonstrate:
- Issuer operations (deploy, mint, burn, supply dashboard)
- Multisig proposal and approval workflows
- WASM Miden SDK running in the browser
- PSM integration for multi-party signing

---

## Appendix A: Configuration Reference

All settings via environment variables or `.env` file:

| Variable | Default | Description |
|----------|---------|-------------|
| `MIDEN_RPC_ENDPOINT` | `https://rpc.testnet.miden.io` | Miden node gRPC endpoint |
| `PSM_ENDPOINT` | `http://localhost:50051` | PSM server (for multisig) |
| `SQLITE_STORE_PATH` | `./store.sqlite3` | Client state database |
| `KEYSTORE_PATH` | `./keystore` | Filesystem key storage |
| `ISSUER_ACCOUNT_ID` | — | Faucet ID (set after deploy) |
| `LOG_LEVEL` | `info` | Tracing log level |

## Appendix B: Binary Reference

| Binary | Command | Purpose |
|--------|---------|---------|
| `create_account` | `make create-account` | Create ECDSA K256 wallet |
| `deploy_issuer` | `make deploy-issuer` | Deploy faucet with ECDSA auth |
| `mint` | `make mint` | Mint tokens to target account |
| `burn` | `make burn` | Burn tokens (reduce supply) |
| `transfer` | `make transfer` | P2ID transfer between accounts |
| `read_balance` | `make read-balance` | Read account balance |
| `read_supply` | `make read-supply` | Read total supply |
| `full_demo` | `make full-demo` | Full E2E lifecycle demo |
| `multisig_demo` | `make multisig-demo` | 2-of-3 multisig demo (requires PSM) |

## Appendix C: Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `miden-client` | 0.13 | Miden SDK (accounts, transactions, RPC) |
| `miden-protocol` | 0.13 | Protocol types (AccountId, Note, etc.) |
| `miden-standards` | 0.13 | Standard components (BasicWallet, BasicFungibleFaucet) |
| `miden-testing` | 0.13 | MockChain for deterministic testing |
| `miden-tx` | 0.13 | Transaction execution and authentication |
| `miden-multisig-client` | 0.13.0 (git) | PSM multisig orchestration |
| `tokio` | 1.40 | Async runtime |
| `sha3` | 0.10 | Keccak256 hashing |
| Rust toolchain | `nightly-2025-12-10` | Required by Miden SDK |
