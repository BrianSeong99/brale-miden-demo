# Demo Walkthrough

Step-by-step breakdown of what each demo does, what's happening on-chain, and what to expect.

## Full Demo (`make full-demo`)

**Requirements:** Miden testnet connectivity
**Runtime:** ~30–60 seconds (depends on testnet block times)
**State:** Uses a fresh temp directory per run — no setup needed, no stale state

The full demo exercises the complete stablecoin lifecycle through a single `BraleKeystore` client
backed by a `SimulatedMpcSigner`. All three accounts (issuer + two wallets) share one client
instance, simulating a custodian managing multiple accounts.

### Step 1: Deploy Issuer

Creates an ECDSA K256 faucet account on the Miden testnet.

- **Token:** USDB (Brale USD stablecoin)
- **Decimals:** 6
- **Max supply:** 1,000,000,000,000 (1 trillion base units)
- **Auth:** `AuthEcdsaK256Keccak` — the faucet is controlled by a single secp256k1 key

The issuer account acts as both the token definition and the minting authority. On Miden, faucets
are first-class account types that can mint and burn their associated token.

### Step 2: Create Wallets

Creates two regular wallet accounts (Wallet A and Wallet B), each with their own ECDSA K256 keypair.

After creation, the client calls `sync_state()` to register note tags for the new accounts. This
ensures the client will detect notes addressed to these wallets in subsequent syncs.

### Step 3: Mint Tokens

The issuer mints 1,000 tokens to Wallet A.

**What happens on-chain:**
1. Client builds a mint transaction against the issuer account
2. `BraleKeystore` signs via `SimulatedMpcSigner` (in production, this calls an MPC/HSM provider)
3. Transaction is proven locally (STARK proof, ~2s)
4. Proven transaction is submitted to the Miden node
5. Node includes it in a block → creates an output note addressed to Wallet A

**What happens locally:**
1. The client auto-detects the output note is relevant to Wallet A (tracked account)
2. Note enters `Expected` state in the local store
3. Client polls `sync_state()` until the note transitions to `Committed` (block inclusion)
4. Once committed, `consume_notes` builds a consume transaction for Wallet A
5. Wallet A's balance increases by 1,000

### Step 4: Transfer Tokens

Wallet A sends 300 tokens to Wallet B via a P2ID (Pay-to-ID) note.

**What happens on-chain:**
1. Client builds a send transaction from Wallet A
2. Transaction creates a P2ID note: "300 USDB tokens, claimable only by Wallet B"
3. Transaction is proven and submitted
4. Node includes it in a block

**What happens locally:**
1. Client detects the P2ID note is for Wallet B
2. Polls until note is committed
3. Wallet B consumes the note → balance increases by 300

**After transfer:** Wallet A has 700, Wallet B has 300.

### Step 5: Read Balances

Reads the fungible asset vault of each wallet account from the local store. No network call needed —
balances are tracked locally after each transaction.

### Step 6: Read Total Supply

Reads the issuer's on-chain supply counter. This is stored in the faucet account's storage slot and
reflects the net tokens in circulation (minted minus burned).

> **Note:** In the current Miden SDK, the local faucet account's storage may not reflect
> the on-chain state until a full resync. The demo reads from the local store.

### Step 7: Burn Tokens

Wallet B burns 100 tokens. Burning is a two-step process:

1. **Wallet B sends a burn note** — creates a special note addressed to the issuer containing
   100 tokens. This is a regular send transaction from Wallet B's perspective.
2. **Issuer consumes the burn note** — the issuer account consumes the note, which destroys
   the tokens and decrements the supply counter.

The client polls `sync_state()` between these steps, waiting for the burn note to be included
in a block before the issuer can consume it.

### Final State

```
Wallet A: 700   (minted 1000, sent 300)
Wallet B: 200   (received 300, burned 100)
```

---

## Multisig Demo (`make multisig-demo`)

**Requirements:** Miden testnet connectivity + PSM server on `localhost:50051`
**Runtime:** ~5 seconds
**State:** Creates `./multisig-accounts/` directory for account data

The multisig demo shows institutional custody using the PSM (Private State Manager) for 2-of-3
threshold ECDSA K256 signing. This is the pattern for treasury management where multiple parties
must authorize transactions.

### Step 1: Generate 3 ECDSA K256 Keypairs

Creates three independent `SimulatedMpcKeyManager` instances, each with its own secp256k1 keypair.
In production, each keypair lives in a different custodian's MPC/HSM infrastructure.

Each signer produces a **commitment** — a keccak256 hash of the public key encoded as a `Word`
(four Miden field elements). Commitments are used to register signers without revealing public keys.

### Step 2: Build MultisigClient

Connects to both the Miden testnet and the local PSM server:

- **Miden endpoint:** For on-chain account creation and transaction submission
- **PSM endpoint:** For multisig coordination (proposal, signature collection, execution)
- **Authentication:** Signer 1's secret key (the client operator)

The `MultisigClient` wraps both connections and manages the multisig lifecycle.

### Step 3: Create 2-of-3 Multisig Account

Registers all three signer commitments with PSM and creates a Miden account with threshold auth.

**What PSM does:**
1. Receives the 3 commitments and threshold (2)
2. Creates a Miden account with a `multisig_ecdsa` authentication component
3. The auth component is configured to require 2-of-3 valid ECDSA signatures
4. Returns the account ID

**Privacy property:** When a transaction is later executed, the network only sees a single
aggregated proof that the threshold was met. Individual signatures and which signers participated
are not revealed on-chain.

### Step 4: Demonstrate Signing

Two of the three signers produce signatures over a test message:

1. Signer 1 signs → signature collected
2. Signer 2 signs → signature collected
3. Threshold met (2 of 3)

In a full transaction flow (not shown in this demo), the signatures would be:
1. **Proposed** via `client.propose(...)` — creates a pending transaction
2. **Signed** by each signer via `client.sign(proposal_id)` — collects signatures at PSM
3. **Executed** via `client.execute(proposal_id)` — once threshold is met, PSM submits to Miden

### PSM Server Setup

The PSM server defaults to writing to `/var/psm/` which requires root. Use local paths:

```bash
cd /path/to/private-state-manager
PSM_STORAGE_PATH=./data/storage \
PSM_METADATA_PATH=./data/metadata \
PSM_KEYSTORE_PATH=./data/keystore \
cargo run -p private-state-manager-server
```

Listens on gRPC `localhost:50051` and HTTP `localhost:3000`.
