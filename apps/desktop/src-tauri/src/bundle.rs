use crate::error::{AppError, Result};
use std::path::PathBuf;
use tauri::Manager;

/// Returns the path to the bundled brainctl-mcp sidecar binary.
///
/// In dev (`tauri dev` / `cargo run`), Tauri resolves this to
/// `src-tauri/binaries/brainctl-mcp-<target-triple>`.
/// In production, it's inside the app bundle's resources.
pub fn brainctl_mcp_path<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<PathBuf> {
    let resolver = app.path();
    let target_triple = target_triple();
    let candidate = resolver
        .resolve(
            format!("binaries/brainctl-mcp-{target_triple}"),
            tauri::path::BaseDirectory::Resource,
        )
        .map_err(|e| AppError::BrainctlUnavailable {
            reason: format!("could not resolve sidecar path: {e}"),
        })?;

    if !candidate.exists() {
        return Err(AppError::BrainctlUnavailable {
            reason: format!("bundled brainctl-mcp not found at {}", candidate.display()),
        });
    }
    Ok(candidate)
}

fn target_triple() -> &'static str {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "aarch64-apple-darwin"
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        "x86_64-apple-darwin"
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "x86_64-unknown-linux-gnu"
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        "x86_64-pc-windows-msvc"
    } else {
        "unknown"
    }
}
