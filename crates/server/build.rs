use std::process::Command;

fn main() {
    // Re-run this script only when id-core source changes
    println!("cargo:rerun-if-changed=../id-core/src");
    println!("cargo:rerun-if-changed=../id-core/Cargo.toml");

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
        .expect("wasm-pack not found — run: cargo install wasm-pack");

    if !status.success() {
        panic!("wasm-pack build failed");
    }
}
