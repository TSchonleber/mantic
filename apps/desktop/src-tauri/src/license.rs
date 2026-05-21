use crate::error::{AppError, Result};
use std::sync::OnceLock;

const SERVICE: &str = "org.brainctl.mantic";
const KEY: &str = "license-jwt";

pub struct KeyringStore {
    entry: OnceLock<std::result::Result<keyring::Entry, keyring::Error>>,
}

impl KeyringStore {
    pub fn new() -> Self {
        Self {
            entry: OnceLock::new(),
        }
    }

    fn entry(&self) -> Result<&keyring::Entry> {
        #[cfg(test)]
        {
            static TEST_INIT: std::sync::Once = std::sync::Once::new();
            TEST_INIT.call_once(|| {
                keyring::set_default_credential_builder(keyring::mock::default_credential_builder());
            });
        }
        let cell = self.entry.get_or_init(|| keyring::Entry::new(SERVICE, KEY));
        cell.as_ref()
            .map_err(|e| AppError::Keyring(clone_keyring_error(e)))
    }

    pub fn save(&self, token: &str) -> Result<()> {
        self.entry()?.set_password(token)?;
        Ok(())
    }

    pub fn load(&self) -> Result<String> {
        match self.entry()?.get_password() {
            Ok(p) => Ok(p),
            Err(keyring::Error::NoEntry) => Err(AppError::NoLicense),
            Err(e) => Err(AppError::Keyring(e)),
        }
    }

    pub fn clear(&self) -> Result<()> {
        match self.entry()?.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(AppError::Keyring(e)),
        }
    }
}

impl Default for KeyringStore {
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

    fn reset(store: &KeyringStore) {
        let _ = store.clear();
    }

    #[test]
    fn save_then_load_roundtrip() {
        let store = KeyringStore::new();
        reset(&store);
        store.save("token-123").unwrap();
        assert_eq!(store.load().unwrap(), "token-123");
    }

    #[test]
    fn load_with_no_entry_returns_no_license() {
        let store = KeyringStore::new();
        reset(&store);
        match store.load() {
            Err(AppError::NoLicense) => {}
            other => panic!("expected NoLicense, got {other:?}"),
        }
    }

    #[test]
    fn clear_is_idempotent() {
        let store = KeyringStore::new();
        reset(&store);
        store.clear().unwrap();
        store.clear().unwrap();
    }
}
