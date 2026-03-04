# Signer Injection: MPC/HSM Bridge

[Back to Index](../WALKTHROUGH.md) | [Previous: Architecture](02-architecture.md)

---

This is the core integration point for Brale's existing custody infrastructure.
The design goal: **Blockdaemon BV should work without understanding Miden internals.**

## The ExternalSigner Trait

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

## BraleKeystore: How It Plugs Into the Miden SDK

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

## The Signing Chain (Byte-Level)

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

## SimulatedMpcSigner (Test Double)

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

## Supported Authentication Schemes

Miden supports two signature schemes for account authentication. **Ed25519 is not
supported** — the RFI response was incorrect on this point.

### ECDSA K256 (secp256k1) — Recommended for Brale

- **Same curve as Ethereum**. Key material, signing, and verification are identical.
- **MPC-compatible**. Blockdaemon BV, Fireblocks, and other MPC providers already
  support secp256k1 threshold signing. No new cryptographic integration needed.
- **This demo uses this scheme** via `AuthEcdsaK256Keccak`.
- The digest format is Keccak256 (same as `eth_sign`). MPC providers receive a
  standard 32-byte hash and return a standard 65-byte ECDSA signature.
- Account component: `AuthEcdsaK256Keccak`

### RPO-Falcon512 (Post-Quantum)

- **Lattice-based signature scheme** (NIST PQC finalist family).
- Provides post-quantum security, but **not recommended for Brale's use case**:
  - Key sharding for MPC is complex for lattice schemes — no standard MPC protocol
    exists for Falcon.
  - Larger signatures (~700 bytes vs 65 bytes for ECDSA).
  - No existing MPC provider support.
- Suitable for single-signer scenarios where post-quantum resistance is required.
- Account component: `AuthRpoFalcon512`

### Why ECDSA K256 is the Right Choice

For a custody provider like Brale, the signing infrastructure is the critical path.
ECDSA K256 means:

1. **Zero MPC changes** — Blockdaemon BV already supports secp256k1
2. **Same key material** — existing Ethereum keys work on Miden
3. **Same signing flow** — receive Keccak256 digest, return ECDSA signature
4. **Battle-tested** — years of production use across Ethereum ecosystem

---

## Production Integration: What Changes

| Component | This Demo | Production |
|-----------|-----------|------------|
| `ExternalSigner` impl | `SimulatedMpcSigner` (local keys) | HTTP client to Blockdaemon BV |
| Key storage | `FilesystemKeyStore` (local disk) | HSM-backed key storage |
| Key generation | `SecretKey::new()` (random) | MPC key generation ceremony |
| Client state | SQLite file | PostgreSQL / managed database |
| RPC endpoint | Testnet (`rpc.testnet.miden.io`) | Production node |

The `ExternalSigner` trait, `BraleKeystore`, and all operations code remain
unchanged. Only the signer implementation and infrastructure config change.

See also: [SIGNING_SPEC.md](../SIGNING_SPEC.md) for the full byte-level protocol
specification.

---

[Next: PSM Multisig](04-psm-multisig.md) | [Back to Index](../WALKTHROUGH.md)
