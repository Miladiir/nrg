//! Cloudflare Workers adapter.
//!
//! `worker-build` compiles this crate to `wasm32-unknown-unknown`. The generated
//! JavaScript is only the Workers runtime shim around this Rust/Wasm module.

use tower_service::Service;
use worker::{event, Context, Env, HttpRequest, Result};

const LEI_LOOKUP_PATH: &str = "/api/v1/business/organizations/lei/lookup";
const LEI_LOOKUP_RATE_LIMIT_BINDING: &str = "LEI_LOOKUP_RATE_LIMITER";
const LEI_LOOKUP_RATE_LIMIT_KEY: &str = "gleif-live-lookup";
const LEI_LOOKUP_EDGE_RETRY_AFTER_SECONDS: u64 = 60;

#[event(fetch)]
async fn fetch(
    request: HttpRequest,
    env: Env,
    _context: Context,
) -> Result<axum::http::Response<axum::body::Body>> {
    if request.method() == axum::http::Method::POST && request.uri().path() == LEI_LOOKUP_PATH {
        // A route-class key protects the GLEIF upstream without putting a full
        // LEI, client IP, or other high-cardinality identifier into the rate
        // limiter's counters, logs, or metrics.
        let outcome = env
            .rate_limiter(LEI_LOOKUP_RATE_LIMIT_BINDING)?
            .limit(LEI_LOOKUP_RATE_LIMIT_KEY.to_string())
            .await?;
        if !outcome.success {
            return Ok(nrg_api::lei_lookup_rate_limit_response(
                LEI_LOOKUP_EDGE_RETRY_AFTER_SECONDS,
            ));
        }
    }

    Ok(nrg_api::router().call(request).await?)
}
