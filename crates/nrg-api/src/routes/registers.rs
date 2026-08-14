use axum::{extract::rejection::JsonRejection, Json};
use id_core::{
    catalog::{
        AccountExistenceStatus, AllocationStatus, CheckStatus, Checks, CollisionGuarantee,
        GenerateRequest, GeneratedIdentifier, IdentifierKind, IdentifierPart, ReferenceData,
        Sector, ValidationReport,
    },
    identifiers::registers::{
        generate_synthetic_mastr, lookup_eic_directory, validate_eic, validate_mastr,
        EicDirectoryRecord, EicError, EicObjectType, MastrError, MastrPrefix, MastrRoleSuffix,
        MastrSector, EIC_IMPLEMENTATION_GUIDE_VERSION, EIC_REFERENCE_MANUAL_VERSION,
    },
    reference_data::{
        ENTSO_E_EIC_DIRECTORY_ACTIVE_RECORD_COUNT, ENTSO_E_EIC_DIRECTORY_CREATED_AT,
        ENTSO_E_EIC_DIRECTORY_INACTIVE_RECORD_COUNT, ENTSO_E_EIC_DIRECTORY_NAME,
        ENTSO_E_EIC_DIRECTORY_PROJECTION_SHA256, ENTSO_E_EIC_DIRECTORY_RECORD_COUNT,
        ENTSO_E_EIC_DIRECTORY_SOURCE_URL, MASTR_PREFIXES_METADATA,
    },
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{
    generate_prepared_batch, prepare_generation, GenerateApiResponses, GenerateResponse,
    ValidationApiResponses,
};
use crate::{parse_validate_payload, ApiError, ErrorResponse, ValidatePayload, ValidateRequest};

const REGISTER_VALIDATION_WARNING: &str =
    "Syntax and checksum validation do not prove registry allocation or current status.";
const EIC_REGISTRY_URL: &str = "https://www.entsoe.eu/data/energy-identification-codes-eic/";
const EIC_APPROVED_CODES_URL: &str =
    "https://www.entsoe.eu/data/energy-identification-codes-eic/eic-approved-codes/";

fn mastr_reference() -> ReferenceData {
    ReferenceData {
        name: MASTR_PREFIXES_METADATA.name.to_string(),
        version: Some(format!(
            "{}; checked {}",
            MASTR_PREFIXES_METADATA.version, MASTR_PREFIXES_METADATA.checked_at
        )),
        valid_from: None,
        valid_to: None,
        sha256: Some(MASTR_PREFIXES_METADATA.sha256.to_string()),
    }
}

fn eic_reference() -> ReferenceData {
    ReferenceData {
        name: "entso_e_eic_rules".to_string(),
        version: Some(format!(
            "reference-manual {EIC_REFERENCE_MANUAL_VERSION}; implementation-guide {EIC_IMPLEMENTATION_GUIDE_VERSION}"
        )),
        valid_from: None,
        valid_to: None,
        sha256: None,
    }
}

fn eic_directory_reference() -> ReferenceData {
    ReferenceData {
        name: ENTSO_E_EIC_DIRECTORY_NAME.to_string(),
        version: Some(format!(
            "created {ENTSO_E_EIC_DIRECTORY_CREATED_AT}; {ENTSO_E_EIC_DIRECTORY_RECORD_COUNT} records ({ENTSO_E_EIC_DIRECTORY_ACTIVE_RECORD_COUNT} active, {ENTSO_E_EIC_DIRECTORY_INACTIVE_RECORD_COUNT} inactive)"
        )),
        valid_from: None,
        valid_to: None,
        sha256: Some(ENTSO_E_EIC_DIRECTORY_PROJECTION_SHA256.to_string()),
    }
}

fn invalid_report(
    kind: IdentifierKind,
    input: String,
    error: String,
    checksum: CheckStatus,
    reference_data: ReferenceData,
) -> ValidationReport {
    ValidationReport {
        kind,
        input,
        normalized: None,
        valid: false,
        checks: Checks {
            syntax: if checksum == CheckStatus::Invalid {
                CheckStatus::Valid
            } else {
                CheckStatus::Invalid
            },
            checksum,
            directory: CheckStatus::NotChecked,
            assignment: CheckStatus::Unknown,
        },
        allocation_status: AllocationStatus::Unknown,
        synthetic: None,
        production_usable: None,
        parts: Vec::new(),
        reference_data: Some(reference_data),
        warnings: Vec::new(),
        errors: vec![error],
    }
}

fn mastr_sector_name(sector: MastrSector) -> &'static str {
    match sector {
        MastrSector::Electricity => "electricity",
        MastrSector::Gas => "gas",
        MastrSector::CrossSector => "cross_sector",
    }
}

