use crate::error::{AppError, Result};
use ed25519_dalek::{Signer, SigningKey, VerifyingKey, SECRET_KEY_LENGTH, SIGNATURE_LENGTH};
use rand::rngs::OsRng;

/// A locally-generated ed25519 keypair used as the Solana session wallet.
pub struct SessionKey {
    signing: SigningKey,
}

impl SessionKey {
    /// Generate a new random session keypair using the OS RNG.
    pub fn generate() -> Self {
        let signing = SigningKey::generate(&mut OsRng);
        Self { signing }
    }

    /// Returns the public key (Solana address) as a 32-byte array.
    pub fn pubkey(&self) -> [u8; 32] {
        self.signing.verifying_key().to_bytes()
    }

    /// Returns the public key in Solana's standard base58 representation.
    pub fn pubkey_base58(&self) -> String {
        bs58::encode(self.pubkey()).into_string()
    }

    /// Sign an arbitrary byte slice. Returns a 64-byte ed25519 signature.
    pub fn sign(&self, message: &[u8]) -> [u8; SIGNATURE_LENGTH] {
        self.signing.sign(message).to_bytes()
    }

    /// Serialize the 32-byte secret key as hex for storage.
    pub fn to_hex(&self) -> String {
        hex::encode(self.signing.to_bytes())
    }

    /// Reconstruct a SessionKey from its hex-encoded secret.
    pub fn from_hex(s: &str) -> Result<Self> {
        let bytes = hex::decode(s).map_err(|e| AppError::WalletBridge(format!("hex decode: {e}")))?;
        if bytes.len() != SECRET_KEY_LENGTH {
            return Err(AppError::WalletBridge(format!(
                "expected {SECRET_KEY_LENGTH} secret bytes, got {}",
                bytes.len()
            )));
        }
        let mut arr = [0u8; SECRET_KEY_LENGTH];
        arr.copy_from_slice(&bytes);
        let signing = SigningKey::from_bytes(&arr);
        Ok(Self { signing })
    }
}

/// Free-standing verify for any ed25519 signature against a pubkey + message.
/// Used by `authorization::verify_signature` for the master wallet's signature.
pub fn verify_signature(pubkey: &[u8; 32], message: &[u8], signature: &[u8; 64]) -> Result<()> {
    let verifying = VerifyingKey::from_bytes(pubkey)
        .map_err(|_| AppError::WalletInvalidSignature)?;
    let sig = ed25519_dalek::Signature::from_bytes(signature);
    verifying
        .verify_strict(message, &sig)
        .map_err(|_| AppError::WalletInvalidSignature)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_then_pubkey_is_32_bytes() {
        let key = SessionKey::generate();
        let pk = key.pubkey();
        assert_eq!(pk.len(), 32);
    }

    #[test]
    fn sign_and_self_verify_roundtrip() {
        let key = SessionKey::generate();
        let msg = b"hello mantic";
        let sig = key.sign(msg);
        verify_signature(&key.pubkey(), msg, &sig).unwrap();
    }

    #[test]
    fn verify_rejects_tampered_message() {
        let key = SessionKey::generate();
        let sig = key.sign(b"original");
        let err = verify_signature(&key.pubkey(), b"tampered", &sig).unwrap_err();
        assert!(matches!(err, AppError::WalletInvalidSignature));
    }

    #[test]
    fn verify_rejects_wrong_pubkey() {
        let a = SessionKey::generate();
        let b = SessionKey::generate();
        let msg = b"hello";
        let sig = a.sign(msg);
        let err = verify_signature(&b.pubkey(), msg, &sig).unwrap_err();
        assert!(matches!(err, AppError::WalletInvalidSignature));
    }

    #[test]
    fn hex_roundtrip_preserves_signing_capability() {
        let key = SessionKey::generate();
        let hex = key.to_hex();
        let key2 = SessionKey::from_hex(&hex).unwrap();
        let msg = b"persistence test";
        let sig = key2.sign(msg);
        verify_signature(&key.pubkey(), msg, &sig).unwrap();
    }

    #[test]
    fn pubkey_base58_is_valid_solana_format() {
        let key = SessionKey::generate();
        let bs58_str = key.pubkey_base58();
        // Solana pubkeys are typically 43-44 base58 chars
        assert!(bs58_str.len() >= 32 && bs58_str.len() <= 44);
        let decoded = bs58::decode(&bs58_str).into_vec().unwrap();
        assert_eq!(decoded, key.pubkey().to_vec());
    }
}
