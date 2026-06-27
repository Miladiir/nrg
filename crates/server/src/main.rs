use axum::{
    extract::Json,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use tower_http::{cors::CorsLayer, services::ServeDir};
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;

use id_core::{
    generate_malo, generate_melo, generate_nelo, validate_malo, validate_melo, validate_nelo,
};

// ─────────────────────────────────────────────────────────────────────────────
// Request / Response types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Deserialize, ToSchema)]
struct ValidateRequest {
    /// The ID to validate
    id: String,
}

#[derive(Serialize, ToSchema)]
struct GenerateMaloResponse {
    /// Generated MaLo-ID (11 digits)
    id: String,
    /// Check digit (last digit)
    checksum: u8,
    /// Issuing authority: DVGW or BDEW
    issuer: String,
}

#[derive(Serialize, ToSchema)]
struct ValidateMaloResponse {
    /// Whether the ID is valid
    valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Check digit if valid
    checksum: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Issuing authority if valid
    issuer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Validation error message if invalid
    error: Option<String>,
}

#[derive(Serialize, ToSchema)]
struct GenerateMeloResponse {
    /// Generated MeLo-ID (33 characters: DE + 6-digit network operator + 5-digit postal code + 20-char alphanumeric meter point)
    id: String,
}

#[derive(Serialize, ToSchema)]
struct ErrorResponse {
    /// HTTP error description
    error: String,
}

#[derive(Serialize, ToSchema)]
struct ValidateMeloResponse {
    /// Whether the ID is valid
    valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Validation error message if invalid
    error: Option<String>,
}

#[derive(Serialize, ToSchema)]
struct GenerateNeloResponse {
    /// Generated NeLo-ID (11 characters: E + 9 alphanumeric + check digit)
    id: String,
    /// Check digit (last character)
    checksum: u8,
}

#[derive(Serialize, ToSchema)]
struct ValidateNeloResponse {
    /// Whether the ID is valid
    valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Validation error message if invalid
    error: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Handlers
// ─────────────────────────────────────────────────────────────────────────────

/// Generate a valid Marktlokations-ID (MaLo-ID)
#[utoipa::path(
    get,
    path = "/api/malo/generate",
    responses(
        (status = 200, description = "Generated MaLo-ID", body = GenerateMaloResponse)
    ),
    tag = "MaLo-ID"
)]
async fn handle_malo_generate() -> impl IntoResponse {
    let id = generate_malo();
    let info = validate_malo(&id).unwrap();
    Json(GenerateMaloResponse {
        id: info.id,
        checksum: info.checksum,
        issuer: info.issuer.to_string(),
    })
}

/// Validate a Marktlokations-ID (MaLo-ID)
#[utoipa::path(
    post,
    path = "/api/malo/validate",
    request_body = ValidateRequest,
    responses(
        (status = 200, description = "Validation result (check `valid` field for outcome)", body = ValidateMaloResponse),
        (status = 422, description = "Unprocessable Entity — malformed JSON or missing `id` field", body = ErrorResponse),
    ),
    tag = "MaLo-ID"
)]
async fn handle_malo_validate(Json(req): Json<ValidateRequest>) -> impl IntoResponse {
    match validate_malo(&req.id) {
        Ok(info) => Json(ValidateMaloResponse {
            valid: true,
            checksum: Some(info.checksum),
            issuer: Some(info.issuer.to_string()),
            error: None,
        }),
        Err(e) => Json(ValidateMaloResponse {
            valid: false,
            checksum: None,
            issuer: None,
            error: Some(e),
        }),
    }
}

/// Generate a valid Messlokations-ID (MeLo-ID)
#[utoipa::path(
    get,
    path = "/api/melo/generate",
    responses(
        (status = 200, description = "Generated MeLo-ID", body = GenerateMeloResponse)
    ),
    tag = "MeLo-ID"
)]
async fn handle_melo_generate() -> impl IntoResponse {
    Json(GenerateMeloResponse { id: generate_melo() })
}

