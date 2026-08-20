use axum::{extract::rejection::JsonRejection, Json};
use id_core::ops::{self, GenerateOptions};
use serde::Deserialize;
use utoipa::ToSchema;

use super::{
    generate_batch, run_generate, validation_response, Format, GenerateApiResponses,
    GeneratePayload, GenerateRequest, GenerateResponse, ValidateApiResponses, ValidateResponse,
};
use crate::{parse_validate_payload, ApiError, ValidatePayload, ValidateRequest};

/// IBAN generation additionally accepts the country and output format.
#[derive(Debug, Deserialize, ToSchema)]
pub(crate) struct IbanGenerateRequest {
    #[serde(flatten)]
    pub request: GenerateRequest,
    /// `formatted` groups the value in blocks of four characters.
    #[serde(default)]
    pub format: Option<Format>,
    /// ISO 3166 alpha-2 IBAN country. Omitted defaults to `DE`. German IBANs
    /// use a bank code absent from the embedded Bundesbank directory; other
    /// countries get country-format and MOD-97-valid values.
    #[serde(default)]
    pub country: Option<String>,
}

type IbanGeneratePayload = Result<Json<IbanGenerateRequest>, JsonRejection>;

#[utoipa::path(
    post,
    path = "/api/v1/iban/generate",
    request_body = IbanGenerateRequest,
    responses(GenerateApiResponses),
    tag = "Zahlungsverkehr"
)]
pub(crate) async fn handle_iban_generate(
    payload: IbanGeneratePayload,
) -> Result<Json<GenerateResponse>, ApiError> {
    let Json(request) = payload.map_err(ApiError::invalid_generate_json)?;
    let options = GenerateOptions {
        format: request.format.map(|format| format.as_str().to_string()),
        country: request.country,
        ..request.request.into_options()
    };
    run_generate("iban", options)
}

#[utoipa::path(
    post,
    path = "/api/v1/iban/validate",
    request_body = ValidateRequest,
    responses(ValidateApiResponses),
    tag = "Zahlungsverkehr"
)]
pub(crate) async fn handle_iban_validate(
    payload: ValidatePayload,
) -> Result<Json<ValidateResponse>, ApiError> {
    let request = parse_validate_payload(payload)?;
    Ok(validation_response(ops::validate("iban", &request.id)))
}

/// BIC generation additionally selects the 8- or 11-character form.
#[derive(Debug, Deserialize, ToSchema)]
pub(crate) struct BicGenerateRequest {
    #[serde(flatten)]
    pub request: GenerateRequest,
    /// Generate an 11-character BIC with branch identifier instead of BIC8.
    #[serde(default)]
    pub include_branch: bool,
}

type BicGeneratePayload = Result<Json<BicGenerateRequest>, JsonRejection>;

#[utoipa::path(
    post,
    path = "/api/v1/bic/generate",
    request_body = BicGenerateRequest,
    responses(GenerateApiResponses),
    tag = "Zahlungsverkehr"
)]
pub(crate) async fn handle_bic_generate(
    payload: BicGeneratePayload,
) -> Result<Json<GenerateResponse>, ApiError> {
    let Json(request) = payload.map_err(ApiError::invalid_generate_json)?;
    let options = GenerateOptions {
        include_branch: Some(request.include_branch),
        ..request.request.into_options()
    };
    run_generate("bic", options)
}

#[utoipa::path(
    post,
    path = "/api/v1/bic/validate",
    request_body = ValidateRequest,
    responses(ValidateApiResponses),
    tag = "Zahlungsverkehr"
)]
pub(crate) async fn handle_bic_validate(
    payload: ValidatePayload,
) -> Result<Json<ValidateResponse>, ApiError> {
    let request = parse_validate_payload(payload)?;
    Ok(validation_response(ops::validate("bic", &request.id)))
}

#[utoipa::path(
    post,
    path = "/api/v1/creditor-id/generate",
    request_body = GenerateRequest,
    responses(GenerateApiResponses),
    tag = "Zahlungsverkehr"
)]
pub(crate) async fn handle_creditor_id_generate(
    payload: GeneratePayload,
) -> Result<Json<GenerateResponse>, ApiError> {
    generate_batch("creditor-id", payload)
}

#[utoipa::path(
    post,
    path = "/api/v1/creditor-id/validate",
    request_body = ValidateRequest,
    responses(ValidateApiResponses),
    tag = "Zahlungsverkehr"
)]
pub(crate) async fn handle_creditor_id_validate(
    payload: ValidatePayload,
) -> Result<Json<ValidateResponse>, ApiError> {
    let request = parse_validate_payload(payload)?;
    Ok(validation_response(ops::validate(
        "creditor-id",
        &request.id,
    )))
}

