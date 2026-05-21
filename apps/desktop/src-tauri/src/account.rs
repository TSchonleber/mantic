use crate::error::{AppError, Result};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct AccountInfo {
    pub account_id: String,
    pub tier: String,
    pub expires_at: i64,
}

#[derive(Debug, Deserialize)]
struct Claims {
    account_id: String,
    tier: String,
    exp: i64,
}

pub fn decode_claims(token: &str) -> Result<AccountInfo> {
    let mut validation = Validation::new(jsonwebtoken::Algorithm::HS256);
    validation.insecure_disable_signature_validation();
    validation.validate_exp = false;
    let data = decode::<Claims>(token, &DecodingKey::from_secret(&[]), &validation)?;
    let c = data.claims;
    let now = chrono::Utc::now().timestamp();
    if c.exp <= now {
        return Err(AppError::LicenseExpired);
    }
    Ok(AccountInfo {
        account_id: c.account_id,
        tier: c.tier,
        expires_at: c.exp,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{encode, EncodingKey, Header};
    use serde_json::json;

    fn make_token(account_id: &str, tier: &str, exp: i64) -> String {
        let claims = json!({ "account_id": account_id, "tier": tier, "exp": exp });
        encode(
            &Header::new(jsonwebtoken::Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(b"test"),
        )
        .unwrap()
    }

    #[test]
    fn decodes_a_valid_token() {
        let exp = chrono::Utc::now().timestamp() + 3600;
        let token = make_token("acct_42", "pro", exp);
        let info = decode_claims(&token).unwrap();
        assert_eq!(info.account_id, "acct_42");
        assert_eq!(info.tier, "pro");
        assert_eq!(info.expires_at, exp);
    }

    #[test]
    fn rejects_expired_token() {
        let exp = chrono::Utc::now().timestamp() - 1;
        let token = make_token("acct_42", "pro", exp);
        let err = decode_claims(&token).unwrap_err();
        assert!(matches!(err, AppError::LicenseExpired));
    }
}
