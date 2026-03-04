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
