use std::process::Command;

fn main() {
    // Re-run this script only when id-core source changes
    println!("cargo:rerun-if-changed=../id-core/src");
    println!("cargo:rerun-if-changed=../id-core/Cargo.toml");

    let status = Command::new("wasm-pack")
        .args([
            "build",
            "crates/id-core",
            "--target", "web",
            "--out-dir", "../../frontend/pkg",
            "--release",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR").to_string() + "/../..")
        .status()
        .expect("failed to run wasm-pack; is it installed? (cargo install wasm-pack)");

    if !status.success() {
        panic!("wasm-pack build failed");
    }
}
