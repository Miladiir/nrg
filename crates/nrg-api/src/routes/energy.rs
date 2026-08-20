use axum::{extract::rejection::JsonRejection, Json};
use id_core::ops::{self, GenerateOptions};
use serde::Deserialize;
use utoipa::ToSchema;

use super::{
    generate_batch, run_generate, validation_response, GenerateApiResponses, GeneratePayload,
    GenerateRequest, GenerateResponse, Sector, ValidateApiResponses, ValidateResponse,
};
use crate::{parse_validate_payload, ApiError, ValidatePayload, ValidateRequest};

#[utoipa::path(
    post,
    path = "/api/v1/malo/generate",
    request_body = GenerateRequest,
    responses(GenerateApiResponses),
    tag = "Energie"
)]
pub(crate) async fn handle_malo_generate(
    payload: GeneratePayload,
) -> Result<Json<GenerateResponse>, ApiError> {
    generate_batch("malo", payload)
}

#[utoipa::path(
    post,
    path = "/api/v1/malo/validate",
    request_body = ValidateRequest,
    responses(ValidateApiResponses),
    tag = "Energie"
)]
pub(crate) async fn handle_malo_validate(
    payload: ValidatePayload,
) -> Result<Json<ValidateResponse>, ApiError> {
    let request = parse_validate_payload(payload)?;
    Ok(validation_response(ops::validate("malo", &request.id)))
}

#[utoipa::path(
    post,
    path = "/api/v1/melo/generate",
    request_body = GenerateRequest,
    responses(GenerateApiResponses),
    tag = "Energie"
)]
pub(crate) async fn handle_melo_generate(
    payload: GeneratePayload,
) -> Result<Json<GenerateResponse>, ApiError> {
    generate_batch("melo", payload)
}

#[utoipa::path(
    post,
    path = "/api/v1/melo/validate",
    request_body = ValidateRequest,
    responses(ValidateApiResponses),
    tag = "Energie"
)]
pub(crate) async fn handle_melo_validate(
    payload: ValidatePayload,
) -> Result<Json<ValidateResponse>, ApiError> {
    let request = parse_validate_payload(payload)?;
    Ok(validation_response(ops::validate("melo", &request.id)))
}

#[utoipa::path(
    post,
    path = "/api/v1/nelo/generate",
    request_body = GenerateRequest,
    responses(GenerateApiResponses),
    tag = "Energie"
)]
pub(crate) async fn handle_nelo_generate(
    payload: GeneratePayload,
) -> Result<Json<GenerateResponse>, ApiError> {
    generate_batch("nelo", payload)
}

#[utoipa::path(
    post,
    path = "/api/v1/nelo/validate",
    request_body = ValidateRequest,
    responses(ValidateApiResponses),
    tag = "Energie"
)]
pub(crate) async fn handle_nelo_validate(
    payload: ValidatePayload,
) -> Result<Json<ValidateResponse>, ApiError> {
    let request = parse_validate_payload(payload)?;
    Ok(validation_response(ops::validate("nelo", &request.id)))
}

#[utoipa::path(
    post,
    path = "/api/v1/nebe/generate",
    request_body = GenerateRequest,
    responses(GenerateApiResponses),
    tag = "Energie"
)]
pub(crate) async fn handle_nebe_generate(
    payload: GeneratePayload,
) -> Result<Json<GenerateResponse>, ApiError> {
    generate_batch("nebe", payload)
}

#[utoipa::path(
    post,
    path = "/api/v1/nebe/validate",
    request_body = ValidateRequest,
    responses(ValidateApiResponses),
    tag = "Energie"
)]
pub(crate) async fn handle_nebe_validate(
    payload: ValidatePayload,
) -> Result<Json<ValidateResponse>, ApiError> {
    let request = parse_validate_payload(payload)?;
    Ok(validation_response(ops::validate("nebe", &request.id)))
}

/// MP-ID generation additionally selects the sector-specific formation rules.
#[derive(Debug, Deserialize, ToSchema)]
pub(crate) struct MarketPartnerGenerateRequest {
    #[serde(flatten)]
    pub request: GenerateRequest,
    /// `electricity` uses the BDEW rules, `gas` the DVGW rules. Omitted
    /// defaults to electricity.
    #[serde(default)]
    pub sector: Option<Sector>,
}

type MarketPartnerGeneratePayload = Result<Json<MarketPartnerGenerateRequest>, JsonRejection>;

#[utoipa::path(
    post,
    path = "/api/v1/mp-id/generate",
    request_body = MarketPartnerGenerateRequest,
    responses(GenerateApiResponses),
    tag = "Energie"
)]
pub(crate) async fn handle_market_partner_generate(
    payload: MarketPartnerGeneratePayload,
) -> Result<Json<GenerateResponse>, ApiError> {
    let Json(request) = payload.map_err(ApiError::invalid_generate_json)?;
    let options = GenerateOptions {
        sector: request.sector.map(|sector| sector.as_str().to_string()),
        ..request.request.into_options()
    };
    run_generate("mp-id", options)
}

#[utoipa::path(
    post,
    path = "/api/v1/mp-id/validate",
    request_body = ValidateRequest,
    responses(ValidateApiResponses),
    tag = "Energie"
)]
pub(crate) async fn handle_market_partner_validate(
    payload: ValidatePayload,
) -> Result<Json<ValidateResponse>, ApiError> {
    let request = parse_validate_payload(payload)?;
    Ok(validation_response(ops::validate("mp-id", &request.id)))
}

#[utoipa::path(
    post,
    path = "/api/v1/cr-id/generate",
    request_body = GenerateRequest,
    responses(GenerateApiResponses),
    tag = "Energie"
)]
pub(crate) async fn handle_cr_generate(
    payload: GeneratePayload,
) -> Result<Json<GenerateResponse>, ApiError> {
    generate_batch("cr-id", payload)
}

#[utoipa::path(
    post,
    path = "/api/v1/sg-id/generate",
    request_body = GenerateRequest,
    responses(GenerateApiResponses),
    tag = "Energie"
)]
pub(crate) async fn handle_sg_generate(
    payload: GeneratePayload,
) -> Result<Json<GenerateResponse>, ApiError> {
    generate_batch("sg-id", payload)
}

#[utoipa::path(
    post,
    path = "/api/v1/sr-id/generate",
    request_body = GenerateRequest,
    responses(GenerateApiResponses),
    tag = "Energie"
)]
pub(crate) async fn handle_sr_generate(
    payload: GeneratePayload,
) -> Result<Json<GenerateResponse>, ApiError> {
    generate_batch("sr-id", payload)
}

#[utoipa::path(
    post,
    path = "/api/v1/tr-id/generate",
    request_body = GenerateRequest,
    responses(GenerateApiResponses),
    tag = "Energie"
)]
pub(crate) async fn handle_tr_generate(
    payload: GeneratePayload,
) -> Result<Json<GenerateResponse>, ApiError> {
    generate_batch("tr-id", payload)
}

#[utoipa::path(
    post,
    path = "/api/v1/package-id/generate",
    request_body = GenerateRequest,
    responses(GenerateApiResponses),
    tag = "Energie"
)]
pub(crate) async fn handle_package_generate(
    payload: GeneratePayload,
) -> Result<Json<GenerateResponse>, ApiError> {
    generate_batch("package-id", payload)
}
