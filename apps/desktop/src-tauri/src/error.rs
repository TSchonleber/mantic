use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("keyring error: {0}")]
    Keyring(#[from] keyring::Error),

    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("jwt error: {0}")]
    Jwt(#[from] jsonwebtoken::errors::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("invalid pairing code")]
    InvalidCode,

    #[error("no license stored")]
    NoLicense,

    #[error("license expired")]
    LicenseExpired,

    #[error("server returned {status}: {body}")]
    Server { status: u16, body: String },

    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("brainctl returned error {code}: {message}")]
    Brainctl { code: i64, message: String },

    #[error("brainctl unavailable: {reason}")]
    BrainctlUnavailable { reason: String },

    #[error("wallet not connected")]
    WalletNotConnected,

    #[error("wallet connect already in progress")]
    WalletConnectInProgress,

    #[error("wallet connection timeout")]
    WalletConnectionTimeout,

    #[error("wallet signature verification failed")]
    WalletInvalidSignature,

    #[error("wallet nonce mismatch")]
    WalletNonceMismatch,

    #[error("wallet bridge error: {0}")]
    WalletBridge(String),

    #[error("bs58 decode error: {0}")]
    Bs58(#[from] bs58::decode::Error),
}

impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
