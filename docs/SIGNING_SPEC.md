# Signing Specification — ECDSA K256 Keccak for Miden

This document specifies the byte-level signing protocol for integrating external
signing backends (MPC, HSM, custodian) with Miden transactions via ECDSA K256 Keccak.

## Overview

Miden transactions are authorized by signing a **transaction commitment** — a
Poseidon2 hash of the transaction's account delta, input notes, output notes, and
salt. The commitment is a `Word` (4 field elements). External signers receive a
Keccak256 digest of this commitment and return a 65-byte ECDSA signature.

## Signing Chain

```
SigningInputs::to_commitment()
    → Word (4 Felts, Poseidon2 hash)
    → [u8; 32] (Word byte representation)
    → Keccak256(bytes) → [u8; 32] digest
    → ExternalSigner::sign_prehash(commitment, digest)
    → [u8; 65] (r[32] || s[32] || v[1])
    → Signature::EcdsaK256Keccak
    → encode_signature() → 26 packed-u32 felts → advice stack
```

## Step 1: Transaction Commitment

The transaction executor calls `SigningInputs::to_commitment()` which produces a
`Word` — four `Felt` values representing a Poseidon2 hash of:

- Account delta (4 Felts)
- Input notes commitment (4 Felts)
- Output notes commitment (4 Felts)
- Salt (4 Felts)

## Step 2: Word to Bytes

Each `Felt` is converted to its canonical u64 representation, then to 8 bytes
(little-endian). Four Felts produce 32 bytes total:

```
word_bytes: [u8; 32] = Word::into()
```

## Step 3: Keccak256 Digest

The 32-byte word representation is hashed with Keccak256:

```
digest: [u8; 32] = Keccak256(word_bytes)
```

This matches the internal `hash_message` function in `miden_crypto::dsa::ecdsa_k256_keccak`.

## Step 4: ECDSA Signing

The external signer receives:
- `pub_key_commitment: PublicKeyCommitment` — identifies which key to use
- `digest: [u8; 32]` — the Keccak256 hash to sign

It returns 65 bytes in the following format:

| Offset | Length | Field | Encoding |
|--------|--------|-------|----------|
| 0      | 32     | `r`   | Big-endian unsigned integer |
| 32     | 32     | `s`   | Big-endian unsigned integer |
| 64     | 1      | `v`   | Recovery ID (0 or 1) |

**Important**: The signature is NOT DER-encoded. It uses raw `r || s || v` format.

## Step 5: Signature Reconstruction

The 65 bytes are parsed into `ecdsa_k256_keccak::Signature`:

```rust
let sig = Signature::from_sec1_bytes_and_recovery_id(
    sec1_bytes,  // [u8; 64] = r || s
    v,           // u8 = recovery ID
)?;
```

## Step 6: VM Encoding

The Miden VM encodes the signature as 26 packed-u32 felts for the advice stack:

```
[public_key(9 felts) || signature(17 felts)]
```

This encoding uses `bytes_to_packed_u32_elements()` from `miden-vm`.

## ExternalSigner Trait

```rust
#[async_trait]
pub trait ExternalSigner: Send + Sync {
    async fn sign_prehash(
        &self,
        pub_key_commitment: PublicKeyCommitment,
        digest: [u8; 32],
    ) -> Result<[u8; 65], ExternalSignerError>;
}
```

## BraleKeystore Flow

```
TransactionAuthenticator::get_signature(commitment, signing_inputs)
  1. commitment = signing_inputs.to_commitment()  // Word
  2. digest = keccak256_hash_word(commitment)      // [u8; 32]
  3. sig_bytes = signer.sign_prehash(commitment, digest)  // [u8; 65]
  4. signature = parse_ecdsa_signature(&sig_bytes)  // ecdsa_k256_keccak::Signature
  5. return Signature::EcdsaK256Keccak(signature)
```

## Key Derivation

```
SecretKey::new()                         // random secp256k1 secret key
  → SecretKey::public_key()              // compressed public key
  → PublicKey::to_commitment()           // Poseidon2 hash of packed-u32 public key bytes
  → PublicKeyCommitment::from(commitment) // commitment wrapper
```

## Security Notes

- The Keccak256 step prevents the signer from needing to understand Miden's field
  element encoding — it receives a standard 32-byte hash.
- Recovery ID `v` is required for the Miden VM's signature verification.
- The secp256k1 curve is used (same as Ethereum), making this compatible with
  existing MPC infrastructure (Blockdaemon BV, Fireblocks, etc.).
- Signing is deterministic per RFC 6979 — same key + message always produces the
  same signature.
