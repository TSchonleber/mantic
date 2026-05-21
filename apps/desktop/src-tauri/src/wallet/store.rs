use crate::error::{AppError, Result};
use crate::wallet::authorization::StoredAuthorization;
use crate::wallet::session::SessionKey;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

const SERVICE: &str = "org.brainctl.mantic";
const KEY_SESSION_PRIV: &str = "wallet-session-privkey";
const KEY_MASTER_PUB: &str = "wallet-master-pubkey";
const KEY_AUTHORIZATION: &str = "wallet-authorization";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WalletCredentials {
    pub session_pubkey_b58: String,
    pub master_pubkey_b58: String,
    pub authorization: StoredAuthorization,
}

pub struct WalletStore {
    session_entry: OnceLock<std::result::Result<keyring::Entry, keyring::Error>>,
    master_entry: OnceLock<std::result::Result<keyring::Entry, keyring::Error>>,
    auth_entry: OnceLock<std::result::Result<keyring::Entry, keyring::Error>>,
}

impl WalletStore {
    pub fn new() -> Self {
        Self {
            session_entry: OnceLock::new(),
            master_entry: OnceLock::new(),
            auth_entry: OnceLock::new(),
        }
    }

    fn entry<'a>(
        &self,
        cell: &'a OnceLock<std::result::Result<keyring::Entry, keyring::Error>>,
        key: &str,
    ) -> Result<&'a keyring::Entry> {
        #[cfg(test)]
        {
            static TEST_INIT: std::sync::Once = std::sync::Once::new();
            TEST_INIT.call_once(|| {
                keyring::set_default_credential_builder(keyring::mock::default_credential_builder());
            });
        }
        let c = cell.get_or_init(|| keyring::Entry::new(SERVICE, key));
        c.as_ref().map_err(|e| AppError::Keyring(clone_keyring_error(e)))
    }

    pub fn save(
        &self,
        session: &SessionKey,
        master_pubkey_b58: &str,
        authorization: &StoredAuthorization,
    ) -> Result<()> {
        self.entry(&self.session_entry, KEY_SESSION_PRIV)?
            .set_password(&session.to_hex())?;
        self.entry(&self.master_entry, KEY_MASTER_PUB)?
            .set_password(master_pubkey_b58)?;
        let auth_json = serde_json::to_string(authorization)?;
        self.entry(&self.auth_entry, KEY_AUTHORIZATION)?
            .set_password(&auth_json)?;
        Ok(())
    }

    pub fn load(&self) -> Result<(SessionKey, WalletCredentials)> {
        let priv_hex = self.entry(&self.session_entry, KEY_SESSION_PRIV)?.get_password()
            .map_err(|e| match e {
                keyring::Error::NoEntry => AppError::WalletNotConnected,
                other => AppError::Keyring(other),
            })?;
        let session = SessionKey::from_hex(&priv_hex)?;
        let master_pubkey_b58 = self.entry(&self.master_entry, KEY_MASTER_PUB)?.get_password()
            .map_err(|e| match e {
                keyring::Error::NoEntry => AppError::WalletNotConnected,
                other => AppError::Keyring(other),
            })?;
        let auth_json = self.entry(&self.auth_entry, KEY_AUTHORIZATION)?.get_password()
            .map_err(|e| match e {
                keyring::Error::NoEntry => AppError::WalletNotConnected,
                other => AppError::Keyring(other),
            })?;
        let authorization: StoredAuthorization = serde_json::from_str(&auth_json)?;
        let creds = WalletCredentials {
            session_pubkey_b58: session.pubkey_base58(),
            master_pubkey_b58,
            authorization,
        };
        Ok((session, creds))
    }

    pub fn status(&self) -> Result<Option<WalletCredentials>> {
        match self.load() {
            Ok((_, c)) => Ok(Some(c)),
            Err(AppError::WalletNotConnected) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn clear(&self) -> Result<()> {
        for (cell, key) in [
            (&self.session_entry, KEY_SESSION_PRIV),
            (&self.master_entry, KEY_MASTER_PUB),
            (&self.auth_entry, KEY_AUTHORIZATION),
        ] {
            let entry = self.entry(cell, key)?;
            match entry.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => {}
                Err(e) => return Err(AppError::Keyring(e)),
            }
        }
        Ok(())
    }
}

impl Default for WalletStore {
    fn default() -> Self {
        Self::new()
    }
}

fn clone_keyring_error(e: &keyring::Error) -> keyring::Error {
    keyring::Error::PlatformFailure(std::io::Error::other(format!("{e}")).into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wallet::authorization::{build_message, generate_nonce};
    use chrono::Utc;

    fn fixture() -> (SessionKey, String, StoredAuthorization) {
        let session = SessionKey::generate();
        let master = SessionKey::generate();
        let nonce = generate_nonce();
        let issued = Utc::now();
        let msg = build_message(&session.pubkey_base58(), &master.pubkey_base58(), issued, &nonce);
        let sig = master.sign(msg.as_bytes());
        let auth = StoredAuthorization {
            master_pubkey_b58: master.pubkey_base58(),
            session_pubkey_b58: session.pubkey_base58(),
            message: msg,
            signature_b58: bs58::encode(sig).into_string(),
            signed_at: Utc::now(),
        };
        (session, master.pubkey_base58(), auth)
    }

    fn reset(store: &WalletStore) {
        let _ = store.clear();
    }

    #[test]
    fn save_then_load_roundtrip() {
        let store = WalletStore::new();
        reset(&store);
        let (session, master_pub, auth) = fixture();
        let session_pub_b58 = session.pubkey_base58();
        store.save(&session, &master_pub, &auth).unwrap();

        let (loaded_session, loaded_creds) = store.load().unwrap();
        assert_eq!(loaded_session.pubkey_base58(), session_pub_b58);
        assert_eq!(loaded_creds.master_pubkey_b58, master_pub);
        assert_eq!(loaded_creds.authorization.signature_b58, auth.signature_b58);
    }

    #[test]
    fn status_returns_none_when_empty() {
        let store = WalletStore::new();
        reset(&store);
        assert!(store.status().unwrap().is_none());
    }

    #[test]
    fn clear_then_load_returns_wallet_not_connected() {
        let store = WalletStore::new();
        reset(&store);
        let (session, master_pub, auth) = fixture();
        store.save(&session, &master_pub, &auth).unwrap();
        store.clear().unwrap();
        let err = store.load().unwrap_err();
        assert!(matches!(err, AppError::WalletNotConnected));
    }

    #[test]
    fn clear_is_idempotent() {
        let store = WalletStore::new();
        reset(&store);
        store.clear().unwrap();
        store.clear().unwrap();
    }
}
