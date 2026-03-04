# Signing Test Vectors

Test vectors for validating ECDSA K256 signing integration with Miden.

These values are produced by the `signing_test_vector` test in
`integration/tests/operations_unit_test.rs`. Run it with:

```bash
cargo test -p integration signing_test_vector -- --nocapture
```

---

## Key Material

**Secret key** (32 bytes, TEST ONLY — never use in production):

```
0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20
```

**Public key** (33 bytes, SEC1 compressed):

```
0284bf7562262bbd6940085748f3be6afa52ae317155181ece31b66351ccffa4b0
```

**Public key commitment** (Poseidon2 hash of public key, used as key identifier):

```
Word([4643914429727197305, 17938415812757176126, 1005359790276176491, 2558399003173972745])
```

---

## Signing Chain

The MPC provider receives a 32-byte Keccak256 digest and returns a 65-byte ECDSA
signature. Here is the full chain from transaction commitment to verified signature.

### Step 1 — Transaction Commitment (Word)

In production, this comes from `signing_inputs.to_commitment()` — a Poseidon2 hash of
the transaction summary. For the test vector we use a known value.

**Commitment** (4 field elements):

```
[0xdeadbeef00000001, 0xdeadbeef00000002, 0xdeadbeef00000003, 0xdeadbeef00000004]
```

### Step 2 — Commitment → Bytes

Each field element is converted to 8 bytes (little-endian u64), concatenated to 32 bytes:

```
01000000efbeadde02000000efbeadde03000000efbeadde04000000efbeadde
```

### Step 3 — Keccak256 Digest

The 32-byte commitment is hashed with Keccak256. **This is what the MPC provider signs.**

```
3a74bb6f13f369539a15f33be7c1f81ad34aa82f3b6d7f54bd2d8afe0aacc90a
```

This is identical to Ethereum's `keccak256()` — standard libraries (ethers.js, OpenSSL,
Blockdaemon BV SDK) will produce this same output for the same input.

### Step 4 — ECDSA K256 Signature

The MPC provider signs the 32-byte digest using secp256k1 (same curve as Ethereum).
RFC 6979 deterministic nonce generation is expected.

**Signature r** (32 bytes, big-endian):

```
87c2c584c773357d5d4d498809a0af5ca6babc7e0b8c8b3e4bb70866abc1e3a9
```

**Signature s** (32 bytes, big-endian):

```
34bd1df7715a93ff25a45225e6a79d10a6c76a234b9c5b7d4640afe12d825ecf
```

**Recovery ID (v)**: `1`

**Full signature** (65 bytes: `r[32] || s[32] || v[1]`):

```
87c2c584c773357d5d4d498809a0af5ca6babc7e0b8c8b3e4bb70866abc1e3a934bd1df7715a93ff25a45225e6a79d10a6c76a234b9c5b7d4640afe12d825ecf01
```

### Step 5 — Verification

The Miden VM verifies the signature against the account's stored public key commitment.
The test confirms `PublicKey::verify(commitment, &signature)` returns `true`.

---

## Integration Checklist for MPC Providers

To validate your signing integration:

1. Load the test secret key (or derive the same public key via your key generation)
2. Receive the 32-byte digest: `3a74bb6f13f369539a15f33be7c1f81ad34aa82f3b6d7f54bd2d8afe0aacc90a`
3. Sign with ECDSA secp256k1 using RFC 6979 deterministic nonce
4. Verify your output matches the signature above
5. Return the 65-byte result as `r[32] || s[32] || v[1]`

**What your MPC provider does NOT need to handle:**
- Field element encoding (done by `BraleKeystore` before calling `sign_prehash`)
- Poseidon2 hashing (internal to Miden SDK)
- VM advice stack encoding (handled by SDK after signature is returned)

The MPC provider's interface is identical to Ethereum signing: receive a 32-byte
Keccak256 digest, return a 65-byte ECDSA signature.

---

See also: [SIGNING_SPEC.md](SIGNING_SPEC.md) for the full byte-level protocol specification.