fn eic_object_type_name(object_type: EicObjectType) -> &'static str {
    match object_type {
        EicObjectType::Party => "party",
        EicObjectType::Area => "area",
        EicObjectType::MeasurementPoint => "measurement_point",
        EicObjectType::ResourceObject => "resource_object",
        EicObjectType::TieLine => "tie_line",
        EicObjectType::Location => "location",
        EicObjectType::Substation => "substation",
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub(crate) struct MastrGenerateRequest {
    #[serde(flatten)]
    pub request: GenerateRequest,
    /// Three-letter MaStR object prefix. If omitted, electricity defaults to
    /// `SEE`, gas to `GEE`, and cross-sector requests to `ABR`.
    pub prefix: Option<String>,
    /// Optional two-letter role suffix. It is accepted only for prefixes whose
    /// official number concept allows that role.
    pub role_suffix: Option<String>,
}

type MastrGeneratePayload = Result<Json<MastrGenerateRequest>, JsonRejection>;

fn default_prefix(sector: Option<Sector>) -> MastrPrefix {
    match sector {
        Some(Sector::Gas) => MastrPrefix::GasGenerationUnit,
        Some(Sector::CrossSector) => MastrPrefix::InstallationOperator,
        Some(Sector::Electricity) | None => MastrPrefix::ElectricityGenerationUnit,
    }
}

fn parse_prefix(value: Option<&str>, sector: Option<Sector>) -> Result<MastrPrefix, ApiError> {
    match value {
        None => Ok(default_prefix(sector)),
        Some(value) => {
            let value = value.trim().to_ascii_uppercase();
            MastrPrefix::from_code(&value)
                .ok_or_else(|| ApiError::invalid_request(format!("Unknown MaStR prefix {value:?}")))
        }
    }
}

fn parse_role_suffix(value: Option<&str>) -> Result<Option<MastrRoleSuffix>, ApiError> {
    value
        .map(|value| {
            let value = value.trim().to_ascii_uppercase();
            MastrRoleSuffix::from_code(&value).ok_or_else(|| {
                ApiError::invalid_request(format!("Unknown MaStR role suffix {value:?}"))
            })
        })
        .transpose()
}

fn mastr_item(
    prepared: &super::PreparedGeneration,
    index: u32,
    prefix: MastrPrefix,
    role_suffix: Option<MastrRoleSuffix>,
) -> Result<GeneratedIdentifier, String> {
    let fixture = generate_synthetic_mastr(prefix, role_suffix, &prepared.fixture_seed, index)
        .map_err(|error| error.to_string())?;
    // Keep generation and parsing as separate invariants. A change to either
    // side cannot silently emit a value rejected by the public validator.
    let parsed = validate_mastr(&fixture.identifier.value).map_err(|error| error.to_string())?;

    let mut parts = vec![
        IdentifierPart::new("prefix", parsed.prefix.code()),
        IdentifierPart::new("prefix_label", parsed.prefix.label_de()),
        IdentifierPart::new("sector", mastr_sector_name(parsed.prefix.sector())),
        IdentifierPart::new("version", parsed.version.to_string()),
        IdentifierPart::new("random_body", parsed.random_body),
        IdentifierPart::new("check_digit", parsed.check_digit.to_string()),
    ];
    if let Some(suffix) = parsed.role_suffix {
        parts.push(IdentifierPart::new("role_suffix", suffix.code()));
    }

    Ok(GeneratedIdentifier {
        value: parsed.value,
        formatted: None,
        kind: IdentifierKind::Mastr,
        profile: prepared.profile,
        synthetic: fixture.synthetic,
        production_usable: fixture.production_usable,
        checks: Checks {
            syntax: CheckStatus::Valid,
            checksum: CheckStatus::Valid,
            directory: CheckStatus::NotChecked,
            assignment: CheckStatus::Unknown,
        },
        allocation_status: AllocationStatus::Unknown,
        account_existence: AccountExistenceStatus::NotApplicable,
        collision_guarantee: CollisionGuarantee::None,
        parts,
        reference_data: Some(mastr_reference()),
        generator_version: fixture.generator_version.to_string(),
        warnings: vec![
            "Synthetic checksum-valid fixture; it can collide with a centrally allocated MaStR number."
                .to_string(),
            "It must not be used as evidence of MaStR registration.".to_string(),
        ],
    })
}

/// Catalog-driven generation entry point used by scenarios and negative
/// fixtures. Identifier-specific HTTP requests can still override the prefix
/// and role suffix through [`MastrGenerateRequest`].
pub(super) fn generate_item(
    prepared: &super::PreparedGeneration,
    index: u32,
) -> Result<GeneratedIdentifier, String> {
    if prepared.kind != IdentifierKind::Mastr {
        return Err("unsupported register identifier kind".to_string());
    }
    mastr_item(prepared, index, default_prefix(prepared.sector), None)
}

#[utoipa::path(
    post,
    path = "/api/v1/energy/registers/mastr/generate",
    operation_id = "energyMastrGenerate",
    request_body = MastrGenerateRequest,
    responses(GenerateApiResponses),
    tag = "Energie · Register & Anlagen"
)]
pub(crate) async fn handle_mastr_generate(
    payload: MastrGeneratePayload,
) -> Result<Json<GenerateResponse>, ApiError> {
    let Json(request) = payload.map_err(ApiError::invalid_generate_json)?;
    let prefix = parse_prefix(request.prefix.as_deref(), request.request.sector)?;
    let role_suffix = parse_role_suffix(request.role_suffix.as_deref())?;
    if let Some(role_suffix) = role_suffix {
        if !prefix.allowed_role_suffixes().contains(&role_suffix) {
            return Err(ApiError::invalid_request(format!(
                "MaStR role suffix {} is not allowed for prefix {}",
                role_suffix.code(),
                prefix.code()
            )));
        }
    }
    let prepared = prepare_generation(IdentifierKind::Mastr, request.request)?;
    generate_prepared_batch(prepared, |prepared, index| {
        mastr_item(prepared, index, prefix, role_suffix)
    })
}

