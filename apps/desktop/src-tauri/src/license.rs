use crate::error::{AppError, Result};
use std::sync::OnceLock;

const SERVICE: &str = "org.brainctl.mantic";
const KEY: &str = "license-jwt";

fn entry() -> Result<&'static keyring::Entry> {
    static ENTRY: OnceLock<std::result::Result<keyring::Entry, keyring::Error>> = OnceLock::new();
    #[cfg(test)]
    {
        static TEST_INIT: std::sync::Once = std::sync::Once::new();
        TEST_INIT.call_once(|| {
            keyring::set_default_credential_builder(keyring::mock::default_credential_builder());
        });
    }
    let cell = ENTRY.get_or_init(|| keyring::Entry::new(SERVICE, KEY));
    cell.as_ref().map_err(|e| AppError::Keyring(clone_keyring_error(e)))
}

// keyring::Error is not Clone in v3 — we need a small adapter.
// The only realistic init failures are PlatformFailure / NoStorageAccess.
// We can't truly clone, but we can wrap a fresh PlatformFailure with a
// stringified message. This branch is unreachable in practice because
// keyring::Entry::new only fails on truly broken platforms.
fn clone_keyring_error(e: &keyring::Error) -> keyring::Error {
    keyring::Error::PlatformFailure(std::io::Error::other(format!("{e}")).into())
}

pub fn save(token: &str) -> Result<()> {
    entry()?.set_password(token)?;
    Ok(())
}

pub fn load() -> Result<String> {
    match entry()?.get_password() {
        Ok(p) => Ok(p),
        Err(keyring::Error::NoEntry) => Err(AppError::NoLicense),
        Err(e) => Err(AppError::Keyring(e)),
    }
}

pub fn clear() -> Result<()> {
    match entry()?.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(AppError::Keyring(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reset() {
        // Force entry() to initialize (which installs the mock builder via TEST_INIT)
        // and clear any leftover state from previous tests.
        let _ = clear();
    }

    #[test]
    fn save_then_load_roundtrip() {
        reset();
        save("token-123").unwrap();
        assert_eq!(load().unwrap(), "token-123");
    }

    #[test]
    fn load_with_no_entry_returns_no_license() {
        reset();
        match load() {
            Err(AppError::NoLicense) => {}
            other => panic!("expected NoLicense, got {other:?}"),
        }
    }

    #[test]
    fn clear_is_idempotent() {
        reset();
        clear().unwrap();
        clear().unwrap();
    }
}
