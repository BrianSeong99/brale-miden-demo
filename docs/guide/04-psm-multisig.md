# PSM: Private Multisig Orchestration

[Back to Guide Index](../../README.md#guide) | [Previous: Signer Injection](03-signer-injection.md)

---

## What PSM Solves

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

## KeyManager Bridge

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

## 2-of-3 ECDSA Multisig Flow

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

## Running the Multisig Demo

See [Running the Demos § PSM Server Setup](09-running-demos.md#psm-server-setup) for
PSM server instructions, then run:

```bash
make multisig-demo
```

The demo (`integration/src/bin/multisig_demo.rs`):
1. Generates 3 ECDSA K256 keypairs
2. Builds a `MultisigClient` connected to both Miden RPC and PSM
3. Creates a 2-of-3 multisig account
4. Demonstrates that 2 signers can produce valid signatures

---

[Next: RFI Requirements](05-rfi-requirements.md) | [Back to Guide Index](../../README.md#guide)
