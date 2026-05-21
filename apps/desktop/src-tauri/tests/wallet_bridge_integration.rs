//! End-to-end test of the wallet bridge: spawn the axum server, drive it via reqwest
//! as if we were the wallet-bridge JS page, verify a complete signed authorization
//! round-trips back into a StoredAuthorization.

use desktop_lib::wallet::authorization::AuthorizePayload;
use desktop_lib::wallet::bridge::BridgeServer;
use desktop_lib::wallet::session::SessionKey;

#[tokio::test]
async fn full_bridge_round_trip() {
    let html = "<html>test bridge</html>".to_string();
    let (url, server) = BridgeServer::start(html).await.unwrap();

    // Parse port + nonce from the URL
    let port: u16 = url
        .strip_prefix("http://127.0.0.1:")
        .unwrap()
        .split('/')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    let nonce = url.split("nonce=").nth(1).unwrap().to_string();

    // Pretend we're the browser-side JS: pick a "master" wallet
    let master = SessionKey::generate();
    let auth_msg_url = format!(
        "http://127.0.0.1:{port}/auth-message?nonce={nonce}&master={}",
        master.pubkey_base58()
    );
    let msg = reqwest::get(&auth_msg_url)
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    // Sign the message with the "master" wallet
    let sig = master.sign(msg.as_bytes());
    let payload = AuthorizePayload {
        pubkey: master.pubkey_base58(),
        signature: bs58::encode(sig).into_string(),
        message: msg.clone(),
        nonce: nonce.clone(),
    };

    // POST it back
    let client = reqwest::Client::new();
    let res = client
        .post(format!("http://127.0.0.1:{port}/authorize?nonce={nonce}"))
        .json(&payload)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status().as_u16(), 200);

    // Server completes the await with the verified authorization
    let outcome = server.await_authorization().await.unwrap();
    assert_eq!(outcome.stored.master_pubkey_b58, master.pubkey_base58());
    assert!(outcome
        .stored
        .message
        .contains(&outcome.stored.master_pubkey_b58));
    assert!(outcome
        .stored
        .message
        .contains(&outcome.session_key.pubkey_base58()));
}
