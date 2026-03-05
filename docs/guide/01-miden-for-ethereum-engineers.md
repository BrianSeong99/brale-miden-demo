# Miden for Ethereum Engineers

[Back to Guide Index](../../README.md#guide)

---

## The 60-Second Mental Model

On Ethereum, every transaction re-executes on every validator. On Miden, the user
executes the transaction locally on their own machine, generates a zero-knowledge proof
(STARK) that the execution was correct, and submits the proof to the network. The
network verifies the proof without re-executing anything. This is what makes privacy
and parallel execution possible.

Think of it this way: on Ethereum, the network runs your code. On Miden, you run your
code and prove you did it correctly.

## Accounts

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

## Notes (Miden's UTXO Model)

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

## Client-Side Execution and STARK Proofs

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

## Faucets: Native Token Issuance

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

## Privacy Model

| Component | Public Mode | Private Mode |
|-----------|-------------|--------------|
| Account state | Full state on-chain | Only commitment hash on-chain (40 bytes) |
| Notes | Full note data on-chain | Only commitment on-chain |
| Transaction | Proof + state delta visible | Proof + state delta visible |

Faucets (issuers) should be **public** — total supply must be auditable.
User wallets can be **private** — balances are not visible to the network.

## Concept Mapping: Ethereum to Miden

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

[Next: Architecture](02-architecture.md) | [Back to Guide Index](../../README.md#guide)
