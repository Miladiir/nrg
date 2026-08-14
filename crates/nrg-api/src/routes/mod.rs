use axum::{extract::rejection::JsonRejection, Json};
use id_core::{
    catalog::{
        descriptor, CollisionGuarantee, GenerateRequest, GeneratedIdentifier, GenerationProfile,
        IdentifierFormat, IdentifierKind, Sector, ValidationReport,
    },
    generate_melo, GENERATOR_VERSION,
};
use serde::Serialize;
use utoipa::ToSchema;

use crate::{ApiError, ErrorResponse};

pub(crate) mod business;
pub(crate) mod energy;
pub(crate) mod metering;
pub(crate) mod negative;
pub(crate) mod payments;
pub(crate) mod registers;
pub(crate) mod scenarios;

pub(crate) type GeneratePayload = Result<Json<GenerateRequest>, JsonRejection>;

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct GenerateResponse {
    /// Version of the deterministic algorithms used for this batch.
    pub generator_version: String,
    /// Effective seed. Persist it together with the generator version and, for
    /// directory-backed profiles, the returned reference-data version/hash.
    pub fixture_seed: String,
    pub items: Vec<GeneratedIdentifier>,
}

#[allow(dead_code)]
#[derive(utoipa::IntoResponses)]
pub(crate) enum GenerateApiResponses {
    /// Generated identifier batch.
    #[response(status = 200)]
    Success(GenerateResponse),
    /// Malformed JSON request body.
    #[response(status = 400)]
    MalformedJson(ErrorResponse),
    /// Request body exceeds the configured limit.
    #[response(status = 413)]
    PayloadTooLarge(ErrorResponse),
    /// Content-Type is not application/json.
    #[response(status = 415)]
    UnsupportedMediaType(ErrorResponse),
    /// JSON schema or generation options are invalid.
    #[response(status = 422)]
    InvalidOptions(ErrorResponse),
    /// A generated value failed an internal invariant.
    #[response(status = 500)]
    InternalInvariant(ErrorResponse),
}

// Documentation-only enum; it is never instantiated at runtime.
#[allow(dead_code, clippy::large_enum_variant)]
#[derive(utoipa::IntoResponses)]
pub(crate) enum ValidationApiResponses {
    /// Detailed validation report.
    #[response(status = 200)]
    Success(ValidationReport),
    /// Malformed JSON request body.
    #[response(status = 400)]
    MalformedJson(ErrorResponse),
    /// Request body exceeds the configured limit.
    #[response(status = 413)]
    PayloadTooLarge(ErrorResponse),
    /// Content-Type is not application/json.
    #[response(status = 415)]
    UnsupportedMediaType(ErrorResponse),
    /// JSON request body does not match the validation schema.
    #[response(status = 422)]
    InvalidRequest(ErrorResponse),
}

pub(crate) struct PreparedGeneration {
    pub kind: IdentifierKind,
    pub profile: GenerationProfile,
    pub count: u8,
    pub fixture_seed: String,
    pub format: IdentifierFormat,
    pub sector: Option<Sector>,
    pub country: Option<String>,
}

pub(crate) fn parse_generate_payload(
    payload: GeneratePayload,
) -> Result<GenerateRequest, ApiError> {
    payload
        .map(|Json(request)| request)
        .map_err(ApiError::invalid_generate_json)
}

