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

    let client = BrainctlClient::new(bin, db_path.clone());
    let result = client
        .memory_add(
            "integration test entry",
            "lesson",
            Some("project:mantic-test"),
            None,
        )
        .await;
    client.shutdown().await;

    // We expect either Ok with a memory_id field OR a deterministic brainctl error.
    // The point of this test is to confirm spawn + handshake + request/response works
    // end-to-end. The exact response shape may evolve; we just check it didn't blow up.
    match result {
        Ok(v) => {
            assert!(
                v.is_object() || v.is_null(),
                "expected JSON object/null, got: {v}"
            );
        }
        Err(e) => panic!("brainctl-mcp memory_add failed: {e}"),
    }

    // brainctl-mcp should have created the db file
    assert!(Path::new(&db_path).exists(), "brain.db not created at {db_path:?}");

    // cleanup
    let _ = std::fs::remove_file(&db_path);
}
