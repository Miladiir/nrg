use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest_dir.join("../..").canonicalize().unwrap();
    let id_core = manifest_dir.join("../id-core").canonicalize().unwrap();

    // Absolute paths so Cargo can reliably track changes
    println!("cargo:rerun-if-changed={}", id_core.join("src").display());
    println!("cargo:rerun-if-changed={}", id_core.join("Cargo.toml").display());

    // Match wasm-pack profile to the active Cargo profile
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    let wasm_profile = if profile == "release" { "--release" } else { "--dev" };

    let status = Command::new("wasm-pack")
        .args([
            "build",
            "crates/id-core",
            "--target", "web",
            "--out-dir", "../../frontend/pkg",
            wasm_profile,
        ])
        .current_dir(&workspace)
        .status()
        .expect("wasm-pack not found — run: cargo install wasm-pack");

    if !status.success() {
        panic!("wasm-pack build failed");
    }
}