pub(crate) fn prepare_generation(
    kind: IdentifierKind,
    mut request: GenerateRequest,
) -> Result<PreparedGeneration, ApiError> {
    let descriptor = descriptor(kind)
        .ok_or_else(|| ApiError::invalid_request("Unknown identifier kind".to_string()))?;
    let count = request
        .validated_count()
        .map_err(|error| ApiError::invalid_request(error.to_string()))?;
    if kind == IdentifierKind::Iban {
        let country = request
            .country
            .as_deref()
            .unwrap_or("DE")
            .trim()
            .to_ascii_uppercase();
        id_core::identifiers::payments::international_iban::iban_country_spec(&country)
            .map_err(|error| ApiError::invalid_request(error.to_string()))?;
        request.country = Some(country);
    }
    let international_iban_default = (kind == IdentifierKind::Iban
        && request
            .country
            .as_deref()
            .is_some_and(|country| !country.eq_ignore_ascii_case("DE")))
    .then_some(GenerationProfile::ChecksumOnly);
    let profile = request
        .profile
        .or(international_iban_default)
        .or(descriptor.default_profile)
        .ok_or_else(|| {
            ApiError::invalid_request(format!(
                "{} has no default generation profile",
                descriptor.slug
            ))
        })?;
    if !descriptor.supports_profile(profile) {
        return Err(ApiError::invalid_request(format!(
            "Profile '{}' is not supported for {}",
            profile.as_str(),
            descriptor.slug
        )));
    }
    if kind == IdentifierKind::Iban
        && request.country.as_deref() != Some("DE")
        && !matches!(
            profile,
            GenerationProfile::ChecksumOnly | GenerationProfile::OfficialExample
        )
    {
        return Err(ApiError::invalid_request(format!(
            "Profile '{}' is available only for German IBANs; international generation supports 'checksum_only' and 'official_example'",
            profile.as_str()
        )));
    }
    if kind == IdentifierKind::MarketPartnerId && request.sector == Some(Sector::CrossSector) {
        return Err(ApiError::invalid_request(
            "MP-ID generation requires sector 'electricity' or 'gas'".to_string(),
        ));
    }

    let fixture_seed = request.fixture_seed.unwrap_or_else(generate_melo);
    Ok(PreparedGeneration {
        kind,
        profile,
        count,
        fixture_seed,
        format: request.format,
        sector: request.sector,
        country: request.country,
    })
}

pub(crate) fn generate_batch<F>(
    kind: IdentifierKind,
    payload: GeneratePayload,
    generator: F,
) -> Result<Json<GenerateResponse>, ApiError>
where
    F: FnMut(&PreparedGeneration, u32) -> Result<GeneratedIdentifier, String>,
{
    let request = parse_generate_payload(payload)?;
    let prepared = prepare_generation(kind, request)?;
    generate_prepared_batch(prepared, generator)
}

pub(crate) fn generate_prepared_batch<F>(
    prepared: PreparedGeneration,
    mut generator: F,
) -> Result<Json<GenerateResponse>, ApiError>
where
    F: FnMut(&PreparedGeneration, u32) -> Result<GeneratedIdentifier, String>,
{
    let mut items = Vec::with_capacity(usize::from(prepared.count));
    for index in 0..u32::from(prepared.count) {
        let item = generator(&prepared, index).map_err(|message| {
            ApiError::generation_failed_with_message(prepared.kind.as_str(), message)
        })?;
        if item.collision_guarantee == CollisionGuarantee::WithinBatch
            && items
                .iter()
                .any(|existing: &GeneratedIdentifier| existing.value == item.value)
        {
            return Err(ApiError::generation_failed_with_message(
                prepared.kind.as_str(),
                "generator violated its within-batch uniqueness guarantee".to_string(),
            ));
        }
        items.push(item);
    }

    Ok(Json(GenerateResponse {
        generator_version: GENERATOR_VERSION.to_string(),
        fixture_seed: prepared.fixture_seed,
        items,
    }))
}

pub(crate) fn rendered_value(
    electronic: &str,
    formatted: Option<&str>,
    format: IdentifierFormat,
) -> String {
    match (format, formatted) {
        (IdentifierFormat::Formatted, Some(formatted)) => formatted.to_string(),
        _ => electronic.to_string(),
    }
}

pub(crate) fn generate_identifier_item(
    prepared: &PreparedGeneration,
    index: u32,
) -> Result<GeneratedIdentifier, String> {
    match prepared.kind {
        IdentifierKind::Malo
        | IdentifierKind::Melo
        | IdentifierKind::Nelo
        | IdentifierKind::Nebe
        | IdentifierKind::MarketPartnerId
        | IdentifierKind::ClusterResourceId
        | IdentifierKind::SteeringGroupId
        | IdentifierKind::ControllableResourceId
        | IdentifierKind::TechnicalResourceId
        | IdentifierKind::PackageId => energy::generate_item(prepared, index),
        IdentifierKind::Iban
        | IdentifierKind::Bic
        | IdentifierKind::CreditorId
        | IdentifierKind::MandateReference
        | IdentifierKind::EndToEndId
        | IdentifierKind::RfReference
        | IdentifierKind::Uetr => payments::generate_item(prepared, index),
        IdentifierKind::Mastr => registers::generate_item(prepared, index),
        IdentifierKind::VatId
        | IdentifierKind::Lei
        | IdentifierKind::Eic
        | IdentifierKind::Obis
        | IdentifierKind::Din43849 => Err(format!(
            "{} does not support generation",
            prepared.kind.as_str()
        )),
    }
}
