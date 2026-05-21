use crate::error::{AppError, Result};
use crate::wallet::authorization::{
    build_message, generate_nonce, verify_payload, AuthorizePayload, StoredAuthorization,
};
use crate::wallet::session::SessionKey;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use serde::Deserialize;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{oneshot, Mutex};
use tokio::task::JoinHandle;

const HOST: IpAddr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
const PORT_RANGE: std::ops::Range<u16> = 18421..18431;
const BRIDGE_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Debug, Deserialize)]
struct NonceQuery {
    nonce: String,
}

struct SharedState {
    expected_nonce: String,
    session_pubkey_b58: String,
    auth_message: String,
    result_tx: Mutex<Option<oneshot::Sender<Result<StoredAuthorization>>>>,
    bridge_html: String,
}

/// The result of a successful bridge round trip: the authorized credentials.
#[derive(Debug)]
pub struct BridgeOutcome {
    pub session_key: SessionKey,
    pub stored: StoredAuthorization,
}

/// One-shot bridge: spawns an axum server, opens the browser, waits for authorize POST,
/// shuts down. Caller is expected to await the returned future.
pub struct BridgeServer {
    state: Arc<SharedState>,
    bind_addr: SocketAddr,
    result_rx: oneshot::Receiver<Result<StoredAuthorization>>,
    server_task: JoinHandle<()>,
    session_key: Option<SessionKey>,
}

impl BridgeServer {
    /// Start the bridge. Returns the URL the user should be sent to, plus a handle
    /// that resolves to the authorized credentials when the user completes the flow.
    pub async fn start(bridge_html: String) -> Result<(String, Self)> {
        let session = SessionKey::generate();
        let session_pubkey_b58 = session.pubkey_base58();
        let nonce = generate_nonce();
        let issued = Utc::now();
        // The master_pubkey_b58 isn't known yet — the user picks it in the bridge.
        // We render the message with a placeholder master, then on POST we re-build
        // and verify the message the user actually signed (which the bridge JS will
        // re-fetch via /auth-message after wallet-connect).
        let placeholder_message = build_message(
            &session_pubkey_b58,
            "<pending>",
            issued,
            &nonce,
        );

        let (result_tx, result_rx) = oneshot::channel::<Result<StoredAuthorization>>();
        let state = Arc::new(SharedState {
            expected_nonce: nonce.clone(),
            session_pubkey_b58: session_pubkey_b58.clone(),
            auth_message: placeholder_message,
            result_tx: Mutex::new(Some(result_tx)),
            bridge_html,
        });

        let (listener, bind_addr) = bind_one_of(PORT_RANGE).await?;
        let app = Router::new()
            .route("/", get(serve_html))
            .route("/connect", get(serve_html))
            .route("/auth-message", get(auth_message_handler))
            .route("/authorize", post(authorize_handler))
            .with_state(state.clone());

        let server_task = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let url = format!(
            "http://{}:{}/connect?nonce={}",
            bind_addr.ip(),
            bind_addr.port(),
            nonce
        );
        Ok((
            url,
            Self {
                state,
                bind_addr,
                result_rx,
                server_task,
                session_key: Some(session),
            },
        ))
    }

    /// Await the user's authorization. Returns when:
    /// - the user successfully authorizes (Ok)
    /// - the bridge times out after 5 minutes (Err)
    pub async fn await_authorization(mut self) -> Result<BridgeOutcome> {
        let session = self.session_key.take().expect("session_key consumed twice");
        let result = tokio::select! {
            r = &mut self.result_rx => r.map_err(|_| AppError::WalletBridge("bridge closed unexpectedly".into()))?,
            _ = tokio::time::sleep(BRIDGE_TIMEOUT) => Err(AppError::WalletConnectionTimeout),
        };
        self.server_task.abort();
        let stored = result?;
        Ok(BridgeOutcome { session_key: session, stored })
    }

    pub fn bind_addr(&self) -> SocketAddr {
        self.bind_addr
    }
}

async fn bind_one_of(range: std::ops::Range<u16>) -> Result<(tokio::net::TcpListener, SocketAddr)> {
    for port in range {
        let addr = SocketAddr::new(HOST, port);
        if let Ok(l) = tokio::net::TcpListener::bind(addr).await {
            return Ok((l, addr));
        }
    }
    Err(AppError::WalletBridge(
        "no localhost port in 18421..18430 is available".into(),
    ))
}

async fn serve_html(State(state): State<Arc<SharedState>>) -> Html<String> {
    Html(state.bridge_html.clone())
}

#[derive(Debug, Deserialize)]
struct AuthMessageQuery {
    master: Option<String>,
    nonce: Option<String>,
}

