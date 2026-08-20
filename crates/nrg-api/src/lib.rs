//! Shared HTTP API used by both the native container server and Cloudflare Worker.

use axum::{
    extract::{rejection::JsonRejection, DefaultBodyLimit},
    http::{header::CONTENT_TYPE, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tower_http::cors::{Any, CorsLayer};
use utoipa::{OpenApi, ToSchema};

mod routes;

use routes::{business, energy, metering, payments, registers};

pub const OPENAPI_JSON_PATH: &str = "/api-docs/openapi.json";
pub const SWAGGER_UI_PATH: &str = "/swagger-ui";
const MAX_REQUEST_BODY_BYTES: usize = 1024;

#[derive(Debug, Deserialize, ToSchema)]
pub struct ValidateRequest {
    /// The ID to validate.
    pub id: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct ErrorResponse {
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_seconds: Option<u64>,
}

pub(crate) type ValidatePayload = Result<Json<ValidateRequest>, JsonRejection>;

#[derive(Debug)]
pub(crate) struct ApiError {
    status: StatusCode,
    message: String,
    retry_after_seconds: Option<u64>,
}

impl ApiError {
    fn invalid_json(rejection: JsonRejection) -> Self {
        let status = rejection.status();
        let message = match status {
            StatusCode::BAD_REQUEST => "Malformed JSON request body".to_string(),
            StatusCode::PAYLOAD_TOO_LARGE => {
                format!("Request body exceeds {MAX_REQUEST_BODY_BYTES} bytes")
            }
            StatusCode::UNSUPPORTED_MEDIA_TYPE => {
                "Content-Type must be application/json".to_string()
            }
            StatusCode::UNPROCESSABLE_ENTITY => {
                "JSON request body must contain a string field named 'id'".to_string()
            }
            _ => "Invalid JSON request body".to_string(),
        };

        Self {
            status,
            message,
            retry_after_seconds: None,
        }
    }

    pub(crate) fn invalid_generate_json(rejection: JsonRejection) -> Self {
        let status = rejection.status();
        let message = match status {
            StatusCode::BAD_REQUEST => "Malformed JSON request body".to_string(),
            StatusCode::PAYLOAD_TOO_LARGE => {
                format!("Request body exceeds {MAX_REQUEST_BODY_BYTES} bytes")
            }
            StatusCode::UNSUPPORTED_MEDIA_TYPE => {
                "Content-Type must be application/json".to_string()
            }
            StatusCode::UNPROCESSABLE_ENTITY => {
                "JSON request body does not match the generation schema".to_string()
            }
            _ => "Invalid JSON request body".to_string(),
        };
        Self {
            status,
            message,
            retry_after_seconds: None,
        }
    }

    pub(crate) fn invalid_request(message: String) -> Self {
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            message,
            retry_after_seconds: None,
        }
    }

    pub(crate) fn generation_failed_with_message(identifier: &str, detail: String) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("Failed to generate a valid {identifier}: {detail}"),
            retry_after_seconds: None,
        }
    }

    pub(crate) fn rate_limited(retry_after_seconds: u64) -> Self {
        Self {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: "LEI lookup rate limit exceeded; retry later".to_string(),
            retry_after_seconds: Some(retry_after_seconds.max(1)),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut response = (
            self.status,
            Json(ErrorResponse {
                error: self.message,
                retry_after_seconds: self.retry_after_seconds,
            }),
        )
            .into_response();
        if let Some(seconds) = self.retry_after_seconds {
            if let Ok(value) = seconds.to_string().parse() {
                response
                    .headers_mut()
                    .insert(axum::http::header::RETRY_AFTER, value);
            }
        }
        response
    }
}

/// Builds the standard LEI lookup abuse-protection response for deployment
/// adapters that enforce an additional edge-side limit.
pub fn lei_lookup_rate_limit_response(retry_after_seconds: u64) -> Response {
    ApiError::rate_limited(retry_after_seconds).into_response()
}

pub(crate) fn parse_validate_payload(
    payload: ValidatePayload,
) -> Result<ValidateRequest, ApiError> {
    payload
        .map(|Json(request)| request)
        .map_err(ApiError::invalid_json)
}

#[derive(OpenApi)]
#[openapi(
    paths(
        energy::handle_malo_generate,
        energy::handle_malo_validate,
        energy::handle_melo_generate,
        energy::handle_melo_validate,
        energy::handle_nelo_generate,
        energy::handle_nelo_validate,
        energy::handle_nebe_generate,
        energy::handle_nebe_validate,
        energy::handle_market_partner_generate,
        energy::handle_market_partner_validate,
        energy::handle_cr_generate,
        energy::handle_sg_generate,
        energy::handle_sr_generate,
        energy::handle_tr_generate,
        energy::handle_package_generate,
        registers::handle_mastr_generate,
        registers::handle_mastr_validate,
        registers::handle_eic_validate,
        registers::handle_eic_lookup,
        payments::handle_iban_generate,
        payments::handle_iban_validate,
        payments::handle_bic_generate,
        payments::handle_bic_validate,
        payments::handle_creditor_id_generate,
        payments::handle_creditor_id_validate,
        payments::handle_mandate_reference_generate,
        payments::handle_mandate_reference_validate,
        payments::handle_end_to_end_id_generate,
        payments::handle_end_to_end_id_validate,
        payments::handle_rf_reference_generate,
        payments::handle_rf_reference_validate,
        payments::handle_uetr_generate,
        payments::handle_uetr_validate,
        metering::handle_obis_validate,
        metering::handle_obis_lookup,
        metering::handle_din_43849_validate,
        business::handle_vat_id_validate,
        business::handle_lei_validate,
        business::handle_lei_lookup,
    ),
    components(schemas(
        ValidateRequest,
        ErrorResponse,
        routes::GenerateRequest,
        routes::GenerateResponse,
        routes::ValidateResponse,
        routes::Sector,
        routes::Format,
        routes::energy::MarketPartnerGenerateRequest,
        routes::registers::MastrGenerateRequest,
        routes::registers::EicLookupResponse,
        routes::payments::IbanGenerateRequest,
        routes::payments::BicGenerateRequest,
        routes::payments::RfGenerateRequest,
        routes::metering::ObisCatalogLookupEntry,
        routes::metering::ObisLookupResponse,
        routes::business::LeiLookupStatus,
        routes::business::LeiLookupCacheStatus,
        routes::business::LeiRegistryRecord,
        routes::business::LeiLookupResponse,
    )),
    tags(
        (name = "Energie", description = "Energiemarkt-Kennungen"),
        (name = "Zahlungsverkehr", description = "Konto-, Instituts- und SEPA-Kennungen"),
        (name = "Messwesen", description = "Geräte- und Messwertkennungen"),
        (name = "Unternehmen", description = "Unternehmens- und Steuerkennungen")
    ),
    info(
        title = "NRG ID Generator API",
        version = "2.0.0",
        description = "Generate and validate synthetic test identifiers for German energy-market and payment workflows. Generated values are test data, not real accounts or registrations."
    )
)]
struct ApiDoc;

pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}

async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(openapi())
}

/// Builds the platform-neutral Axum router shared by both deployment targets.
pub fn router() -> Router {
    Router::new()
        // Energie
        .route("/api/v1/malo/generate", post(energy::handle_malo_generate))
        .route("/api/v1/malo/validate", post(energy::handle_malo_validate))
        .route("/api/v1/melo/generate", post(energy::handle_melo_generate))
        .route("/api/v1/melo/validate", post(energy::handle_melo_validate))
        .route("/api/v1/nelo/generate", post(energy::handle_nelo_generate))
        .route("/api/v1/nelo/validate", post(energy::handle_nelo_validate))
        .route("/api/v1/nebe/generate", post(energy::handle_nebe_generate))
        .route("/api/v1/nebe/validate", post(energy::handle_nebe_validate))
        .route(
            "/api/v1/mp-id/generate",
            post(energy::handle_market_partner_generate),
        )
        .route(
            "/api/v1/mp-id/validate",
            post(energy::handle_market_partner_validate),
        )
        .route("/api/v1/cr-id/generate", post(energy::handle_cr_generate))
        .route("/api/v1/sg-id/generate", post(energy::handle_sg_generate))
        .route("/api/v1/sr-id/generate", post(energy::handle_sr_generate))
        .route("/api/v1/tr-id/generate", post(energy::handle_tr_generate))
        .route(
            "/api/v1/package-id/generate",
            post(energy::handle_package_generate),
        )
        .route(
            "/api/v1/mastr/generate",
            post(registers::handle_mastr_generate),
        )
        .route(
            "/api/v1/mastr/validate",
            post(registers::handle_mastr_validate),
        )
        .route("/api/v1/eic/validate", post(registers::handle_eic_validate))
        .route("/api/v1/eic/lookup", post(registers::handle_eic_lookup))
        // Zahlungsverkehr
        .route(
            "/api/v1/iban/generate",
            post(payments::handle_iban_generate),
        )
        .route(
            "/api/v1/iban/validate",
            post(payments::handle_iban_validate),
        )
        .route("/api/v1/bic/generate", post(payments::handle_bic_generate))
        .route("/api/v1/bic/validate", post(payments::handle_bic_validate))
        .route(
            "/api/v1/creditor-id/generate",
            post(payments::handle_creditor_id_generate),
        )
        .route(
            "/api/v1/creditor-id/validate",
            post(payments::handle_creditor_id_validate),
        )
        .route(
            "/api/v1/mandate-reference/generate",
            post(payments::handle_mandate_reference_generate),
        )
        .route(
            "/api/v1/mandate-reference/validate",
            post(payments::handle_mandate_reference_validate),
        )
        .route(
            "/api/v1/end-to-end-id/generate",
            post(payments::handle_end_to_end_id_generate),
        )
        .route(
            "/api/v1/end-to-end-id/validate",
            post(payments::handle_end_to_end_id_validate),
        )
        .route(
            "/api/v1/rf-reference/generate",
            post(payments::handle_rf_reference_generate),
        )
        .route(
            "/api/v1/rf-reference/validate",
            post(payments::handle_rf_reference_validate),
        )
        .route(
            "/api/v1/uetr/generate",
            post(payments::handle_uetr_generate),
        )
        .route(
            "/api/v1/uetr/validate",
            post(payments::handle_uetr_validate),
        )
        // Messwesen
        .route(
            "/api/v1/obis/validate",
            post(metering::handle_obis_validate),
        )
        .route("/api/v1/obis/lookup", post(metering::handle_obis_lookup))
        .route(
            "/api/v1/din-43849/validate",
            post(metering::handle_din_43849_validate),
        )
        // Unternehmen
        .route(
            "/api/v1/vat-id/validate",
            post(business::handle_vat_id_validate),
        )
        .route("/api/v1/lei/validate", post(business::handle_lei_validate))
        .route("/api/v1/lei/lookup", post(business::handle_lei_lookup))
        .route(OPENAPI_JSON_PATH, get(openapi_json))
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BODY_BYTES))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
                .allow_headers([CONTENT_TYPE]),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::{header, Request},
    };
    use serde_json::{json, Value};
    use tower::ServiceExt;

    const MAX_TEST_RESPONSE_BYTES: usize = 512 * 1024;

    async fn send_json(method: Method, path: &str, body: Value) -> (StatusCode, Value) {
        let response = router()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), MAX_TEST_RESPONSE_BYTES)
            .await
            .unwrap();
        (status, serde_json::from_slice(&body).unwrap())
    }

    async fn generate(path: &str, options: Value) -> Value {
        let (status, body) = send_json(Method::POST, path, options).await;
        assert_eq!(status, StatusCode::OK, "path: {path}, body: {body}");
        body
    }

    #[tokio::test]
    async fn every_generator_returns_values_accepted_by_its_core_validator() {
        use id_core::identifiers::{
            energy::{
                validate_cr_id, validate_market_partner_id, validate_nebe_id, validate_package_id,
                validate_sg_id, validate_sr_id, validate_tr_id,
            },
            payments::{
                bic::validate_bic, creditor_id::validate_german_creditor_id,
                end_to_end_id::validate_end_to_end_id,
                international_iban::validate_international_iban,
                mandate_reference::validate_mandate_reference, rf_reference::validate_rf_reference,
                uetr::validate_uetr,
            },
            registers::validate_mastr,
        };

        let cases = [
            ("/api/v1/malo/generate", "malo"),
            ("/api/v1/melo/generate", "melo"),
            ("/api/v1/nelo/generate", "nelo"),
            ("/api/v1/nebe/generate", "nebe"),
            ("/api/v1/mp-id/generate", "mp-id"),
            ("/api/v1/cr-id/generate", "cr-id"),
            ("/api/v1/sg-id/generate", "sg-id"),
            ("/api/v1/sr-id/generate", "sr-id"),
            ("/api/v1/tr-id/generate", "tr-id"),
            ("/api/v1/package-id/generate", "package-id"),
            ("/api/v1/mastr/generate", "mastr"),
            ("/api/v1/iban/generate", "iban"),
            ("/api/v1/bic/generate", "bic"),
            ("/api/v1/creditor-id/generate", "creditor-id"),
            ("/api/v1/mandate-reference/generate", "mandate-reference"),
            ("/api/v1/end-to-end-id/generate", "end-to-end-id"),
            ("/api/v1/rf-reference/generate", "rf-reference"),
            ("/api/v1/uetr/generate", "uetr"),
        ];

        for (path, kind) in cases {
            let response =
                generate(path, json!({ "count": 3, "seed": "generator-roundtrip" })).await;
            let values = response["values"].as_array().unwrap();
            assert_eq!(values.len(), 3, "path: {path}");
            for value in values {
                let value = value.as_str().unwrap();
                let accepted = match kind {
                    "malo" => id_core::validate_malo(value).is_ok(),
                    "melo" => id_core::validate_melo(value).is_ok(),
                    "nelo" => id_core::validate_nelo(value).is_ok(),
                    "nebe" => validate_nebe_id(value).is_ok(),
                    "mp-id" => validate_market_partner_id(value).is_ok(),
                    "cr-id" => validate_cr_id(value).is_ok(),
                    "sg-id" => validate_sg_id(value).is_ok(),
                    "sr-id" => validate_sr_id(value).is_ok(),
                    "tr-id" => validate_tr_id(value).is_ok(),
                    "package-id" => validate_package_id(value).is_ok(),
                    "mastr" => validate_mastr(value).is_ok(),
                    "iban" => validate_international_iban(value).is_ok(),
                    "bic" => validate_bic(value).is_ok(),
                    "creditor-id" => validate_german_creditor_id(value).is_ok(),
                    "mandate-reference" => validate_mandate_reference(value).is_ok(),
                    "end-to-end-id" => validate_end_to_end_id(value).is_ok(),
                    "rf-reference" => validate_rf_reference(value).is_ok(),
                    "uetr" => validate_uetr(value).is_ok(),
                    _ => unreachable!(),
                };
                assert!(accepted, "{kind} generator returned invalid value {value}");
            }
        }
    }

    #[tokio::test]
    async fn seeded_generation_is_deterministic_and_responses_carry_only_values() {
        let first = generate("/api/v1/malo/generate", json!({ "count": 5, "seed": "x" })).await;
        let second = generate("/api/v1/malo/generate", json!({ "count": 5, "seed": "x" })).await;
        assert_eq!(first["values"], second["values"]);
        assert_eq!(first.as_object().unwrap().keys().count(), 1);

        let random = generate("/api/v1/malo/generate", json!({})).await;
        assert_eq!(random["values"].as_array().unwrap().len(), 1);
        assert_ne!(random["values"], first["values"]);
    }

    #[tokio::test]
    async fn generation_count_is_bounded() {
        for count in [0, 101] {
            let (status, body) = send_json(
                Method::POST,
                "/api/v1/iban/generate",
                json!({ "count": count }),
            )
            .await;
            assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "count: {count}");
            assert!(body["error"].as_str().unwrap().contains("count"));
        }
    }

    #[tokio::test]
    async fn iban_options_cover_countries_and_reject_unknown_ones() {
        let german = generate("/api/v1/iban/generate", json!({ "seed": "iban-de" })).await;
        let value = german["values"][0].as_str().unwrap();
        assert!(value.starts_with("DE"));
        assert!(id_core::identifiers::payments::iban::validate_german_iban(value).is_ok());

        let french = generate(
            "/api/v1/iban/generate",
            json!({ "seed": "iban-fr", "country": "FR" }),
        )
        .await;
        assert!(french["values"][0].as_str().unwrap().starts_with("FR"));

        let (status, _) = send_json(
            Method::POST,
            "/api/v1/iban/generate",
            json!({ "country": "ZZ" }),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn formatted_output_is_available_for_iban_and_rf_reference() {
        for path in ["/api/v1/iban/generate", "/api/v1/rf-reference/generate"] {
            let electronic = generate(path, json!({ "seed": "format" })).await;
            assert!(!electronic["values"][0].as_str().unwrap().contains(' '));

            let formatted =
                generate(path, json!({ "seed": "format", "format": "formatted" })).await;
            let value = formatted["values"][0].as_str().unwrap();
            assert!(value.contains(' '), "path: {path}");
            assert_eq!(
                value.replace(' ', ""),
                electronic["values"][0].as_str().unwrap(),
                "path: {path}"
            );
        }
    }

    #[tokio::test]
    async fn bic_branch_option_switches_between_bic8_and_bic11() {
        let short = generate("/api/v1/bic/generate", json!({ "seed": "bic" })).await;
        assert_eq!(short["values"][0].as_str().unwrap().len(), 8);
        let long = generate(
            "/api/v1/bic/generate",
            json!({ "seed": "bic", "include_branch": true }),
        )
        .await;
        assert_eq!(long["values"][0].as_str().unwrap().len(), 11);
    }

    #[tokio::test]
    async fn rf_reference_accepts_an_explicit_invoice_reference_for_single_values() {
        let single = generate(
            "/api/v1/rf-reference/generate",
            json!({ "invoice_reference": "NRG202600001234" }),
        )
        .await;
        let value = single["values"][0].as_str().unwrap();
        assert!(value.starts_with("RF"));
        assert!(value.ends_with("NRG202600001234"));

        let (status, _) = send_json(
            Method::POST,
            "/api/v1/rf-reference/generate",
            json!({ "count": 2, "invoice_reference": "NRG202600001234" }),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn validation_routes_report_valid_and_invalid_ids() {
        let cases = [
            ("/api/v1/malo/validate", "41373559241", "41373559240"),
            (
                "/api/v1/melo/validate",
                "DE00056266802AO6G56M11SN51G21M24S",
                "DE00056266802ao6g56m11sn51g21m24s",
            ),
            ("/api/v1/nelo/validate", "EABC123DEF8", "EABC123DEF0"),
            (
                "/api/v1/iban/validate",
                "DE79000000001234567890",
                "DE79000000001234567891",
            ),
            (
                "/api/v1/lei/validate",
                "506700GE1G29325QX363",
                "506700GE1G29325QX364",
            ),
        ];

        for (path, valid, invalid) in cases {
            let (status, body) = send_json(Method::POST, path, json!({ "id": valid })).await;
            assert_eq!(status, StatusCode::OK, "path: {path}");
            assert_eq!(body["valid"], true, "path: {path}");
            assert_eq!(body.get("error"), None, "path: {path}");

            let (status, body) = send_json(Method::POST, path, json!({ "id": invalid })).await;
            assert_eq!(status, StatusCode::OK, "path: {path}");
            assert_eq!(body["valid"], false, "path: {path}");
            assert!(body["error"].is_string(), "path: {path}");
        }
    }

    #[tokio::test]
    async fn eic_and_obis_lookups_answer_locally() {
        let (status, body) = send_json(
            Method::POST,
            "/api/v1/obis/lookup",
            json!({ "id": "1-0:1.8.0*255" }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["found"], true);
        assert_eq!(body["entry"]["unit"], "kWh");

        let (status, _) =
            send_json(Method::POST, "/api/v1/eic/lookup", json!({ "id": "nope" })).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn json_rejections_have_precise_status_and_json_body() {
        let cases = [
            (Some("application/json"), "{", StatusCode::BAD_REQUEST),
            (
                Some("application/json"),
                "{}",
                StatusCode::UNPROCESSABLE_ENTITY,
            ),
            (
                None,
                r#"{"id":"41373559241"}"#,
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
            ),
        ];

        for (content_type, body, expected_status) in cases {
            let mut request = Request::builder()
                .method(Method::POST)
                .uri("/api/v1/malo/validate");
            if let Some(content_type) = content_type {
                request = request.header(CONTENT_TYPE, content_type);
            }
            let response = router()
                .oneshot(request.body(Body::from(body)).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), expected_status);
            assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
            let body = to_bytes(response.into_body(), 16 * 1024).await.unwrap();
            let body: ErrorResponse = serde_json::from_slice(&body).unwrap();
            assert!(!body.error.is_empty());
        }
    }

    #[tokio::test]
    async fn oversized_requests_are_rejected() {
        let (status, body) = send_json(
            Method::POST,
            "/api/v1/malo/validate",
            json!({ "id": "1".repeat(MAX_REQUEST_BODY_BYTES + 1) }),
        )
        .await;
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(
            body["error"],
            format!("Request body exceeds {MAX_REQUEST_BODY_BYTES} bytes")
        );
    }

    #[tokio::test]
    async fn non_ascii_ids_are_rejected_without_panicking() {
        for (path, id) in [
            ("/api/v1/malo/validate", "4é13735592"),
            ("/api/v1/melo/validate", "DE00056é66802AO6G56M11SN51G21M2"),
            ("/api/v1/nelo/validate", "EABC12éDEF8"),
        ] {
            let (status, body) = send_json(Method::POST, path, json!({ "id": id })).await;
            assert_eq!(status, StatusCode::OK, "path: {path}");
            assert_eq!(body["valid"], false, "path: {path}");
            assert!(body["error"].is_string(), "path: {path}");
        }
    }

    #[tokio::test]
    async fn cors_preflight_is_handled_by_the_shared_router() {
        let response = router()
            .oneshot(
                Request::builder()
                    .method(Method::OPTIONS)
                    .uri("/api/v1/malo/validate")
                    .header(header::ORIGIN, "https://example.com")
                    .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN], "*");
    }

    #[tokio::test]
    async fn lei_rate_limit_response_has_retry_contract_without_identifier_data() {
        let response = lei_lookup_rate_limit_response(60);
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(response.headers()[header::RETRY_AFTER], "60");
        let body = to_bytes(response.into_body(), MAX_TEST_RESPONSE_BYTES)
            .await
            .unwrap();
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["retry_after_seconds"], 60);
        assert_eq!(body["error"], "LEI lookup rate limit exceeded; retry later");
        assert!(!body.to_string().contains("506700GE1G29325QX363"));
    }

    #[tokio::test]
    async fn every_documented_operation_is_routed() {
        let document = serde_json::to_value(openapi()).unwrap();
        let paths = document["paths"].as_object().unwrap();
        assert_eq!(paths.len(), 39);

        for (path, operations) in paths {
            assert!(
                operations.get("post").is_some(),
                "documented operation {path} is not a POST"
            );
            // An empty JSON object either generates with defaults or is a
            // schema error; a 404/405 would mean the route is not registered.
            let (status, _) = send_json(Method::POST, path, json!({})).await;
            assert!(
                status != StatusCode::NOT_FOUND && status != StatusCode::METHOD_NOT_ALLOWED,
                "documented operation {path} is not routed (status {status})"
            );
        }
    }
}
