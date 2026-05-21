use crate::account::decode_claims;
use crate::error::Result;
use crate::{license, pairing};
use std::time::Duration;

/// Returns true if the token expires within `refresh_window` seconds from now.
pub fn needs_refresh(expires_at: i64, now: i64, refresh_window_secs: i64) -> bool {
    expires_at - now <= refresh_window_secs
}

pub async fn tick(server_url: &str, refresh_window_secs: i64) -> Result<bool> {
    let token = match license::load() {
        Ok(t) => t,
        Err(_) => return Ok(false),
    };
    let info = match decode_claims(&token) {
        Ok(i) => i,
        Err(_) => return Ok(false),
    };
    let now = chrono::Utc::now().timestamp();
    if !needs_refresh(info.expires_at, now, refresh_window_secs) {
        return Ok(false);
    }
    let new_token = pairing::refresh(server_url, &token).await?;
    license::save(&new_token)?;
    Ok(true)
}

pub async fn run_forever(server_url: String, interval: Duration, refresh_window_secs: i64) {
    loop {
        let _ = tick(&server_url, refresh_window_secs).await;
        tokio::time::sleep(interval).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn needs_refresh_true_when_within_window() {
        assert!(needs_refresh(1000, 950, 60));
    }

    #[test]
    fn needs_refresh_false_when_outside_window() {
        assert!(!needs_refresh(1000, 800, 60));
    }

    #[test]
    fn needs_refresh_true_when_already_expired() {
        assert!(needs_refresh(800, 1000, 60));
    }
}
