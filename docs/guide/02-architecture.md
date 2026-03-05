# Architecture

[Back to Index](../WALKTHROUGH.md) | [Previous: Miden for Ethereum Engineers](01-miden-for-ethereum-engineers.md)

---

## System Diagram

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

## How Signing Works: BraleKeystore + ExternalSigner

miden-client requires a `TransactionAuthenticator` to sign transactions. We provide
`BraleKeystore`, which wraps two things:

1. **`FilesystemKeyStore`** (from miden-client) — stores secret keys on disk. Used
   only for key management. Never signs when an external signer is configured.
2. **`ExternalSigner`** (our trait) — the actual signing interface. One async method:
   give it a Keccak256 digest, get back a 65-byte ECDSA signature.

`BraleKeystore` is the router that sits between miden-client and the signing backend:

```
miden-client needs a signature
  │
  ▼
BraleKeystore.get_signature()
  │
  ├─ signer is Some(...)? ──YES──▶ ExternalSigner.sign_prehash(digest)
  │                                  │
  │                                  ├─ SimulatedMpcSigner (demo: signs locally)
  │                                  ├─ BlockdaemonSigner  (production: MPC call)
  │                                  └─ FireblocksSigner   (production: HSM call)
  │
  └─ signer is None? ──────YES──▶ FilesystemKeyStore signs locally (fallback)
```

**In the demo**, `BraleKeystore` is always constructed with `Some(SimulatedMpcSigner)`,
so every transaction signature routes through the `ExternalSigner` trait. The
`FilesystemKeyStore` stores keys but never signs. In production, you swap
`SimulatedMpcSigner` for an HTTP client to your MPC/HSM provider — `BraleKeystore`
and all operations code remain unchanged.

### Demo vs Production: What's Real, What's Simulated

| Component | Demo | Production | Changes? |
|-----------|------|------------|----------|
| `BraleKeystore` (router) | Real | Real | No |
| `ExternalSigner` (trait) | Real | Real | No |
| Signer implementation | `SimulatedMpcSigner` (local) | HTTP client to MPC/HSM | **Swap only this** |
| `FilesystemKeyStore` | Stores keys, never signs | HSM-backed storage | Config change |
| `operations.rs` | Real | Real | No |

### Multisig Demo: Different Path

The multisig demo uses PSM's `EcdsaPsmKeyStore` directly via `SimulatedMpcKeyManager`.
It does **not** go through `ExternalSigner`. The production multisig equivalent is
`BraleMpcKeyManager`, which does route through `ExternalSigner` — same trait, same
MPC backend.

## Component Inventory

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

## Signing Patterns

| Pattern | Use Case | Auth Component | Client Type |
|---------|----------|----------------|-------------|
| **Single signer** | Brale-custodied wallet, single MPC key | `AuthEcdsaK256Keccak` | `Client<BraleKeystore>` |
| **PSM Multisig** | Treasury, governance, multi-party custody | PSM `multisig_ecdsa` | `MultisigClient` |

Both use ECDSA secp256k1 (K256). Both are compatible with Blockdaemon BV / Sepior.

---

[Next: Signer Injection](03-signer-injection.md) | [Back to Index](../WALKTHROUGH.md)
