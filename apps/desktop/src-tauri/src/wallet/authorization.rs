use crate::error::{AppError, Result};
use crate::wallet::session::verify_signature;
use chrono::{DateTime, Utc};
use rand::RngCore;
use serde::{Deserialize, Serialize};

/// Builds the human-readable authorization message that Phantom shows to the user.
///
/// The format is fixed text with five substituted values: session pubkey, master pubkey,
/// ISO-8601 issued time, and a 16-byte random nonce.
pub fn build_message(
    session_pubkey_b58: &str,
    master_pubkey_b58: &str,
    issued_at: DateTime<Utc>,
    nonce_hex: &str,
) -> String {
    format!(
        "Mantic Session Authorization\n\
         \n\
         I authorize Mantic to use the following session wallet for autonomous\n\
         trading on my behalf:\n\
         \n\
           Session wallet: {session_pubkey_b58}\n\
         \n\
         I understand:\n\
           - I will fund this wallet manually from my master wallet\n\
           - Mantic has full control over the session wallet's contents\n\
           - This authorization is recorded off-chain only\n\
         \n\
         Master wallet: {master_pubkey_b58}\n\
         Issued: {issued_at}\n\
         Nonce: {nonce_hex}",
        issued_at = issued_at.to_rfc3339(),
    )
}

/// Generate a fresh 16-byte random nonce as a hex string.
pub fn generate_nonce() -> String {
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// What the bridge HTTP handler receives from the browser when the user signs.
#[derive(Debug, Deserialize)]
pub struct AuthorizePayload {
    pub pubkey: String,    // base58
    pub signature: String, // base58
    pub message: String,
    pub nonce: String,
}

/// What we persist locally after successful verification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoredAuthorization {
    pub master_pubkey_b58: String,
    pub session_pubkey_b58: String,
    pub message: String,
    pub signature_b58: String,
    pub signed_at: DateTime<Utc>,
}

/// Verify the user's signature and return a StoredAuthorization.
/// Caller is responsible for matching the nonce against the expected value.
pub fn verify_payload(
    payload: &AuthorizePayload,
    expected_nonce: &str,
    expected_session_pubkey_b58: &str,
) -> Result<StoredAuthorization> {
    if payload.nonce != expected_nonce {
        return Err(AppError::WalletNonceMismatch);
    }
    if !payload.message.contains(expected_session_pubkey_b58) {
        return Err(AppError::WalletBridge(
            "session pubkey not present in signed message".into(),
        ));
    }
    let pubkey_bytes = bs58::decode(&payload.pubkey).into_vec()?;
    if pubkey_bytes.len() != 32 {
        return Err(AppError::WalletBridge(format!(
            "pubkey is {} bytes, expected 32",
            pubkey_bytes.len()
        )));
    }
    let mut pk = [0u8; 32];
    pk.copy_from_slice(&pubkey_bytes);

    let sig_bytes = bs58::decode(&payload.signature).into_vec()?;
    if sig_bytes.len() != 64 {
        return Err(AppError::WalletBridge(format!(
            "signature is {} bytes, expected 64",
            sig_bytes.len()
        )));
    }
    let mut sig = [0u8; 64];
    sig.copy_from_slice(&sig_bytes);

    verify_signature(&pk, payload.message.as_bytes(), &sig)?;

    Ok(StoredAuthorization {
        master_pubkey_b58: payload.pubkey.clone(),
        session_pubkey_b58: expected_session_pubkey_b58.to_string(),
        message: payload.message.clone(),
        signature_b58: payload.signature.clone(),
        signed_at: Utc::now(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wallet::session::SessionKey;

    #[test]
    fn build_message_contains_all_fields() {
        let session = "SessSesssessSessAddr".to_string();
        let master = "MASTERmasterMasterAddr".to_string();
        let issued = "2026-05-21T12:00:00Z".parse::<DateTime<Utc>>().unwrap();
        let nonce = "deadbeef".to_string();
        let msg = build_message(&session, &master, issued, &nonce);
        assert!(msg.contains(&session));
        assert!(msg.contains(&master));
        assert!(msg.contains("2026-05-21"));
        assert!(msg.contains(&nonce));
        assert!(msg.starts_with("Mantic Session Authorization"));
    }

    #[test]
    fn generate_nonce_is_32_hex_chars() {
        let n = generate_nonce();
        assert_eq!(n.len(), 32);
        assert!(hex::decode(&n).is_ok());
    }

    #[test]
    fn verify_payload_happy_path() {
        // Use a SessionKey as a stand-in for the master wallet (also ed25519).
        let master = SessionKey::generate();
        let session_pubkey_b58 = "SessionFakePubkey1234567890";

        let issued = Utc::now();
        let nonce = generate_nonce();
        let msg = build_message(
            session_pubkey_b58,
            &master.pubkey_base58(),
            issued,
            &nonce,
        );

        let sig_bytes = master.sign(msg.as_bytes());
        let payload = AuthorizePayload {
            pubkey: master.pubkey_base58(),
            signature: bs58::encode(sig_bytes).into_string(),
            message: msg.clone(),
            nonce: nonce.clone(),
        };

        let stored = verify_payload(&payload, &nonce, session_pubkey_b58).unwrap();
        assert_eq!(stored.master_pubkey_b58, master.pubkey_base58());
        assert_eq!(stored.session_pubkey_b58, session_pubkey_b58);
        assert_eq!(stored.message, msg);
    }

    #[test]
    fn verify_payload_rejects_nonce_mismatch() {
        let master = SessionKey::generate();
        let msg = "any text".to_string();
        let sig = master.sign(msg.as_bytes());
        let payload = AuthorizePayload {
            pubkey: master.pubkey_base58(),
            signature: bs58::encode(sig).into_string(),
            message: msg,
            nonce: "actual".into(),
        };
        let err = verify_payload(&payload, "expected", "any-session").unwrap_err();
        assert!(matches!(err, AppError::WalletNonceMismatch));
    }

    #[test]
    fn verify_payload_rejects_session_pubkey_not_in_message() {
        let master = SessionKey::generate();
        let nonce = "n0nce".to_string();
        // Build a message that does NOT contain the expected session pubkey
        let msg = "this message lacks the session pubkey".to_string();
        let sig = master.sign(msg.as_bytes());
        let payload = AuthorizePayload {
            pubkey: master.pubkey_base58(),
            signature: bs58::encode(sig).into_string(),
            message: msg,
            nonce: nonce.clone(),
        };
        let err = verify_payload(&payload, &nonce, "ExpectedSessionPubkey").unwrap_err();
        match err {
            AppError::WalletBridge(m) => assert!(m.contains("session pubkey")),
            other => panic!("expected WalletBridge, got {other:?}"),
        }
    }

    #[test]
    fn verify_payload_rejects_bad_signature() {
        let master = SessionKey::generate();
        let other = SessionKey::generate();
        let session_pubkey_b58 = "SessionFakePubkey1234567890";
        let issued = Utc::now();
        let nonce = generate_nonce();
        let msg = build_message(
            session_pubkey_b58,
            &master.pubkey_base58(),
            issued,
            &nonce,
        );
        // Sign with `other`, claim it's from `master`
        let sig = other.sign(msg.as_bytes());
        let payload = AuthorizePayload {
            pubkey: master.pubkey_base58(),
            signature: bs58::encode(sig).into_string(),
            message: msg,
            nonce: nonce.clone(),
        };
        let err = verify_payload(&payload, &nonce, session_pubkey_b58).unwrap_err();
        assert!(matches!(err, AppError::WalletInvalidSignature));
    }
}