async fn auth_message_handler(
    State(state): State<Arc<SharedState>>,
    Query(q): Query<AuthMessageQuery>,
) -> impl IntoResponse {
    let nonce_match = q.nonce.as_deref() == Some(&state.expected_nonce);
    if !nonce_match {
        return (StatusCode::BAD_REQUEST, "nonce mismatch".to_string());
    }
    // If the client passes ?master=<pubkey>, render the canonical message they
    // will sign. Otherwise return the placeholder.
    let master = q.master.unwrap_or_else(|| "<pending>".into());
    let issued = Utc::now();
    let msg = build_message(
        &state.session_pubkey_b58,
        &master,
        issued,
        &state.expected_nonce,
    );
    (StatusCode::OK, msg)
}

async fn authorize_handler(
    State(state): State<Arc<SharedState>>,
    Query(NonceQuery { nonce }): Query<NonceQuery>,
    Json(payload): Json<AuthorizePayload>,
) -> impl IntoResponse {
    let _ = nonce; // accepted but ignored; the nonce is in the payload and re-verified
    let outcome = verify_payload(&payload, &state.expected_nonce, &state.session_pubkey_b58);
    let mut tx = state.result_tx.lock().await;
    if let Some(sender) = tx.take() {
        let send_result = match outcome {
            Ok(s) => {
                let _ = sender.send(Ok(s));
                (StatusCode::OK, "ok".to_string())
            }
            Err(e) => {
                let msg = format!("{e}");
                let _ = sender.send(Err(e));
                (StatusCode::BAD_REQUEST, msg)
            }
        };
        send_result
    } else {
        (StatusCode::CONFLICT, "already authorized".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wallet::session::SessionKey;

    fn fixture_html() -> String {
        "<html>test</html>".to_string()
    }

    #[tokio::test]
    async fn start_returns_url_with_nonce() {
        let (url, server) = BridgeServer::start(fixture_html()).await.unwrap();
        assert!(url.starts_with("http://127.0.0.1:"));
        assert!(url.contains("/connect?nonce="));
        let _ = server.await_authorization_with_short_timeout().await;
    }

    #[tokio::test]
    async fn timeout_returns_connection_timeout_error() {
        let (_url, server) = BridgeServer::start(fixture_html()).await.unwrap();
        let err = server.await_authorization_with_short_timeout().await.unwrap_err();
        assert!(matches!(err, AppError::WalletConnectionTimeout));
    }

    #[tokio::test]
    async fn full_round_trip_succeeds_with_valid_signature() {
        let (url, server) = BridgeServer::start(fixture_html()).await.unwrap();
        // Parse the bind addr from the URL
        let port: u16 = url.split(':').nth(2).unwrap().split('/').next().unwrap().parse().unwrap();
        let nonce = url.split("nonce=").nth(1).unwrap().to_string();
        // We need the session pubkey to build the message the master "signs"
        // The bridge generated the session keypair internally; we can't see it
        // directly, but the /auth-message endpoint will tell us what message
        // would have been signed.
        let master = SessionKey::generate();
        let auth_msg_url = format!(
            "http://127.0.0.1:{port}/auth-message?nonce={nonce}&master={}",
            master.pubkey_base58()
        );
        let msg = reqwest::get(&auth_msg_url).await.unwrap().text().await.unwrap();

        let sig = master.sign(msg.as_bytes());
        let payload = AuthorizePayload {
            pubkey: master.pubkey_base58(),
            signature: bs58::encode(sig).into_string(),
            message: msg.clone(),
            nonce: nonce.clone(),
        };

        let client = reqwest::Client::new();
        let res = client
            .post(format!("http://127.0.0.1:{port}/authorize?nonce={nonce}"))
            .json(&payload)
            .send()
            .await
            .unwrap();
        assert_eq!(res.status().as_u16(), 200);

        let outcome = server.await_authorization().await.unwrap();
        assert_eq!(outcome.stored.master_pubkey_b58, master.pubkey_base58());
    }

    impl BridgeServer {
        async fn await_authorization_with_short_timeout(mut self) -> Result<BridgeOutcome> {
            let session = self.session_key.take().expect("session_key consumed twice");
            let result = tokio::select! {
                r = &mut self.result_rx => r.map_err(|_| AppError::WalletBridge("bridge closed unexpectedly".into()))?,
                _ = tokio::time::sleep(Duration::from_millis(200)) => Err(AppError::WalletConnectionTimeout),
            };
            self.server_task.abort();
            let stored = result?;
            Ok(BridgeOutcome { session_key: session, stored })
        }
    }
}
