use crate::brain_db::BrainDb;
use crate::brainctl_client::BrainctlClient;
use crate::license::KeyringStore;
use crate::error::{AppError, Result};
use std::path::PathBuf;
use std::sync::Arc;

/// Holds all the long-lived components of the app.
pub struct AppState {
    pub keyring: Arc<KeyringStore>,
    pub brain: Arc<BrainDb>,
    pub brainctl: Arc<BrainctlClient>,
}

impl AppState {
    pub fn new(brain_db_path: PathBuf, brainctl_binary: PathBuf) -> Result<Self> {
        let keyring = Arc::new(KeyringStore::new());
        // brainctl-mcp owns brain.db creation on first init. For now, if the file
        // doesn't exist yet, surface a clear error from BrainDb construction so
        // the user knows to launch brainctl-mcp once. (Tauri setup will hit this
        // path on first launch; we tolerate the failure and defer to lazy retry
        // via try_open_brain).
        let brain = Arc::new(BrainDb::open_read_only(&brain_db_path).or_else(|_| {
            // Bootstrap an empty brain.db with minimal schema so reads return
            // sensible defaults even before brainctl-mcp ever runs.
            bootstrap_empty_brain(&brain_db_path)?;
            BrainDb::open_read_only(&brain_db_path)
        })?);
        let brainctl = Arc::new(BrainctlClient::new(brainctl_binary, brain_db_path));
        Ok(Self {
            keyring,
            brain,
            brainctl,
        })
    }
}

fn bootstrap_empty_brain(path: &std::path::Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| AppError::BrainctlUnavailable {
            reason: format!("failed to create brain.db parent: {e}"),
        })?;
    }
    let conn = rusqlite::Connection::open(path)?;
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS memories (
            id INTEGER PRIMARY KEY,
            category TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
        );
        CREATE TABLE IF NOT EXISTS events (
            id INTEGER PRIMARY KEY,
            event_type TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
        );
        "#,
    )?;
    Ok(())
}
