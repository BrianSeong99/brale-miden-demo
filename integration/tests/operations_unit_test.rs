//! Unit tests for config, key generation, mock signer, and keystore helpers.

use miden_client::auth::PublicKeyCommitment;
use miden_client::{Deserializable, Serializable, SliceReader};
use miden_protocol::crypto::dsa::ecdsa_k256_keccak::SecretKey;

use integration::config::Config;
use integration::keystore::{ExternalSigner, keccak256_hash_word};
use integration::mock_signer::SimulatedMpcSigner;

// ---------------------------------------------------------------------------
// Config tests
// ---------------------------------------------------------------------------

#[test]
fn config_custom_values() {
    // Set env vars, read config, then clean up — all in one test to avoid
    // env var pollution across parallel tests.
    std::env::set_var("MIDEN_RPC_ENDPOINT", "https://custom.rpc:443");
    std::env::set_var("ISSUER_ACCOUNT_ID", "0xabcdef1234567890");
    std::env::set_var("LOG_LEVEL", "debug");

    let config = Config::from_env();
    assert_eq!(config.miden_rpc_endpoint, "https://custom.rpc:443");
    assert_eq!(config.issuer_account_id.as_deref(), Some("0xabcdef1234567890"));
    assert_eq!(config.log_level, "debug");

    // Clean up
    std::env::remove_var("MIDEN_RPC_ENDPOINT");
    std::env::remove_var("ISSUER_ACCOUNT_ID");
    std::env::remove_var("LOG_LEVEL");
}

#[test]
fn config_has_expected_field_names() {
    // Verifies Config struct has all the expected fields (compile-time check)
    let config = Config {
        miden_rpc_endpoint: "rpc".into(),
        psm_endpoint: "psm".into(),
        sqlite_store_path: "db".into(),
        keystore_path: "ks".into(),
        issuer_account_id: None,
        log_level: "info".into(),
    };
    assert_eq!(config.miden_rpc_endpoint, "rpc");
    assert!(config.issuer_account_id.is_none());
}

// ---------------------------------------------------------------------------
// Key generation tests
// ---------------------------------------------------------------------------

#[test]
fn ecdsa_key_generation() {
    let sk = SecretKey::new();
    let pk = sk.public_key();
    // Public key should be constructible and yield a commitment
    let _commitment = PublicKeyCommitment::from(pk.to_commitment());
}

#[test]
fn commitment_from_public_key_deterministic() {
    let sk = SecretKey::new();
    let pk = sk.public_key();

    let c1 = PublicKeyCommitment::from(pk.to_commitment());
    let c2 = PublicKeyCommitment::from(pk.to_commitment());
    assert_eq!(c1, c2, "commitment derivation should be deterministic");
}

// ---------------------------------------------------------------------------
// Mock signer tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn mock_signer_sign_returns_65_bytes() {
    let signer = SimulatedMpcSigner::new();
    let sk = SecretKey::new();
    let commitment = signer.register_key(sk);

    let digest = [0u8; 32];
    let sig = signer
        .sign_prehash(commitment, digest)
        .await
        .expect("signing should succeed");

    assert_eq!(sig.len(), 65, "ECDSA signature should be 65 bytes (r||s||v)");
}

#[tokio::test]
async fn mock_signer_deterministic() {
    // ECDSA K256 with RFC 6979 is deterministic for the same key + message
    let signer = SimulatedMpcSigner::new();
    let sk = SecretKey::new();
    let commitment = signer.register_key(sk);

    let digest = [42u8; 32];
    let sig1 = signer.sign_prehash(commitment, digest).await.unwrap();
    let sig2 = signer.sign_prehash(commitment, digest).await.unwrap();
    assert_eq!(sig1, sig2, "same key + message should produce same signature");
}

#[tokio::test]
async fn mock_signer_unknown_key_fails() {
    let signer = SimulatedMpcSigner::new();
    // Don't register any key — use a random commitment
    let sk = SecretKey::new();
    let pk = sk.public_key();
    let unknown_commitment = PublicKeyCommitment::from(pk.to_commitment());

    let result = signer.sign_prehash(unknown_commitment, [0u8; 32]).await;
    assert!(result.is_err(), "signing with unknown key should fail");
}

// ---------------------------------------------------------------------------
// Keccak256 helper tests
// ---------------------------------------------------------------------------

#[test]
fn keccak256_hash_word_deterministic() {
    use miden_client::Word;

    let word = Word::default();
    let h1 = keccak256_hash_word(word);
    let h2 = keccak256_hash_word(word);
    assert_eq!(h1, h2, "keccak256 hash should be deterministic");
}

#[test]
fn keccak256_hash_word_different_inputs() {
    use miden_client::Word;
    use miden_client::Felt;

    let word1 = Word::default();
    let word2: Word = [Felt::new(1), Felt::new(2), Felt::new(3), Felt::new(4)].into();
    let h1 = keccak256_hash_word(word1);
    let h2 = keccak256_hash_word(word2);
    assert_ne!(h1, h2, "different inputs should produce different hashes");
}

// ---------------------------------------------------------------------------
// Signing test vector — deterministic end-to-end signing chain
// ---------------------------------------------------------------------------