#[utoipa::path(
    post,
    path = "/api/v1/energy/registers/mastr/validate",
    operation_id = "energyMastrValidate",
    request_body = ValidateRequest,
    responses(ValidationApiResponses),
    tag = "Energie · Register & Anlagen"
)]
pub(crate) async fn handle_mastr_validate(
    payload: ValidatePayload,
) -> Result<Json<ValidationReport>, ApiError> {
    let request = parse_validate_payload(payload)?;
    let input = request.id;
    let report = match validate_mastr(&input) {
        Ok(parsed) => {
            let mut parts = vec![
                IdentifierPart::new("prefix", parsed.prefix.code()),
                IdentifierPart::new("prefix_label", parsed.prefix.label_de()),
                IdentifierPart::new("sector", mastr_sector_name(parsed.prefix.sector())),
                IdentifierPart::new("version", parsed.version.to_string()),
                IdentifierPart::new("random_body", parsed.random_body),
                IdentifierPart::new("check_digit", parsed.check_digit.to_string()),
            ];
            if let Some(suffix) = parsed.role_suffix {
                parts.push(IdentifierPart::new("role_suffix", suffix.code()));
            }
            ValidationReport {
                kind: IdentifierKind::Mastr,
                input: input.clone(),
                normalized: Some(parsed.value),
                valid: true,
                checks: Checks {
                    syntax: CheckStatus::Valid,
                    checksum: CheckStatus::Valid,
                    directory: CheckStatus::NotChecked,
                    assignment: CheckStatus::Unknown,
                },
                allocation_status: AllocationStatus::Unknown,
                synthetic: None,
                production_usable: None,
                parts,
                reference_data: Some(mastr_reference()),
                warnings: vec![REGISTER_VALIDATION_WARNING.to_string()],
                errors: Vec::new(),
            }
        }
        Err(error) => invalid_report(
            IdentifierKind::Mastr,
            input,
            error.to_string(),
            if matches!(error, MastrError::ChecksumMismatch { .. }) {
                CheckStatus::Invalid
            } else {
                CheckStatus::NotChecked
            },
            mastr_reference(),
        ),
    };
    Ok(Json(report))
}