#[utoipa::path(
    post,
    path = "/api/v1/mandate-reference/generate",
    request_body = GenerateRequest,
    responses(GenerateApiResponses),
    tag = "Zahlungsverkehr"
)]
pub(crate) async fn handle_mandate_reference_generate(
    payload: GeneratePayload,
) -> Result<Json<GenerateResponse>, ApiError> {
    generate_batch("mandate-reference", payload)
}

#[utoipa::path(
    post,
    path = "/api/v1/mandate-reference/validate",
    request_body = ValidateRequest,
    responses(ValidateApiResponses),
    tag = "Zahlungsverkehr"
)]
pub(crate) async fn handle_mandate_reference_validate(
    payload: ValidatePayload,
) -> Result<Json<ValidateResponse>, ApiError> {
    let request = parse_validate_payload(payload)?;
    Ok(validation_response(ops::validate(
        "mandate-reference",
        &request.id,
    )))
}

#[utoipa::path(
    post,
    path = "/api/v1/end-to-end-id/generate",
    request_body = GenerateRequest,
    responses(GenerateApiResponses),
    tag = "Zahlungsverkehr"
)]
pub(crate) async fn handle_end_to_end_id_generate(
    payload: GeneratePayload,
) -> Result<Json<GenerateResponse>, ApiError> {
    generate_batch("end-to-end-id", payload)
}

#[utoipa::path(
    post,
    path = "/api/v1/end-to-end-id/validate",
    request_body = ValidateRequest,
    responses(ValidateApiResponses),
    tag = "Zahlungsverkehr"
)]
pub(crate) async fn handle_end_to_end_id_validate(
    payload: ValidatePayload,
) -> Result<Json<ValidateResponse>, ApiError> {
    let request = parse_validate_payload(payload)?;
    Ok(validation_response(ops::validate(
        "end-to-end-id",
        &request.id,
    )))
}

/// RF creditor references accept an output format and optionally wrap an
/// explicit reference body.
#[derive(Debug, Deserialize, ToSchema)]
pub(crate) struct RfGenerateRequest {
    #[serde(flatten)]
    pub request: GenerateRequest,
    /// `formatted` groups the value in blocks of four characters.
    #[serde(default)]
    pub format: Option<Format>,
    /// Optional reference body, for example `NRG202600001234`. Explicit bodies
    /// are accepted only for a single-value request.
    pub invoice_reference: Option<String>,
}

type RfGeneratePayload = Result<Json<RfGenerateRequest>, JsonRejection>;

#[utoipa::path(
    post,
    path = "/api/v1/rf-reference/generate",
    request_body = RfGenerateRequest,
    responses(GenerateApiResponses),
    tag = "Zahlungsverkehr"
)]
pub(crate) async fn handle_rf_reference_generate(
    payload: RfGeneratePayload,
) -> Result<Json<GenerateResponse>, ApiError> {
    let Json(request) = payload.map_err(ApiError::invalid_generate_json)?;
    let options = GenerateOptions {
        format: request.format.map(|format| format.as_str().to_string()),
        invoice_reference: request.invoice_reference,
        ..request.request.into_options()
    };
    run_generate("rf-reference", options)
}

#[utoipa::path(
    post,
    path = "/api/v1/rf-reference/validate",
    request_body = ValidateRequest,
    responses(ValidateApiResponses),
    tag = "Zahlungsverkehr"
)]
pub(crate) async fn handle_rf_reference_validate(
    payload: ValidatePayload,
) -> Result<Json<ValidateResponse>, ApiError> {
    let request = parse_validate_payload(payload)?;
    Ok(validation_response(ops::validate(
        "rf-reference",
        &request.id,
    )))
}

#[utoipa::path(
    post,
    path = "/api/v1/uetr/generate",
    request_body = GenerateRequest,
    responses(GenerateApiResponses),
    tag = "Zahlungsverkehr"
)]
pub(crate) async fn handle_uetr_generate(
    payload: GeneratePayload,
) -> Result<Json<GenerateResponse>, ApiError> {
    generate_batch("uetr", payload)
}

#[utoipa::path(
    post,
    path = "/api/v1/uetr/validate",
    request_body = ValidateRequest,
    responses(ValidateApiResponses),
    tag = "Zahlungsverkehr"
)]
pub(crate) async fn handle_uetr_validate(
    payload: ValidatePayload,
) -> Result<Json<ValidateResponse>, ApiError> {
    let request = parse_validate_payload(payload)?;
    Ok(validation_response(ops::validate("uetr", &request.id)))
}
