use crate::account::{decode_claims, AccountInfo};
use crate::error::Result;
use crate::license::KeyringStore;
use crate::pairing;
use std::sync::Arc;
use tauri::State;

const DEFAULT_SERVER: &str = "http://localhost:4001";

pub fn server_url() -> String {
    std::env::var("MANTIC_LICENSE_SERVER").unwrap_or_else(|_| DEFAULT_SERVER.to_string())
}

#[tauri::command]
pub async fn pair_with_code(
    code: String,
    keyring: State<'_, Arc<KeyringStore>>,
) -> Result<AccountInfo> {
    let token = pairing::pair(&server_url(), &code).await?;
    let info = decode_claims(&token)?;
    keyring.save(&token)?;
    Ok(info)
}

#[tauri::command]
pub fn current_account(keyring: State<'_, Arc<KeyringStore>>) -> Result<AccountInfo> {
    let token = keyring.load()?;
    decode_claims(&token)
}

#[tauri::command]
pub fn sign_out(keyring: State<'_, Arc<KeyringStore>>) -> Result<()> {
    keyring.clear()
}
