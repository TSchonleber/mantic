use crate::account::{decode_claims, AccountInfo};
use crate::error::Result;
use crate::{license, pairing};

const DEFAULT_SERVER: &str = "http://localhost:4001";

fn server_url() -> String {
    std::env::var("MANTIC_LICENSE_SERVER").unwrap_or_else(|_| DEFAULT_SERVER.to_string())
}

#[tauri::command]
pub async fn pair_with_code(code: String) -> Result<AccountInfo> {
    let token = pairing::pair(&server_url(), &code).await?;
    license::save(&token)?;
    decode_claims(&token)
}

#[tauri::command]
pub fn current_account() -> Result<AccountInfo> {
    let token = license::load()?;
    decode_claims(&token)
}

#[tauri::command]
pub fn sign_out() -> Result<()> {
    license::clear()
}