/// Full signing chain test vector with known key, commitment, and signature.
///
/// This test produces deterministic hex values at each step of the signing pipeline:
///   1. Secret key (32 bytes) → Public key (33 bytes compressed)
///   2. Transaction commitment Word (4 field elements) → commitment bytes (32 bytes LE)
///   3. Keccak256(commitment_bytes) → digest (32 bytes)
///   4. ECDSA K256 sign_prehash(digest) → signature r (32) || s (32) || v (1) = 65 bytes
///   5. Public key verifies signature against the original commitment
///
/// Brale's MPC team (Blockdaemon BV) can use these values to validate their
/// signing integration independently.
#[tokio::test]
async fn signing_test_vector() {
    use miden_client::{Felt, Word};

    // -----------------------------------------------------------------------
    // Step 0: Deterministic secret key from known bytes
    // -----------------------------------------------------------------------
    // This is a TEST-ONLY key. Never use in production.
    let sk_bytes: [u8; 32] = [
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
        0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10,
        0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18,
        0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f, 0x20,
    ];

    let sk = SecretKey::read_from(&mut SliceReader::new(&sk_bytes))
        .expect("valid secret key bytes");
    let pk = sk.public_key();

    // Serialize public key for hex output
    let mut pk_bytes = Vec::new();
    pk.write_into(&mut pk_bytes);

    // Public key commitment (Poseidon2 hash)
    let pk_commitment_word = pk.to_commitment();
    let pk_commitment = PublicKeyCommitment::from(pk_commitment_word);

    // -----------------------------------------------------------------------
    // Step 1: Simulated transaction commitment (Word = 4 field elements)
    // -----------------------------------------------------------------------
    // In production this comes from signing_inputs.to_commitment() — a Poseidon2
    // hash of the transaction summary. For the test vector we use a known value.
    let commitment: Word = [
        Felt::new(0xdead_beef_0000_0001),
        Felt::new(0xdead_beef_0000_0002),
        Felt::new(0xdead_beef_0000_0003),
        Felt::new(0xdead_beef_0000_0004),
    ].into();

    // -----------------------------------------------------------------------
    // Step 2: Word → bytes (32 bytes, little-endian per element)
    // -----------------------------------------------------------------------
    let commitment_bytes: [u8; 32] = commitment.into();

    // -----------------------------------------------------------------------
    // Step 3: Keccak256(commitment_bytes) → 32-byte digest
    // -----------------------------------------------------------------------
    let digest = keccak256_hash_word(commitment);

    // -----------------------------------------------------------------------
    // Step 4: ECDSA K256 sign_prehash → 65-byte signature
    // -----------------------------------------------------------------------
    let signer = SimulatedMpcSigner::new();
    signer.register_key(sk);

    let sig_bytes = signer
        .sign_prehash(pk_commitment, digest)
        .await
        .expect("signing should succeed");

    assert_eq!(sig_bytes.len(), 65, "signature must be 65 bytes (r||s||v)");

    let r = &sig_bytes[0..32];
    let s = &sig_bytes[32..64];
    let v = sig_bytes[64];

    // -----------------------------------------------------------------------
    // Step 5: Verify signature
    // -----------------------------------------------------------------------
    let pk_verify = sk_to_pk_from_bytes(&sk_bytes);
    assert!(
        pk_verify.verify(commitment, &sig_to_miden(r, s, v)),
        "signature must verify against original commitment"
    );

    // -----------------------------------------------------------------------
    // Print all hex values for docs/SIGNING_TEST_VECTORS.md
    // -----------------------------------------------------------------------
    println!("\n=== SIGNING TEST VECTOR ===");
    println!("secret_key_hex:       {}", hex::encode(sk_bytes));
    println!("public_key_hex:       {}", hex::encode(&pk_bytes));
    println!("pk_commitment_word:   {:?}", pk_commitment_word);
    println!("commitment_felts:     [0x{:016x}, 0x{:016x}, 0x{:016x}, 0x{:016x}]",
        commitment[0].as_int(), commitment[1].as_int(),
        commitment[2].as_int(), commitment[3].as_int());
    println!("commitment_bytes_hex: {}", hex::encode(commitment_bytes));
    println!("digest_hex:           {}", hex::encode(digest));
    println!("signature_r_hex:      {}", hex::encode(r));
    println!("signature_s_hex:      {}", hex::encode(s));
    println!("signature_v:          {}", v);
    println!("signature_full_hex:   {}", hex::encode(&sig_bytes));
    println!("=== END TEST VECTOR ===\n");
}

// Helpers for the signing test vector (avoid exposing internals)
fn sk_to_pk_from_bytes(sk_bytes: &[u8; 32]) -> miden_protocol::crypto::dsa::ecdsa_k256_keccak::PublicKey {
    let sk = SecretKey::read_from(&mut SliceReader::new(sk_bytes))
        .expect("valid secret key bytes");
    sk.public_key()
}

fn sig_to_miden(r: &[u8], s: &[u8], v: u8) -> miden_protocol::crypto::dsa::ecdsa_k256_keccak::Signature {
    let mut sec1 = [0u8; 64];
    sec1[0..32].copy_from_slice(r);
    sec1[32..64].copy_from_slice(s);
    miden_protocol::crypto::dsa::ecdsa_k256_keccak::Signature::from_sec1_bytes_and_recovery_id(sec1, v)
        .expect("valid signature")
}
