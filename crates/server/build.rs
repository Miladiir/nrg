use std::process::Command;

fn main() {
    // Re-run this script only when id-core source changes
    println!("cargo:rerun-if-changed=../id-core/src");
    println!("cargo:rerun-if-changed=../id-core/Cargo.toml");

    // Install wasm-pack if not present
    if Command::new("wasm-pack").arg("--version").output().is_err() {
        eprintln!("wasm-pack not found, installing via cargo install...");
        let install = Command::new("cargo")
            .args(["install", "wasm-pack"])
            .status()
            .expect("failed to run cargo install wasm-pack");
        if !install.success() {
            panic!("failed to install wasm-pack");
        }
    }

    let workspace = env!("CARGO_MANIFEST_DIR").to_string() + "/../..";
    let status = Command::new("wasm-pack")
        .args([
            "build",
            "crates/id-core",
            "--target", "web",
            "--out-dir", "../../frontend/pkg",
            "--release",
        ])
        .current_dir(&workspace)
        .status()
        .expect("failed to run wasm-pack");

    if !status.success() {
        panic!("wasm-pack build failed");
    }
}
