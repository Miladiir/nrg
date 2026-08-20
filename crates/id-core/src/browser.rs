//! WebAssembly exports for the browser frontend.
//!
//! The frontend generates and validates identifiers locally through these
//! wrappers around [`crate::ops`] — the same dispatch the HTTP API uses — so
//! both run the exact same code.

use wasm_bindgen::prelude::*;

use crate::ops::{self, GenerateOptions};

fn error_json(message: &str) -> String {
    serde_json::json!({ "error": message }).to_string()
}

/// Returns `{"values": [...]}` or `{"error": ...}` as JSON.
#[wasm_bindgen]
pub fn generate(slug: &str, options_json: &str) -> String {
    let source = if options_json.trim().is_empty() {
        "{}"
    } else {
        options_json
    };
    let options: GenerateOptions = match serde_json::from_str(source) {
        Ok(options) => options,
        Err(error) => return error_json(&format!("Invalid options: {error}")),
    };
    match ops::generate(slug, &options) {
        Ok(values) => serde_json::json!({ "values": values }).to_string(),
        Err(error) => error_json(&error.to_string()),
    }
}

/// Returns `{"valid": true}` or `{"valid": false, "error": ...}` as JSON.
#[wasm_bindgen]
pub fn validate(slug: &str, value: &str) -> String {
    match ops::validate(slug, value) {
        Ok(()) => r#"{"valid":true}"#.to_string(),
        Err(error) => serde_json::json!({ "valid": false, "error": error }).to_string(),
    }
}
