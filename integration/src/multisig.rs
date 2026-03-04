//! Multisig wrapper — bridges `ExternalSigner` to PSM's `KeyManager` trait.
//!
//! `BraleMpcKeyManager` implements PSM's `KeyManager` so that multisig signing
//! requests are routed through our `ExternalSigner` (MPC, HSM, custodian).
//! For testing, `SimulatedMpcKeyManager` wraps `EcdsaPsmKeyStore` directly.

use std::sync::Arc;

use miden_client::{Serializable, auth::PublicKeyCommitment, Word};
use miden_multisig_client::{
    EcdsaPsmKeyStore, KeyManager, SchemeSecretKey, SignatureScheme,
};
use miden_protocol::crypto::dsa::ecdsa_k256_keccak;

use crate::keystore::{keccak256_hash_word, ExternalSigner};

// ---------------------------------------------------------------------------
// BraleMpcKeyManager — routes PSM signing through ExternalSigner
// ---------------------------------------------------------------------------

/// PSM `KeyManager` backed by an `ExternalSigner` (MPC, HSM, custodian).
///
/// Holds an ECDSA K256 secret key for public key / commitment derivation,
/// but delegates actual signing to the external backend.
pub struct BraleMpcKeyManager {
    /// The ECDSA secret key (needed for commitment derivation and key export).
    secret_key: ecdsa_k256_keccak::SecretKey,
    /// Public key derived from the secret key.
    public_key: ecdsa_k256_keccak::PublicKey,
    /// Commitment derived from public key.
    commitment: Word,
    /// Hex-encoded commitment.
    commitment_hex: String,
    /// External signing backend.
    signer: Arc<dyn ExternalSigner>,
}

impl BraleMpcKeyManager {
    /// Create a new key manager that delegates signing to the given `ExternalSigner`.
    pub fn new(
        secret_key: ecdsa_k256_keccak::SecretKey,
        signer: Arc<dyn ExternalSigner>,
    ) -> Self {
        let public_key = secret_key.public_key();
        let commitment_word: Word = public_key.to_commitment().into();
        let commitment_bytes: [u8; 32] = commitment_word.into();
        let commitment_hex = format!("0x{}", hex::encode(commitment_bytes));

        Self {
            secret_key,
            public_key,
            commitment: commitment_word,
            commitment_hex,
            signer,
        }
    }
}

impl KeyManager for BraleMpcKeyManager {
    fn scheme(&self) -> SignatureScheme {
        SignatureScheme::Ecdsa
    }

    fn commitment(&self) -> Word {
        self.commitment
    }

    fn commitment_hex(&self) -> String {
        self.commitment_hex.clone()
    }

    fn sign_hex(&self, message: Word) -> String {
        // PSM's KeyManager::sign_hex is synchronous, but our ExternalSigner is async.
        // Bridge using tokio::runtime::Handle for the blocking context.
        let pub_key_commitment = PublicKeyCommitment::from(self.public_key.to_commitment());
        let digest = keccak256_hash_word(message);

        let signer = self.signer.clone();
        let sig_bytes = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                signer.sign_prehash(pub_key_commitment, digest).await
            })
        });

        match sig_bytes {
            Ok(bytes) => format!("0x{}", hex::encode(bytes)),
            Err(e) => panic!("BraleMpcKeyManager: signing failed: {e}"),
        }
    }

    fn secret_key(&self) -> SchemeSecretKey {
        SchemeSecretKey::Ecdsa(self.secret_key.clone())
    }

    fn public_key_hex(&self) -> Option<String> {
        Some(format!("0x{}", hex::encode(self.public_key.to_bytes())))
    }
}

// ---------------------------------------------------------------------------
// SimulatedMpcKeyManager — wraps EcdsaPsmKeyStore for testing
// ---------------------------------------------------------------------------

/// Test-only key manager that uses PSM's `EcdsaPsmKeyStore` directly.
pub struct SimulatedMpcKeyManager {
    inner: EcdsaPsmKeyStore,
}

impl SimulatedMpcKeyManager {
    /// Generate a new random ECDSA key manager for testing.
    pub fn generate() -> Self {
        Self {
            inner: EcdsaPsmKeyStore::generate(),
        }
    }

    /// Create from an existing secret key.
    pub fn new(secret_key: ecdsa_k256_keccak::SecretKey) -> Self {
        Self {
            inner: EcdsaPsmKeyStore::new(secret_key),
        }
    }

    /// Access the inner PSM key store.
    pub fn inner(&self) -> &EcdsaPsmKeyStore {
        &self.inner
    }

    /// Get the ECDSA public key.
    pub fn public_key(&self) -> ecdsa_k256_keccak::PublicKey {
        self.inner.public_key()
    }

    /// Clone the secret key.
    pub fn clone_secret_key(&self) -> ecdsa_k256_keccak::SecretKey {
        self.inner.clone_ecdsa_secret_key()
    }
}

impl KeyManager for SimulatedMpcKeyManager {
    fn scheme(&self) -> SignatureScheme {
        self.inner.scheme()
    }

    fn commitment(&self) -> Word {
        self.inner.commitment()
    }

    fn commitment_hex(&self) -> String {
        self.inner.commitment_hex()
    }

    fn sign_hex(&self, message: Word) -> String {
        self.inner.sign_hex(message)
    }

    fn secret_key(&self) -> SchemeSecretKey {
        self.inner.secret_key()
    }

    fn public_key_hex(&self) -> Option<String> {
        self.inner.public_key_hex()
    }
}