#[utoipa::path(
    post,
    path = "/api/v1/energy/registers/eic/validate",
    operation_id = "energyEicValidate",
    request_body = ValidateRequest,
    responses(ValidationApiResponses),
    tag = "Energie · Register & Anlagen"
)]
pub(crate) async fn handle_eic_validate(
    payload: ValidatePayload,
) -> Result<Json<ValidationReport>, ApiError> {
    let request = parse_validate_payload(payload)?;
    let input = request.id;
    let report = match validate_eic(&input) {
        Ok(parsed) => ValidationReport {
            kind: IdentifierKind::Eic,
            input: input.clone(),
            normalized: Some(parsed.value),
            valid: true,
            checks: Checks {
                syntax: CheckStatus::Valid,
                checksum: CheckStatus::Valid,
                directory: CheckStatus::NotChecked,
                assignment: CheckStatus::Unknown,
            },
            allocation_status: AllocationStatus::Unknown,
            synthetic: None,
            production_usable: None,
            parts: vec![
                IdentifierPart::new("lio_code", parsed.lio_code),
                IdentifierPart::new("object_type", eic_object_type_name(parsed.object_type)),
                IdentifierPart::new("object_type_code", parsed.object_type.code().to_string()),
                IdentifierPart::new("local_identifier", parsed.local_identifier),
                IdentifierPart::new("check_character", parsed.check_character.to_string()),
            ],
            reference_data: Some(eic_reference()),
            warnings: vec![REGISTER_VALIDATION_WARNING.to_string()],
            errors: Vec::new(),
        },
        Err(error) => invalid_report(
            IdentifierKind::Eic,
            input,
            error.to_string(),
            if matches!(error, EicError::ChecksumMismatch { .. }) {
                CheckStatus::Invalid
            } else {
                CheckStatus::NotChecked
            },
            eic_reference(),
        ),
    };
    Ok(Json(report))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EicLookupStatus {
    Found,
    NotFound,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub(crate) struct EicLookupResponse {
    pub kind: IdentifierKind,
    pub value: String,
    pub registry: String,
    pub registry_url: String,
    pub approved_codes_url: String,
    pub bulk_xml_url: String,
    pub lookup_status: EicLookupStatus,
    pub lookup_reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record: Option<EicDirectoryRecord>,
    pub checks: Checks,
    pub allocation_status: AllocationStatus,
    pub reference_data: ReferenceData,
    pub warnings: Vec<String>,
}

#[allow(dead_code)]
#[allow(clippy::large_enum_variant)]
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
    path = "/api/v1/energy/registers/eic/lookup",
    operation_id = "energyEicLookup",
    request_body = ValidateRequest,
    responses(EicLookupApiResponses),
    tag = "Energie · Register & Anlagen"
)]
pub(crate) async fn handle_eic_lookup(
    payload: ValidatePayload,
) -> Result<Json<EicLookupResponse>, ApiError> {
    let request = parse_validate_payload(payload)?;
    let parsed =
        validate_eic(&request.id).map_err(|error| ApiError::invalid_request(error.to_string()))?;

    let record = lookup_eic_directory(&parsed.value);
    let (lookup_status, lookup_reason, directory_check, warnings) = match record.as_ref() {
        Some(record) => (
            EicLookupStatus::Found,
            format!(
                "Exact code occurs in the embedded ENTSO-E bulk snapshot created {ENTSO_E_EIC_DIRECTORY_CREATED_AT} with lifecycle status {}.",
                record.status.as_str()
            ),
            CheckStatus::Found,
            vec![
                "A snapshot hit proves only that this exact code occurs in the embedded ENTSO-E bulk export; it is not a live status or entity-identity check."
                    .to_string(),
                "Allocation status remains unknown outside the timestamped snapshot context."
                    .to_string(),
            ],
        ),
        None => (
            EicLookupStatus::NotFound,
            format!(
                "No exact record occurs in the embedded ENTSO-E bulk snapshot created {ENTSO_E_EIC_DIRECTORY_CREATED_AT}."
            ),
            CheckStatus::NotFound,
            vec![
                "Snapshot absence does not prove that an EIC is unallocated: local LIO registries and changes after the snapshot may differ."
                    .to_string(),
            ],
        ),
    };

    Ok(Json(EicLookupResponse {
        kind: IdentifierKind::Eic,
        value: parsed.value,
        registry: "ENTSO-E EIC registry".to_string(),
        registry_url: EIC_REGISTRY_URL.to_string(),
        approved_codes_url: EIC_APPROVED_CODES_URL.to_string(),
        bulk_xml_url: ENTSO_E_EIC_DIRECTORY_SOURCE_URL.to_string(),
        lookup_status,
        lookup_reason,
        record,
        checks: Checks {
            syntax: CheckStatus::Valid,
            checksum: CheckStatus::Valid,
            directory: directory_check,
            assignment: CheckStatus::Unknown,
        },
        allocation_status: AllocationStatus::Unknown,
        reference_data: eic_directory_reference(),
        warnings,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use id_core::catalog::{GenerationProfile, IdentifierFormat};

    fn validate_payload(id: &str) -> ValidatePayload {
        Ok(Json(ValidateRequest { id: id.to_string() }))
    }

    fn generate_payload(seed: &str) -> MastrGeneratePayload {
        Ok(Json(MastrGenerateRequest {
            request: GenerateRequest {
                profile: Some(GenerationProfile::ChecksumOnly),
                count: 2,
                fixture_seed: Some(seed.to_string()),
                format: IdentifierFormat::Electronic,
                sector: Some(Sector::Electricity),
                country: None,
            },
            prefix: Some("SNB".to_string()),
            role_suffix: Some("AN".to_string()),
        }))
    }

    #[tokio::test]
    async fn mastr_generation_is_deterministic_self_validating_and_non_production() {
        let Json(first) = handle_mastr_generate(generate_payload("mastr-api"))
            .await
            .unwrap();
        let Json(second) = handle_mastr_generate(generate_payload("mastr-api"))
            .await
            .unwrap();
        assert_eq!(first.items, second.items);
        for item in first.items {
            assert!(validate_mastr(&item.value).is_ok());
            assert!(item.synthetic);
            assert!(!item.production_usable);
            assert_eq!(item.allocation_status, AllocationStatus::Unknown);
            assert_eq!(item.checks.assignment, CheckStatus::Unknown);
        }
    }

    #[tokio::test]
    async fn mastr_validator_detects_checksum_changes() {
        let Json(report) = handle_mastr_validate(validate_payload("SNB901234567899AN"))
            .await
            .unwrap();
        assert!(!report.valid);
        assert_eq!(report.checks.checksum, CheckStatus::Invalid);
    }

    #[tokio::test]
    async fn eic_lookup_reports_snapshot_hits_and_misses_without_allocation_claims() {
        let Json(validation) = handle_eic_validate(validate_payload("10X---ENTSOE---L"))
            .await
            .unwrap();
        assert!(validation.valid);

        let Json(lookup) = handle_eic_lookup(validate_payload("10X---ENTSOE---L"))
            .await
            .unwrap();
        assert_eq!(lookup.lookup_status, EicLookupStatus::NotFound);
        assert_eq!(lookup.checks.directory, CheckStatus::NotFound);
        assert!(lookup.record.is_none());
        assert_eq!(lookup.allocation_status, AllocationStatus::Unknown);

        let Json(found) = handle_eic_lookup(validate_payload("10X1001A1001A450"))
            .await
            .unwrap();
        assert_eq!(found.lookup_status, EicLookupStatus::Found);
        assert_eq!(found.checks.directory, CheckStatus::Found);
        assert_eq!(
            found.record.as_ref().map(|record| record.value.as_str()),
            Some("10X1001A1001A450")
        );
        assert_eq!(
            serde_json::to_value(&found.record).unwrap(),
            serde_json::json!({
                "value": "10X1001A1001A450",
                "status": "active"
            })
        );
        assert_eq!(found.allocation_status, AllocationStatus::Unknown);
    }
}
