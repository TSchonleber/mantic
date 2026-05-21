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

const LLM_SERVICE: &str = "org.brainctl.mantic";
const LLM_KEY_PREFIX: &str = "agent-llm-anthropic-key-";

pub struct LlmKeyStore;

impl LlmKeyStore {
    pub fn new() -> Self {
        Self
    }

    fn ensure_test_builder() {
        #[cfg(test)]
        {
            static TEST_INIT: std::sync::Once = std::sync::Once::new();
            TEST_INIT.call_once(|| {
                keyring::set_default_credential_builder(
                    keyring::mock::default_credential_builder(),
                );
            });
        }
    }

    fn entry(&self, agent_id: &str) -> Result<keyring::Entry> {
        Self::ensure_test_builder();
        let slot = format!("{LLM_KEY_PREFIX}{agent_id}");
        keyring::Entry::new(LLM_SERVICE, &slot).map_err(AppError::Keyring)
    }

    pub fn save(&self, agent_id: &str, api_key: &str) -> Result<()> {
        #[cfg(test)]
        {
            test_llm_keys()
                .lock()
                .unwrap()
                .insert(agent_id.to_string(), api_key.to_string());
            return Ok(());
        }
        #[cfg(not(test))]
        self.entry(agent_id)?.set_password(api_key)?;
        #[cfg(not(test))]
        Ok(())
    }

    pub fn load(&self, agent_id: &str) -> Result<Option<String>> {
        #[cfg(test)]
        {
            return Ok(test_llm_keys().lock().unwrap().get(agent_id).cloned());
        }
        #[cfg(not(test))]
        match self.entry(agent_id)?.get_password() {
            Ok(p) => Ok(Some(p)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(AppError::Keyring(e)),
        }
    }

    pub fn clear(&self, agent_id: &str) -> Result<()> {
        #[cfg(test)]
        {
            test_llm_keys().lock().unwrap().remove(agent_id);
            return Ok(());
        }
        #[cfg(not(test))]
        match self.entry(agent_id)?.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(AppError::Keyring(e)),
        }
    }
}

impl Default for LlmKeyStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
fn test_llm_keys() -> &'static std::sync::Mutex<std::collections::HashMap<String, String>> {
    static KEYS: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, String>>,
    > = std::sync::OnceLock::new();
    KEYS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
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

    #[test]
    fn llm_key_save_load_clear_roundtrip() {
        let store = LlmKeyStore::new();
        store.save("agent-7", "sk-test-abc").unwrap();
        assert_eq!(
            store.load("agent-7").unwrap().as_deref(),
            Some("sk-test-abc")
        );
        store.clear("agent-7").unwrap();
        assert!(store.load("agent-7").unwrap().is_none());
    }

    #[test]
    fn llm_key_load_unknown_returns_none() {
        let store = LlmKeyStore::new();
        assert!(store.load("nonexistent-agent-id-xyz").unwrap().is_none());
    }

    #[test]
    fn llm_key_clear_unknown_is_idempotent() {
        let store = LlmKeyStore::new();
        store.clear("nonexistent-agent-id-xyz").unwrap();
    }
}
