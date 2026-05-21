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

/// Per-agent Anthropic API key storage. Slot: `agent-llm-anthropic-key-<agent_id>`.
///
/// Entries are cached per `agent_id` so the mock credential builder used in
/// tests (which stores the password inside the `Entry` itself) sees the same
/// handle across `save`/`load`/`clear` calls. On real OS keychains the cache
/// is harmless — `Entry` is just a handle.
pub struct LlmKeyStore {
    entries: std::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<keyring::Entry>>>,
}

impl LlmKeyStore {
    pub fn new() -> Self {
        Self {
            entries: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }

    fn ensure_test_builder() {
        #[cfg(test)]
        {
            static TEST_INIT: std::sync::Once = std::sync::Once::new();
            TEST_INIT.call_once(|| {
                keyring::set_default_credential_builder(keyring::mock::default_credential_builder());
            });
        }
    }

    fn entry(&self, agent_id: &str) -> Result<std::sync::Arc<keyring::Entry>> {
        Self::ensure_test_builder();
        let mut map = self.entries.lock().unwrap();
        if let Some(e) = map.get(agent_id) {
            return Ok(e.clone());
        }
        let slot = format!("{LLM_KEY_PREFIX}{agent_id}");
        let entry = keyring::Entry::new(LLM_SERVICE, &slot).map_err(AppError::Keyring)?;
        let arc = std::sync::Arc::new(entry);
        map.insert(agent_id.to_string(), arc.clone());
        Ok(arc)
    }

    pub fn save(&self, agent_id: &str, api_key: &str) -> Result<()> {
        self.entry(agent_id)?.set_password(api_key)?;
        Ok(())
    }

    pub fn load(&self, agent_id: &str) -> Result<Option<String>> {
        match self.entry(agent_id)?.get_password() {
            Ok(p) => Ok(Some(p)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(AppError::Keyring(e)),
        }
    }

    pub fn clear(&self, agent_id: &str) -> Result<()> {
        match self.entry(agent_id)?.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(AppError::Keyring(e)),
        }
    }
}

impl Default for LlmKeyStore {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod llm_key_tests {
    use super::*;

    #[test]
    fn save_load_clear_roundtrip() {
        let store = LlmKeyStore::new();
        store.save("agent-7", "sk-test-abc").unwrap();
        assert_eq!(store.load("agent-7").unwrap().as_deref(), Some("sk-test-abc"));
        store.clear("agent-7").unwrap();
        assert!(store.load("agent-7").unwrap().is_none());
    }

    #[test]
    fn load_unknown_returns_none() {
        let store = LlmKeyStore::new();
        assert!(store.load("nonexistent-agent-id-xyz").unwrap().is_none());
    }

    #[test]
    fn clear_unknown_is_idempotent() {
        let store = LlmKeyStore::new();
        store.clear("nonexistent-agent-id-xyz").unwrap();
    }
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
