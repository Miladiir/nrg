use axum::{extract::rejection::JsonRejection, Json};
use id_core::identifiers::registers::{lookup_eic_directory, validate_eic};
use id_core::ops::{self, GenerateOptions};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{
    run_generate, validation_response, GenerateApiResponses, GenerateRequest, GenerateResponse,
    Sector, ValidateApiResponses, ValidateResponse,
};
use crate::{parse_validate_payload, ApiError, ErrorResponse, ValidatePayload, ValidateRequest};

/// MaStR generation optionally selects the object prefix and role suffix.
#[derive(Debug, Deserialize, ToSchema)]
pub(crate) struct MastrGenerateRequest {
    #[serde(flatten)]
    pub request: GenerateRequest,
    /// Optional sector selecting the default prefix when `prefix` is omitted.
    #[serde(default)]
    pub sector: Option<Sector>,
    /// Three-letter MaStR object prefix. If omitted, electricity defaults to
    /// `SEE` and gas to `GEE`.
    pub prefix: Option<String>,
    /// Optional two-letter role suffix. It is accepted only for prefixes whose
    /// official number concept allows that role.
    pub role_suffix: Option<String>,
}

type MastrGeneratePayload = Result<Json<MastrGenerateRequest>, JsonRejection>;

#[utoipa::path(
    post,
    path = "/api/v1/mastr/generate",
    request_body = MastrGenerateRequest,
    responses(GenerateApiResponses),
    tag = "Energie"
)]
pub(crate) async fn handle_mastr_generate(
    payload: MastrGeneratePayload,
) -> Result<Json<GenerateResponse>, ApiError> {
    let Json(request) = payload.map_err(ApiError::invalid_generate_json)?;
    let options = GenerateOptions {
        sector: request.sector.map(|sector| sector.as_str().to_string()),
        prefix: request.prefix,
        role_suffix: request.role_suffix,
        ..request.request.into_options()
    };
    run_generate("mastr", options)
}

#[utoipa::path(
    post,
    path = "/api/v1/mastr/validate",
    request_body = ValidateRequest,
    responses(ValidateApiResponses),
    tag = "Energie"
)]
pub(crate) async fn handle_mastr_validate(
    payload: ValidatePayload,
) -> Result<Json<ValidateResponse>, ApiError> {
    let request = parse_validate_payload(payload)?;
    Ok(validation_response(ops::validate("mastr", &request.id)))
}

#[utoipa::path(
    post,
    path = "/api/v1/eic/validate",
    request_body = ValidateRequest,
    responses(ValidateApiResponses),
    tag = "Energie"
)]
pub(crate) async fn handle_eic_validate(
    payload: ValidatePayload,
) -> Result<Json<ValidateResponse>, ApiError> {
    let request = parse_validate_payload(payload)?;
    Ok(validation_response(ops::validate("eic", &request.id)))
}

/// Exact-match result in the embedded ENTSO-E bulk snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub(crate) struct EicLookupResponse {
    pub value: String,
    pub found: bool,
    /// Lifecycle status from the snapshot: `active` or `inactive`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

#[allow(dead_code)]
#[derive(utoipa::IntoResponses)]
pub(crate) enum EicLookupApiResponses {
    /// Exact embedded-snapshot lookup result for a checksum-valid EIC.
    #[response(status = 200)]
    Success(EicLookupResponse),
    /// Malformed JSON request body.
    #[response(status = 400)]
    MalformedJson(ErrorResponse),
    /// Request body exceeds the configured limit.
    #[response(status = 413)]
    PayloadTooLarge(ErrorResponse),
    /// Content-Type is not application/json.
    #[response(status = 415)]
    UnsupportedMediaType(ErrorResponse),
    /// The request schema or EIC is invalid.
    #[response(status = 422)]
    InvalidRequest(ErrorResponse),
}

#[utoipa::path(
    post,
    path = "/api/v1/eic/lookup",
    request_body = ValidateRequest,
    responses(EicLookupApiResponses),
    tag = "Energie"
)]
pub(crate) async fn handle_eic_lookup(
    payload: ValidatePayload,
) -> Result<Json<EicLookupResponse>, ApiError> {
    let request = parse_validate_payload(payload)?;
    let parsed =
        validate_eic(&request.id).map_err(|error| ApiError::invalid_request(error.to_string()))?;
    let record = lookup_eic_directory(&parsed.value);
    Ok(Json(EicLookupResponse {
        value: parsed.value,
        found: record.is_some(),
        status: record.map(|record| record.status.as_str().to_string()),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use id_core::identifiers::registers::validate_mastr;

    fn generate_payload(seed: &str) -> MastrGeneratePayload {
        Ok(Json(MastrGenerateRequest {
            request: GenerateRequest {
                count: Some(2),
                seed: Some(seed.to_string()),
            },
            sector: Some(Sector::Electricity),
            prefix: Some("SNB".to_string()),
            role_suffix: Some("AN".to_string()),
        }))
    }

    #[tokio::test]
    async fn mastr_generation_is_deterministic_and_self_validating() {
        let Json(first) = handle_mastr_generate(generate_payload("mastr-api"))
            .await
            .unwrap();
        let Json(second) = handle_mastr_generate(generate_payload("mastr-api"))
            .await
            .unwrap();
        assert_eq!(first.values, second.values);
        for value in first.values {
            assert!(validate_mastr(&value).is_ok());
            assert!(value.starts_with("SNB"));
            assert!(value.ends_with("AN"));
        }
    }

    #[tokio::test]
    async fn mastr_rejects_unknown_prefix_and_disallowed_role_suffix() {
        let unknown = MastrGeneratePayload::Ok(Json(MastrGenerateRequest {
            request: GenerateRequest::default(),
            sector: None,
            prefix: Some("XXX".to_string()),
            role_suffix: None,
        }));
        assert!(handle_mastr_generate(unknown).await.is_err());

        let disallowed = MastrGeneratePayload::Ok(Json(MastrGenerateRequest {
            request: GenerateRequest::default(),
            sector: None,
            prefix: Some("SEE".to_string()),
            role_suffix: Some("AN".to_string()),
        }));
        assert!(handle_mastr_generate(disallowed).await.is_err());
    }

    #[tokio::test]
    async fn eic_lookup_rejects_invalid_codes_instead_of_reporting_a_miss() {
        let invalid = handle_eic_lookup(Ok(Json(ValidateRequest {
            id: "not-an-eic".to_string(),
        })))
        .await;
        assert!(invalid.is_err());
    }
}
