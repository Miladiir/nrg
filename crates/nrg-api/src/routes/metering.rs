use axum::Json;
use id_core::{
    catalog::{
        AllocationStatus, CheckStatus, Checks, IdentifierKind, IdentifierPart, ReferenceData,
        ValidationReport,
    },
    identifiers::metering::{
        lookup_curated_obis, validate_din_43849, validate_obis, Din43849Category, ObisMedia,
        DIN_43849_EDITION, DIN_43849_PUBLIC_STRUCTURE_SOURCE, OBIS_MARKET_CATALOG_SCOPE,
        OBIS_MARKET_CATALOG_VERSION, OBIS_STRUCTURE_VERSION,
    },
};
use serde::Serialize;
use utoipa::ToSchema;

use super::ValidationApiResponses;
use crate::{parse_validate_payload, ApiError, ErrorResponse, ValidatePayload, ValidateRequest};

fn obis_structure_reference() -> ReferenceData {
    ReferenceData {
        name: "dlms_obis_structure".to_string(),
        version: Some(OBIS_STRUCTURE_VERSION.to_string()),
        valid_from: None,
        valid_to: None,
        sha256: None,
    }
}

fn obis_catalog_reference() -> ReferenceData {
    ReferenceData {
        name: "bnetza_edi_energy_obis_curated_subset".to_string(),
        version: Some(OBIS_MARKET_CATALOG_VERSION.to_string()),
        valid_from: None,
        valid_to: None,
        sha256: None,
    }
}

fn din_reference() -> ReferenceData {
    ReferenceData {
        name: "din_43849_public_structure".to_string(),
        version: Some(format!(
            "{DIN_43849_EDITION}; {DIN_43849_PUBLIC_STRUCTURE_SOURCE}"
        )),
        valid_from: None,
        valid_to: None,
        sha256: None,
    }
}

fn invalid_report(
    kind: IdentifierKind,
    input: String,
    error: String,
    allocation_status: AllocationStatus,
    reference_data: ReferenceData,
) -> ValidationReport {
    ValidationReport {
        kind,
        input,
        normalized: None,
        valid: false,
        checks: Checks {
            syntax: CheckStatus::Invalid,
            checksum: CheckStatus::NotApplicable,
            directory: CheckStatus::NotChecked,
            assignment: if allocation_status == AllocationStatus::NotApplicable {
                CheckStatus::NotApplicable
            } else {
                CheckStatus::Unknown
            },
        },
        allocation_status,
        synthetic: None,
        production_usable: None,
        parts: Vec::new(),
        reference_data: Some(reference_data),
        warnings: Vec::new(),
        errors: vec![error],
    }
}

fn obis_media_name(media: ObisMedia) -> String {
    match media {
        ObisMedia::Abstract => "abstract".to_string(),
        ObisMedia::AcElectricity => "ac_electricity".to_string(),
        ObisMedia::DcElectricity => "dc_electricity".to_string(),
        ObisMedia::Reserved(value) => format!("reserved_{value}"),
        ObisMedia::HeatCostAllocator => "heat_cost_allocator".to_string(),
        ObisMedia::ThermalEnergy(value) => format!("thermal_energy_{value}"),
        ObisMedia::Gas => "gas".to_string(),
        ObisMedia::ColdWater => "cold_water".to_string(),
        ObisMedia::HotWater => "hot_water".to_string(),
        ObisMedia::OtherMedia => "other_media".to_string(),
    }
}

