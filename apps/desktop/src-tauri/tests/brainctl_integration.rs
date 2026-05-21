//! Integration test against the real brainctl-mcp sidecar binary.
//! Skipped automatically if the binary isn't present at the expected path.

use desktop_lib::brainctl_client::BrainctlClient;
use std::path::{Path, PathBuf};

fn find_sidecar() -> Option<PathBuf> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("binaries")
        .join("brainctl-mcp-aarch64-apple-darwin");
    if p.exists() { Some(p) } else { None }
}

#[tokio::test]
async fn memory_add_round_trip_against_real_binary() {
    let Some(bin) = find_sidecar() else {
        eprintln!("[skip] brainctl-mcp sidecar not present at binaries/brainctl-mcp-aarch64-apple-darwin");
        return;
    };

    let tmp = tempfile::NamedTempFile::new().unwrap();
    let db_path = tmp.path().to_path_buf();
    drop(tmp); // we want only the path, brainctl creates the file itself

    let client = BrainctlClient::new(bin, db_path.clone(), "integration-test-agent");
    let result = client
        .memory_add(
            "integration test entry",
            "lesson",
            Some("project:mantic-test"),
            None,
        )
        .await;
    client.shutdown().await;

    // brainctl-mcp auto-migrates a fresh brain.db on startup (see
    // _ensure_db_initialized in agentmemory/mcp_server.py), so an empty
    // path passed by Mantic is expected to come back with a real
    // memory_id, not a `no such table: agents` error.
    match result {
        Ok(v) => {
            eprintln!("brainctl-mcp memory_add response: {v}");
            let obj = v
                .as_object()
                .unwrap_or_else(|| panic!("expected JSON object, got: {v}"));
            assert!(
                obj.get("error").is_none(),
                "brainctl-mcp returned an error after migrate fix: {v}"
            );
            assert!(
                obj.get("memory_id").and_then(|m| m.as_i64()).is_some(),
                "expected numeric memory_id in response, got: {v}"
            );
            assert_eq!(
                obj.get("ok").and_then(|b| b.as_bool()),
                Some(true),
                "expected ok=true in response, got: {v}"
            );
        }
        Err(e) => panic!("brainctl-mcp memory_add failed: {e}"),
    }

    // brainctl-mcp should have created the db file
    assert!(Path::new(&db_path).exists(), "brain.db not created at {db_path:?}");

    // cleanup
    let _ = std::fs::remove_file(&db_path);
}