/// Validate a Messlokations-ID (MeLo-ID)
#[utoipa::path(
    post,
    path = "/api/melo/validate",
    request_body = ValidateRequest,
    responses(
        (status = 200, description = "Validation result (check `valid` field for outcome)", body = ValidateMeloResponse),
        (status = 422, description = "Unprocessable Entity — malformed JSON or missing `id` field", body = ErrorResponse),
    ),
    tag = "MeLo-ID"
)]
async fn handle_melo_validate(Json(req): Json<ValidateRequest>) -> impl IntoResponse {
    match validate_melo(&req.id) {
        Ok(()) => Json(ValidateMeloResponse { valid: true, error: None }),
        Err(e) => Json(ValidateMeloResponse { valid: false, error: Some(e) }),
    }
}

/// Generate a valid Netzlokations-ID (NeLo-ID)
#[utoipa::path(
    get,
    path = "/api/nelo/generate",
    responses(
        (status = 200, description = "Generated NeLo-ID", body = GenerateNeloResponse)
    ),
    tag = "NeLo-ID"
)]
async fn handle_nelo_generate() -> impl IntoResponse {
    let id = generate_nelo();
    let checksum = id.chars().last().unwrap().to_digit(10).unwrap() as u8;
    Json(GenerateNeloResponse { id, checksum })
}

/// Validate a Netzlokations-ID (NeLo-ID)
#[utoipa::path(
    post,
    path = "/api/nelo/validate",
    request_body = ValidateRequest,
    responses(
        (status = 200, description = "Validation result (check `valid` field for outcome)", body = ValidateNeloResponse),
        (status = 422, description = "Unprocessable Entity — malformed JSON or missing `id` field", body = ErrorResponse),
    ),
    tag = "NeLo-ID"
)]
async fn handle_nelo_validate(Json(req): Json<ValidateRequest>) -> impl IntoResponse {
    match validate_nelo(&req.id) {
        Ok(()) => Json(ValidateNeloResponse { valid: true, error: None }),
        Err(e) => Json(ValidateNeloResponse { valid: false, error: Some(e) }),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// OpenAPI spec
// ─────────────────────────────────────────────────────────────────────────────

#[derive(OpenApi)]
#[openapi(
    paths(
        handle_malo_generate,
        handle_malo_validate,
        handle_melo_generate,
        handle_melo_validate,
        handle_nelo_generate,
        handle_nelo_validate,
    ),
    components(schemas(
        ValidateRequest,
        GenerateMaloResponse,
        ValidateMaloResponse,
        GenerateMeloResponse,
        ValidateMeloResponse,
        GenerateNeloResponse,
        ValidateNeloResponse,
        ErrorResponse,
    )),
    info(
        title = "NRG ID Generator API",
        version = "1.0.0",
        description = "Generate and validate German energy market location IDs (MaLo, MeLo, NeLo)"
    )
)]
struct ApiDoc;

// ─────────────────────────────────────────────────────────────────────────────
// Main
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let api_routes = Router::new()
        .route("/malo/generate", get(handle_malo_generate))
        .route("/malo/validate", post(handle_malo_validate))
        .route("/melo/generate", get(handle_melo_generate))
        .route("/melo/validate", post(handle_melo_validate))
        .route("/nelo/generate", get(handle_nelo_generate))
        .route("/nelo/validate", post(handle_nelo_validate));

    let swagger = SwaggerUi::new("/swagger-ui")
        .url("/api-docs/openapi.json", ApiDoc::openapi());

    let app = Router::new()
        .merge(swagger)
        .nest("/api", api_routes)
        .fallback_service(ServeDir::new("frontend"))
        .layer(CorsLayer::permissive());

    let addr = "0.0.0.0:8080";
    println!("Server running at http://{}", addr);
    println!("Swagger UI at http://{}/swagger-ui", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

