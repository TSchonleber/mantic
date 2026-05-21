use std::path::PathBuf;

fn main() {
    copy_wallet_bridge();
    tauri_build::build()
}

fn copy_wallet_bridge() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src = manifest_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("wallet-bridge")
        .join("dist")
        .join("index.html");
    let dst_dir = manifest_dir.join("resources");
    let dst = dst_dir.join("wallet-bridge.html");

    println!("cargo:rerun-if-changed={}", src.display());

    if !src.exists() {
        println!(
            "cargo:warning=wallet-bridge bundle not found at {}. Run `pnpm build:wallet-bridge` before tauri build.",
            src.display()
        );
        // Write an empty placeholder so cargo doesn't fail
        std::fs::create_dir_all(&dst_dir).ok();
        let _ = std::fs::write(&dst, b"<html><body>wallet-bridge not built</body></html>");
        return;
    }

    std::fs::create_dir_all(&dst_dir).expect("create resources dir");
    std::fs::copy(&src, &dst).expect("copy wallet-bridge.html");
}
