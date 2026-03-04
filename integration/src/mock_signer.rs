//! SimulatedMpcSigner — local ECDSA K256 signer for testing.
//!
//! Holds a mapping of `PublicKeyCommitment → SecretKey` for deterministic test signing.
//! In production, this would be replaced by an HTTP client to Blockdaemon BV or similar.

use std::collections::BTreeMap;
use std::sync::Mutex;

use miden_client::auth::PublicKeyCommitment;
use miden_protocol::crypto::dsa::ecdsa_k256_keccak::SecretKey;

use crate::keystore::{ExternalSigner, ExternalSignerError};

/// A local ECDSA K256 signer that simulates an MPC backend.
///
/// Thread-safe via `Mutex` on the key map. Keys can be registered after construction.
pub struct SimulatedMpcSigner {
    keys: Mutex<BTreeMap<PublicKeyCommitment, SecretKey>>,
}

impl SimulatedMpcSigner {
    /// Create an empty signer. Register keys via [`register_key`].
    pub fn new() -> Self {
        Self {
            keys: Mutex::new(BTreeMap::new()),
        }
    }

    /// Register a secret key. The public key commitment is derived automatically.
    pub fn register_key(&self, secret_key: SecretKey) -> PublicKeyCommitment {
        let pub_key = secret_key.public_key();
        let commitment = PublicKeyCommitment::from(pub_key.to_commitment());
        self.keys
            .lock()
            .expect("lock poisoned")
            .insert(commitment, secret_key);
        commitment
    }
}

impl Default for SimulatedMpcSigner {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ExternalSigner for SimulatedMpcSigner {
    async fn sign_prehash(
        &self,
        pub_key_commitment: PublicKeyCommitment,
        digest: [u8; 32],
    ) -> Result<[u8; 65], ExternalSignerError> {
        let keys = self.keys.lock().expect("lock poisoned");
        let secret_key = keys
            .get(&pub_key_commitment)
            .ok_or_else(|| {
                ExternalSignerError::SigningFailed(format!(
                    "unknown key commitment: {pub_key_commitment:?}"
                ))
            })?;

        // sign_prehash takes the already-hashed digest (Keccak256 of commitment bytes)
        let signature = secret_key.sign_prehash(digest);

        // Encode as r[32] || s[32] || v[1]
        let mut out = [0u8; 65];
        out[0..32].copy_from_slice(signature.r());
        out[32..64].copy_from_slice(signature.s());
        out[64] = signature.v();

        Ok(out)
    }
}