fn din_category_name(category: Din43849Category) -> String {
    match category {
        Din43849Category::AcElectricityMeter => "ac_electricity_meter".to_string(),
        Din43849Category::DcElectricityMeter => "dc_electricity_meter".to_string(),
        Din43849Category::HeatCostAllocator => "heat_cost_allocator".to_string(),
        Din43849Category::CoolingMeter => "cooling_meter".to_string(),
        Din43849Category::HeatMeter => "heat_meter".to_string(),
        Din43849Category::GasMeter => "gas_meter".to_string(),
        Din43849Category::ColdWaterMeter => "cold_water_meter".to_string(),
        Din43849Category::WarmWaterMeter => "warm_water_meter".to_string(),
        Din43849Category::BusOrSystemDevice => "bus_or_system_device".to_string(),
        Din43849Category::OtherMedia => "other_media".to_string(),
        Din43849Category::ControlOrSwitchingDevice => "control_or_switching_device".to_string(),
        Din43849Category::Unclassified(value) => format!("unclassified_{value}"),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/metering/values/obis/validate",
    operation_id = "meteringObisValidate",
    request_body = ValidateRequest,
    responses(ValidationApiResponses),
    tag = "Messwesen · Geräte & Werte"
)]
pub(crate) async fn handle_obis_validate(
    payload: ValidatePayload,
) -> Result<Json<ValidationReport>, ApiError> {
    let request = parse_validate_payload(payload)?;
    let input = request.id;
    let report = match validate_obis(&input) {
        Ok(code) => {
            let mut parts = vec![
                IdentifierPart::new("group_c", code.c().to_string()),
                IdentifierPart::new("group_d", code.d().to_string()),
                IdentifierPart::new("group_e", code.e().to_string()),
                IdentifierPart::new(
                    "manufacturer_specific",
                    code.is_manufacturer_specific().to_string(),
                ),
            ];
            if let Some(value) = code.a() {
                parts.push(IdentifierPart::new("group_a", value.to_string()));
            }
            if let Some(value) = code.b() {
                parts.push(IdentifierPart::new("group_b", value.to_string()));
            }
            if let Some(value) = code.f() {
                parts.push(IdentifierPart::new("group_f", value.to_string()));
            }
            if let Some(media) = code.media() {
                parts.push(IdentifierPart::new("media", obis_media_name(media)));
            }
            if let Some(logical_name) = code.format_logical_name() {
                parts.push(IdentifierPart::new("logical_name", logical_name));
            }
            ValidationReport {
                kind: IdentifierKind::Obis,
                input: input.clone(),
                normalized: Some(code.format_display()),
                valid: true,
                checks: Checks {
                    syntax: CheckStatus::Valid,
                    checksum: CheckStatus::NotApplicable,
                    // Catalog membership is deliberately handled by the
                    // separate lookup operation.
                    directory: CheckStatus::NotChecked,
                    assignment: CheckStatus::NotApplicable,
                },
                allocation_status: AllocationStatus::NotApplicable,
                synthetic: None,
                production_usable: None,
                parts,
                reference_data: Some(obis_structure_reference()),
                warnings: vec![
                    "Structural validity does not establish membership in a complete OBIS catalog."
                        .to_string(),
                ],
                errors: Vec::new(),
            }
        }
        Err(error) => invalid_report(
            IdentifierKind::Obis,
            input,
            error.to_string(),
            AllocationStatus::NotApplicable,
            obis_structure_reference(),
        ),
    };
    Ok(Json(report))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ObisLookupStatus {
    Found,
    NotFound,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub(crate) struct ObisCatalogLookupEntry {
    pub pattern: String,
    pub label_de: String,
    pub unit: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub(crate) struct ObisLookupResponse {
    pub kind: IdentifierKind,
    pub value: String,
    pub lookup_status: ObisLookupStatus,
    pub checks: Checks,
    pub allocation_status: AllocationStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry: Option<ObisCatalogLookupEntry>,
    pub reference_data: ReferenceData,
    pub catalog_scope: String,
    pub warnings: Vec<String>,
}

#[allow(dead_code)]
#[allow(clippy::large_enum_variant)]
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
    path = "/api/v1/metering/values/obis/lookup",
    operation_id = "meteringObisLookup",
    request_body = ValidateRequest,
    responses(ObisLookupApiResponses),
    tag = "Messwesen · Geräte & Werte"
)]
pub(crate) async fn handle_obis_lookup(
    payload: ValidatePayload,
) -> Result<Json<ObisLookupResponse>, ApiError> {
    let request = parse_validate_payload(payload)?;
    let code =
        validate_obis(&request.id).map_err(|error| ApiError::invalid_request(error.to_string()))?;
    let entry = lookup_curated_obis(code).copied();
    let (lookup_status, directory) = if entry.is_some() {
        (ObisLookupStatus::Found, CheckStatus::Found)
    } else {
        (ObisLookupStatus::NotFound, CheckStatus::NotFound)
    };

    Ok(Json(ObisLookupResponse {
        kind: IdentifierKind::Obis,
        value: code.format_display(),
        lookup_status,
        checks: Checks {
            syntax: CheckStatus::Valid,
            checksum: CheckStatus::NotApplicable,
            directory,
            assignment: CheckStatus::NotApplicable,
        },
        allocation_status: AllocationStatus::NotApplicable,
        entry: entry.map(|entry| ObisCatalogLookupEntry {
            pattern: entry.pattern.to_string(),
            label_de: entry.label_de.to_string(),
            unit: entry.unit.to_string(),
        }),
        reference_data: obis_catalog_reference(),
        catalog_scope: OBIS_MARKET_CATALOG_SCOPE.to_string(),
        warnings: vec![
            "The embedded catalog is non-exhaustive; not_found does not mean the OBIS code is invalid or unstandardised."
                .to_string(),
        ],
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/metering/devices/din-43849/validate",
    operation_id = "meteringDin43849Validate",
    request_body = ValidateRequest,
    responses(ValidationApiResponses),
    tag = "Messwesen · Geräte & Werte"
)]
pub(crate) async fn handle_din_43849_validate(
    payload: ValidatePayload,
) -> Result<Json<ValidationReport>, ApiError> {
    let request = parse_validate_payload(payload)?;
    let input = request.id;
    let report = match validate_din_43849(&input) {
        Ok(identifier) => ValidationReport {
            kind: IdentifierKind::Din43849,
            input: input.clone(),
            normalized: Some(identifier.electronic),
            valid: true,
            checks: Checks {
                syntax: CheckStatus::Valid,
                checksum: CheckStatus::NotApplicable,
                directory: CheckStatus::NotChecked,
                assignment: CheckStatus::Unknown,
            },
            allocation_status: AllocationStatus::Unknown,
            synthetic: None,
            production_usable: None,
            parts: vec![
                IdentifierPart::new("formatted", identifier.formatted),
                IdentifierPart::new("category", din_category_name(identifier.category)),
                IdentifierPart::new(
                    "category_character",
                    identifier.category_character.to_string(),
                ),
                IdentifierPart::new("manufacturer_id", identifier.manufacturer_id),
                IdentifierPart::new("fabrication_block", identifier.fabrication_block),
                IdentifierPart::new("fabrication_number", identifier.fabrication_number),
            ],
            reference_data: Some(din_reference()),
            warnings: vec![
                "Public structure validation does not prove manufacturer registration or device existence."
                    .to_string(),
            ],
            errors: Vec::new(),
        },
        Err(error) => invalid_report(
            IdentifierKind::Din43849,
            input,
            error.to_string(),
            AllocationStatus::Unknown,
            din_reference(),
        ),
    };
    Ok(Json(report))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload(id: &str) -> ValidatePayload {
        Ok(Json(ValidateRequest { id: id.to_string() }))
    }

    #[tokio::test]
    async fn obis_validation_and_lookup_keep_structure_and_catalog_membership_separate() {
        let Json(validation) = handle_obis_validate(payload("1-0:1.8.0*255"))
            .await
            .unwrap();
        assert!(validation.valid);
        assert_eq!(validation.checks.directory, CheckStatus::NotChecked);

        let Json(lookup) = handle_obis_lookup(payload("1-0:1.8.0*255")).await.unwrap();
        assert_eq!(lookup.lookup_status, ObisLookupStatus::Found);
        assert_eq!(lookup.checks.directory, CheckStatus::Found);
        assert_eq!(lookup.entry.unwrap().unit, "kWh");

        let Json(miss) = handle_obis_lookup(payload("1-0:99.1.0")).await.unwrap();
        assert_eq!(miss.lookup_status, ObisLookupStatus::NotFound);
        assert_eq!(miss.checks.directory, CheckStatus::NotFound);
        assert!(miss.entry.is_none());
    }

    #[tokio::test]
    async fn din_validation_reports_no_checksum_or_assignment_claim() {
        let Json(report) = handle_din_43849_validate(payload("7 QDS 01 1122 3344"))
            .await
            .unwrap();
        assert!(report.valid);
        assert_eq!(report.normalized.as_deref(), Some("7QDS0111223344"));
        assert_eq!(report.checks.checksum, CheckStatus::NotApplicable);
        assert_eq!(report.checks.directory, CheckStatus::NotChecked);
        assert_eq!(report.checks.assignment, CheckStatus::Unknown);
        assert_eq!(report.allocation_status, AllocationStatus::Unknown);
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
