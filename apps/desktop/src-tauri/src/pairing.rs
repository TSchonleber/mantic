use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
struct PairRequest<'a> {
    code: &'a str,
}

#[derive(Debug, Deserialize)]
struct PairResponse {
    token: String,
}

pub async fn pair(server_url: &str, code: &str) -> Result<String> {
    if code.trim().is_empty() {
        return Err(AppError::InvalidCode);
    }
    let client = reqwest::Client::new();
    let res = client
        .post(format!("{server_url}/v1/pair"))
        .json(&PairRequest { code })
        .send()
        .await?;

    let status = res.status();
    if !status.is_success() {
        let body = res.text().await.unwrap_or_default();
        if status.as_u16() == 400 {
            return Err(AppError::InvalidCode);
        }
        return Err(AppError::Server {
            status: status.as_u16(),
            body,
        });
    }

    let body: PairResponse = res.json().await?;
    Ok(body.token)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;

    #[tokio::test]
    async fn pair_returns_token_on_200() {
        let mut server = Server::new_async().await;
        let m = server
            .mock("POST", "/v1/pair")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"token":"abc.def.ghi"}"#)
            .create_async()
            .await;

        let token = pair(&server.url(), "654321").await.unwrap();
        assert_eq!(token, "abc.def.ghi");
        m.assert_async().await;
    }

    #[tokio::test]
    async fn pair_rejects_empty_code_without_calling_server() {
        let err = pair("http://unused", "").await.unwrap_err();
        assert!(matches!(err, AppError::InvalidCode));
    }

    #[tokio::test]
    async fn pair_maps_400_to_invalid_code() {
        let mut server = Server::new_async().await;
        server
            .mock("POST", "/v1/pair")
            .with_status(400)
            .with_body(r#"{"error":"invalid_code"}"#)
            .create_async()
            .await;

        let err = pair(&server.url(), "12").await.unwrap_err();
        assert!(matches!(err, AppError::InvalidCode));
    }
}
