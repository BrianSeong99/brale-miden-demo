//! BraleKeystore — wraps FilesystemKeyStore, overrides signing with ExternalSigner.
//!
//! Provides the `ExternalSigner` trait for pluggable signing backends (MPC, HSM, custodian)
//! and `BraleKeystore` which delegates key management to `FilesystemKeyStore` while routing
//! signing through the external backend when configured.
//!
//! Pattern: implements `TransactionAuthenticator + From<FilesystemKeyStore>` to satisfy
//! the `BuilderAuthenticator` bound required by `ClientBuilder`.

use std::sync::Arc;

use miden_client::{
    auth::{PublicKey, PublicKeyCommitment, Signature, SigningInputs, TransactionAuthenticator},
    keystore::FilesystemKeyStore,
    Word,
};
use miden_protocol::crypto::dsa::ecdsa_k256_keccak;
use miden_tx::AuthenticationError;
use sha3::{Digest, Keccak256};

// ---------------------------------------------------------------------------
// ExternalSigner trait
// ---------------------------------------------------------------------------

/// Error type for external signing operations.
#[derive(Debug, thiserror::Error)]
pub enum ExternalSignerError {
    #[error("signing failed: {0}")]
    SigningFailed(String),
    #[error("signer unavailable: {0}")]
    Unavailable(String),
    #[error("invalid response: {0}")]
    InvalidResponse(String),
}

/// Trait for pluggable external signing backends (MPC, HSM, custodian).
///
/// Implementors receive a 32-byte Keccak256 digest and return a 65-byte ECDSA signature.
/// See `docs/SIGNING_SPEC.md` for the byte-level protocol.
#[async_trait::async_trait]
pub trait ExternalSigner: Send + Sync {
    /// Sign a 32-byte Keccak256 digest. Returns `r[32] || s[32] || v[1]` = 65 bytes.
    async fn sign_prehash(
        &self,
        pub_key_commitment: PublicKeyCommitment,
        digest: [u8; 32],
    ) -> Result<[u8; 65], ExternalSignerError>;
}

// ---------------------------------------------------------------------------
// BraleKeystore
// ---------------------------------------------------------------------------

/// Keystore that wraps `FilesystemKeyStore` for key management and optionally
/// routes signing through an `ExternalSigner` (MPC, HSM, etc.).
///
/// When no external signer is configured, all operations (including signing)
/// delegate to the inner `FilesystemKeyStore`.
pub struct BraleKeystore {
    inner: FilesystemKeyStore,
    signer: Option<Arc<dyn ExternalSigner>>,
}

impl BraleKeystore {
    /// Create a new `BraleKeystore` with an external signing backend.
    pub fn new(inner: FilesystemKeyStore, signer: Arc<dyn ExternalSigner>) -> Self {
        Self {
            inner,
            signer: Some(signer),
        }
    }

    /// Create a `BraleKeystore` that delegates all operations to the inner keystore.
    pub fn passthrough(inner: FilesystemKeyStore) -> Self {
        Self {
            inner,
            signer: None,
        }
    }

    /// Access the inner `FilesystemKeyStore` (for key management operations).
    pub fn inner(&self) -> &FilesystemKeyStore {
        &self.inner
    }
}

/// Required by `BuilderAuthenticator = TransactionAuthenticator + From<FilesystemKeyStore>`.
/// When created via `From`, no external signer is configured — all operations
/// delegate to the inner `FilesystemKeyStore`.
impl From<FilesystemKeyStore> for BraleKeystore {
    fn from(inner: FilesystemKeyStore) -> Self {
        Self::passthrough(inner)
    }
}

// ---------------------------------------------------------------------------
// TransactionAuthenticator — override signing when external signer is present
// ---------------------------------------------------------------------------

impl TransactionAuthenticator for BraleKeystore {
    fn get_signature(
        &self,
        pub_key_commitment: PublicKeyCommitment,
        signing_inputs: &SigningInputs,
    ) -> impl std::future::Future<Output = Result<Signature, AuthenticationError>> + Send {
        let commitment: Word = signing_inputs.to_commitment();
        let signer = self.signer.clone();

        // Pre-compute fallback for the no-signer case (synchronous key read + sign).
        let fallback = if signer.is_none() {
            Some(
                self.inner
                    .get_key(pub_key_commitment)
                    .map_err(|e| {
                        AuthenticationError::other_with_source("failed to load secret key", e)
                    })
                    .and_then(|opt| {
                        opt.ok_or(AuthenticationError::UnknownPublicKey(pub_key_commitment))
                    })
                    .map(|key| key.sign(commitment)),
            )
        } else {
            None
        };

        async move {
            // Fast path: no external signer — use inner keystore.
            if let Some(result) = fallback {
                return result;
            }

            // External signer path:
            // 1. Keccak256(commitment_bytes) → 32-byte digest
            // 2. ExternalSigner::sign_prehash → 65-byte ECDSA signature
            // 3. Parse into Signature::EcdsaK256Keccak
            let ext_signer = signer.expect("signer presence checked above");
            let digest = keccak256_hash_word(commitment);

            let sig_bytes = ext_signer
                .sign_prehash(pub_key_commitment, digest)
                .await
                .map_err(|e| AuthenticationError::other(e.to_string()))?;

            let ecdsa_sig = parse_ecdsa_signature(&sig_bytes)
                .map_err(|e| AuthenticationError::other(format!("invalid ECDSA signature: {e}")))?;

            Ok(Signature::EcdsaK256Keccak(ecdsa_sig))
        }
    }

    fn get_public_key(
        &self,
        _pub_key_commitment: PublicKeyCommitment,
    ) -> impl std::future::Future<Output = Option<&PublicKey>> + Send {
        // FilesystemKeyStore returns None for this in v0.13.2.
        async { None }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Keccak256 hash of a Word's byte representation.
/// Matches the internal `hash_message` in `miden_crypto::dsa::ecdsa_k256_keccak`.
pub fn keccak256_hash_word(word: Word) -> [u8; 32] {
    let message_bytes: [u8; 32] = word.into();
    let mut hasher = Keccak256::new();
    hasher.update(message_bytes);
    hasher.finalize().into()
}

/// Parse 65-byte ECDSA signature (r[32] || s[32] || v[1]) into miden-crypto Signature.
fn parse_ecdsa_signature(bytes: &[u8; 65]) -> Result<ecdsa_k256_keccak::Signature, String> {
    let mut sec1_bytes = [0u8; 64];
    sec1_bytes.copy_from_slice(&bytes[0..64]);
    let v = bytes[64];
    ecdsa_k256_keccak::Signature::from_sec1_bytes_and_recovery_id(sec1_bytes, v)
        .map_err(|e| format!("{e}"))
}
