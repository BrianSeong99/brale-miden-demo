//! Unit tests for config, key generation, mock signer, and keystore helpers.

use miden_client::auth::PublicKeyCommitment;
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
