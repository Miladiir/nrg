use axum::Json;
use id_core::identifiers::metering::{lookup_curated_obis, validate_obis};
use id_core::ops;
use serde::Serialize;
use utoipa::ToSchema;

use super::{validation_response, ValidateApiResponses, ValidateResponse};
use crate::{parse_validate_payload, ApiError, ErrorResponse, ValidatePayload, ValidateRequest};

#[utoipa::path(
    post,
    path = "/api/v1/obis/validate",
    request_body = ValidateRequest,
    responses(ValidateApiResponses),
    tag = "Messwesen"
)]
pub(crate) async fn handle_obis_validate(
    payload: ValidatePayload,
) -> Result<Json<ValidateResponse>, ApiError> {
    let request = parse_validate_payload(payload)?;
    Ok(validation_response(ops::validate("obis", &request.id)))
}

/// One entry of the embedded, non-exhaustive OBIS catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub(crate) struct ObisCatalogLookupEntry {
    pub pattern: String,
    pub label_de: String,
    pub unit: String,
}

/// Lookup result in the embedded, non-exhaustive OBIS catalog. `found: false`
/// does not mean the code is invalid or unstandardised.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub(crate) struct ObisLookupResponse {
    pub value: String,
    pub found: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry: Option<ObisCatalogLookupEntry>,
}

#[allow(dead_code)]
#[derive(utoipa::IntoResponses)]
pub(crate) enum ObisLookupApiResponses {
    /// Lookup result in the embedded, explicitly non-exhaustive OBIS subset.
    #[response(status = 200)]
    Success(ObisLookupResponse),
    /// Malformed JSON request body.
    #[response(status = 400)]
    MalformedJson(ErrorResponse),
    /// Request body exceeds the configured limit.
    #[response(status = 413)]
    PayloadTooLarge(ErrorResponse),
    /// Content-Type is not application/json.
    #[response(status = 415)]
    UnsupportedMediaType(ErrorResponse),
    /// The request schema or OBIS code is invalid.
    #[response(status = 422)]
    InvalidRequest(ErrorResponse),
}

#[utoipa::path(
    post,
    path = "/api/v1/obis/lookup",
    request_body = ValidateRequest,
    responses(ObisLookupApiResponses),
    tag = "Messwesen"
)]
pub(crate) async fn handle_obis_lookup(
    payload: ValidatePayload,
) -> Result<Json<ObisLookupResponse>, ApiError> {
    let request = parse_validate_payload(payload)?;
    let code =
        validate_obis(&request.id).map_err(|error| ApiError::invalid_request(error.to_string()))?;
    let entry = lookup_curated_obis(code);
    Ok(Json(ObisLookupResponse {
        value: code.format_display(),
        found: entry.is_some(),
        entry: entry.map(|entry| ObisCatalogLookupEntry {
            pattern: entry.pattern.to_string(),
            label_de: entry.label_de.to_string(),
            unit: entry.unit.to_string(),
        }),
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/din-43849/validate",
    request_body = ValidateRequest,
    responses(ValidateApiResponses),
    tag = "Messwesen"
)]
pub(crate) async fn handle_din_43849_validate(
    payload: ValidatePayload,
) -> Result<Json<ValidateResponse>, ApiError> {
    let request = parse_validate_payload(payload)?;
    Ok(validation_response(ops::validate("din-43849", &request.id)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload(id: &str) -> ValidatePayload {
        Ok(Json(ValidateRequest { id: id.to_string() }))
    }

    #[tokio::test]
    async fn obis_validation_and_lookup_stay_separate() {
        let Json(validation) = handle_obis_validate(payload("1-0:1.8.0*255"))
            .await
            .unwrap();
        assert!(validation.valid);

        let Json(lookup) = handle_obis_lookup(payload("1-0:1.8.0*255")).await.unwrap();
        assert!(lookup.found);
        assert_eq!(lookup.entry.unwrap().unit, "kWh");

        let Json(miss) = handle_obis_lookup(payload("1-0:99.1.0")).await.unwrap();
        assert!(!miss.found);
        assert!(miss.entry.is_none());
    }

    #[tokio::test]
    async fn din_validation_accepts_formatted_input() {
        let Json(report) = handle_din_43849_validate(payload("7 QDS 01 1122 3344"))
            .await
            .unwrap();
        assert!(report.valid);
    }

    #[tokio::test]
    async fn arbitrary_unicode_is_a_normal_invalid_validation_result() {
        let Json(obis) = handle_obis_validate(payload("1-0:1.8.０")).await.unwrap();
        assert!(!obis.valid);
        let Json(device) = handle_din_43849_validate(payload("７QDS0111223344"))
            .await
            .unwrap();
        assert!(!device.valid);
    }
}
