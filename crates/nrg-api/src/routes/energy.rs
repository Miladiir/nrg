use axum::Json;
use id_core::{
    catalog::{
        AccountExistenceStatus, AllocationStatus, CheckStatus, Checks, CollisionGuarantee,
        GeneratedIdentifier, GenerationProfile, IdentifierKind, IdentifierPart, ReferenceData,
        Sector, ValidationReport,
    },
    generate_malo_seeded, generate_melo_seeded, generate_nelo_seeded,
    identifiers::energy::{
        generate_bdew_market_partner_id, generate_cr_id, generate_dvgw_market_partner_id,
        generate_nebe_id, generate_package_id, generate_sg_id, generate_sr_id, generate_tr_id,
        validate_market_partner_id, validate_nebe_id, MarketPartnerIdError, MarketPartnerIdKind,
        NetworkIdentifierError,
    },
    reference_data::BDEW_IDENTIFIERS_METADATA,
    validate_malo, validate_melo, validate_nelo,
};

use super::{
    generate_batch, GenerateApiResponses, GeneratePayload, GenerateResponse, PreparedGeneration,
    ValidationApiResponses,
};
use crate::{parse_validate_payload, ApiError, ValidatePayload, ValidateRequest};

const CENTRAL_GENERATION_WARNING: &str =
    "Synthetic test value; syntax and checksum do not prove allocation.";
const CENTRAL_VALIDATION_WARNING: &str =
    "Syntax and checksum validation do not prove real-world allocation.";

fn bdew_reference() -> ReferenceData {
    ReferenceData {
        name: BDEW_IDENTIFIERS_METADATA.name.to_string(),
        version: Some(format!(
            "{}; checked {}",
            BDEW_IDENTIFIERS_METADATA.version, BDEW_IDENTIFIERS_METADATA.checked_at
        )),
        valid_from: None,
        valid_to: None,
        sha256: Some(BDEW_IDENTIFIERS_METADATA.sha256.to_string()),
    }
}

fn central_checks(checksum: CheckStatus) -> Checks {
    Checks {
        syntax: CheckStatus::Valid,
        checksum,
        directory: CheckStatus::NotApplicable,
        assignment: CheckStatus::Unknown,
    }
}

fn generated_central(
    kind: IdentifierKind,
    profile: GenerationProfile,
    value: String,
    checksum: CheckStatus,
    parts: Vec<IdentifierPart>,
) -> GeneratedIdentifier {
    GeneratedIdentifier {
        value,
        formatted: None,
        kind,
        profile,
        synthetic: true,
        production_usable: false,
        checks: central_checks(checksum),
        allocation_status: AllocationStatus::Unknown,
        account_existence: AccountExistenceStatus::NotApplicable,
        collision_guarantee: CollisionGuarantee::None,
        parts,
        reference_data: Some(bdew_reference()),
        generator_version: id_core::GENERATOR_VERSION.to_string(),
        warnings: vec![CENTRAL_GENERATION_WARNING.to_string()],
    }
}

fn invalid_report(
    kind: IdentifierKind,
    input: String,
    error: String,
    checksum: CheckStatus,
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
            directory: CheckStatus::NotApplicable,
            assignment: CheckStatus::Unknown,
        },
        allocation_status: AllocationStatus::Unknown,
        synthetic: None,
        production_usable: None,
        parts: Vec::new(),
        reference_data: Some(bdew_reference()),
        warnings: Vec::new(),
        errors: vec![error],
    }
}

fn generate_malo_item(
    prepared: &PreparedGeneration,
    index: u32,
) -> Result<GeneratedIdentifier, String> {
    let value = generate_malo_seeded(&prepared.fixture_seed, index);
    let info = validate_malo(&value)?;
    Ok(generated_central(
        prepared.kind,
        prepared.profile,
        value,
        CheckStatus::Valid,
        vec![
            IdentifierPart::new("check_digit", info.checksum.to_string()),
            IdentifierPart::new("issuing_authority", info.issuer),
        ],
    ))
}

fn generate_melo_item(
    prepared: &PreparedGeneration,
    index: u32,
) -> Result<GeneratedIdentifier, String> {
    let value = generate_melo_seeded(&prepared.fixture_seed, index);
    validate_melo(&value)?;
    Ok(generated_central(
        prepared.kind,
        prepared.profile,
        value.clone(),
        CheckStatus::NotApplicable,
        vec![
            IdentifierPart::new("country", &value[0..2]),
            IdentifierPart::new("network_operator", &value[2..8]),
            IdentifierPart::new("postal_code", &value[8..13]),
            IdentifierPart::new("meter_point", &value[13..33]),
        ],
    ))
}

fn generate_nelo_item(
    prepared: &PreparedGeneration,
    index: u32,
) -> Result<GeneratedIdentifier, String> {
    let value = generate_nelo_seeded(&prepared.fixture_seed, index);
    validate_nelo(&value)?;
    Ok(generated_central(
        prepared.kind,
        prepared.profile,
        value.clone(),
        CheckStatus::Valid,
        vec![
            IdentifierPart::new("code_type", "E"),
            IdentifierPart::new("check_digit", &value[10..11]),
        ],
    ))
}

fn generate_nebe_item(
    prepared: &PreparedGeneration,
    index: u32,
) -> Result<GeneratedIdentifier, String> {
    let generated = generate_nebe_id(&prepared.fixture_seed, index);
    Ok(generated_central(
        prepared.kind,
        prepared.profile,
        generated.value,
        CheckStatus::Valid,
        vec![
            IdentifierPart::new("code_type", "F"),
            IdentifierPart::new("check_digit", generated.check_digit.to_string()),
        ],
    ))
}

fn generate_package_item(
    prepared: &PreparedGeneration,
    index: u32,
) -> Result<GeneratedIdentifier, String> {
    let generated = generate_package_id(&prepared.fixture_seed, index);
    Ok(generated_central(
        prepared.kind,
        prepared.profile,
        generated.value,
        CheckStatus::Valid,
        vec![
            IdentifierPart::new("code_type", "P"),
            IdentifierPart::new("issuing_authority", "BDEW"),
            IdentifierPart::new("check_digit", generated.check_digit.to_string()),
        ],
    ))
}

fn generate_market_partner_item(
    prepared: &PreparedGeneration,
    index: u32,
) -> Result<GeneratedIdentifier, String> {
    let generated = match prepared.sector.unwrap_or(Sector::Electricity) {
        Sector::Electricity => generate_bdew_market_partner_id(&prepared.fixture_seed, index),
        Sector::Gas => generate_dvgw_market_partner_id(&prepared.fixture_seed, index),
        Sector::CrossSector => {
            return Err("MP-ID generation requires sector 'electricity' or 'gas'".to_string())
        }
    };
    let (issuer, sector) = match generated.kind {
        MarketPartnerIdKind::BdewElectricity => ("BDEW", "electricity"),
        MarketPartnerIdKind::DvgwGas => ("DVGW", "gas"),
    };
    Ok(generated_central(
        prepared.kind,
        prepared.profile,
        generated.value,
        CheckStatus::Valid,
        vec![
            IdentifierPart::new("issuing_authority", issuer),
            IdentifierPart::new("sector", sector),
            IdentifierPart::new("check_digit", generated.check_digit.to_string()),
        ],
    ))
}

fn generate_resource_item(
    prepared: &PreparedGeneration,
    index: u32,
) -> Result<GeneratedIdentifier, String> {
    let generated = match prepared.kind {
        IdentifierKind::ClusterResourceId => generate_cr_id(&prepared.fixture_seed, index),
        IdentifierKind::SteeringGroupId => generate_sg_id(&prepared.fixture_seed, index),
        IdentifierKind::ControllableResourceId => generate_sr_id(&prepared.fixture_seed, index),
        IdentifierKind::TechnicalResourceId => generate_tr_id(&prepared.fixture_seed, index),
        _ => return Err("unsupported resource identifier kind".to_string()),
    };
    Ok(generated_central(
        prepared.kind,
        prepared.profile,
        generated.value,
        CheckStatus::Valid,
        vec![
            IdentifierPart::new("code_type", generated.kind.prefix().to_string()),
            IdentifierPart::new("check_digit", generated.check_digit.to_string()),
        ],
    ))
}

pub(super) fn generate_item(
    prepared: &PreparedGeneration,
    index: u32,
) -> Result<GeneratedIdentifier, String> {
    match prepared.kind {
        IdentifierKind::Malo => generate_malo_item(prepared, index),
        IdentifierKind::Melo => generate_melo_item(prepared, index),
        IdentifierKind::Nelo => generate_nelo_item(prepared, index),
        IdentifierKind::Nebe => generate_nebe_item(prepared, index),
        IdentifierKind::MarketPartnerId => generate_market_partner_item(prepared, index),
        IdentifierKind::ClusterResourceId
        | IdentifierKind::SteeringGroupId
        | IdentifierKind::ControllableResourceId
        | IdentifierKind::TechnicalResourceId => generate_resource_item(prepared, index),
        IdentifierKind::PackageId => generate_package_item(prepared, index),
        _ => Err("unsupported energy identifier kind".to_string()),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/energy/locations/malo/generate",
    operation_id = "energyMaloGenerate",
    request_body = id_core::catalog::GenerateRequest,
    responses(GenerateApiResponses),
    tag = "Energie · Lokationen"
)]
pub(crate) async fn handle_malo_generate(
    payload: GeneratePayload,
) -> Result<Json<GenerateResponse>, ApiError> {
    generate_batch(IdentifierKind::Malo, payload, generate_malo_item)
}

#[utoipa::path(
    post,
    path = "/api/v1/energy/locations/melo/generate",
    operation_id = "energyMeloGenerate",
    request_body = id_core::catalog::GenerateRequest,
    responses(GenerateApiResponses),
    tag = "Energie · Lokationen"
)]
pub(crate) async fn handle_melo_generate(
    payload: GeneratePayload,
) -> Result<Json<GenerateResponse>, ApiError> {
    generate_batch(IdentifierKind::Melo, payload, generate_melo_item)
}

#[utoipa::path(
    post,
    path = "/api/v1/energy/locations/nelo/generate",
    operation_id = "energyNeloGenerate",
    request_body = id_core::catalog::GenerateRequest,
    responses(GenerateApiResponses),
    tag = "Energie · Lokationen"
)]
pub(crate) async fn handle_nelo_generate(
    payload: GeneratePayload,
) -> Result<Json<GenerateResponse>, ApiError> {
    generate_batch(IdentifierKind::Nelo, payload, generate_nelo_item)
}

#[utoipa::path(
    post,
    path = "/api/v1/energy/locations/nebe/generate",
    operation_id = "energyNebeGenerate",
    request_body = id_core::catalog::GenerateRequest,
    responses(GenerateApiResponses),
    tag = "Energie · Lokationen"
)]
pub(crate) async fn handle_nebe_generate(
    payload: GeneratePayload,
) -> Result<Json<GenerateResponse>, ApiError> {
    generate_batch(IdentifierKind::Nebe, payload, generate_nebe_item)
}

#[utoipa::path(
    post,
    path = "/api/v1/energy/market-partners/mp-id/generate",
    operation_id = "energyMarketPartnerIdGenerate",
    request_body = id_core::catalog::GenerateRequest,
    responses(GenerateApiResponses),
    tag = "Energie · Marktpartner"
)]
pub(crate) async fn handle_market_partner_generate(
    payload: GeneratePayload,
) -> Result<Json<GenerateResponse>, ApiError> {
    generate_batch(
        IdentifierKind::MarketPartnerId,
        payload,
        generate_market_partner_item,
    )
}

macro_rules! resource_generate_handler {
    ($name:ident, $path:literal, $operation_id:literal, $kind:expr) => {
        #[utoipa::path(
                                            post,
                                            path = $path,
                                            operation_id = $operation_id,
                                            request_body = id_core::catalog::GenerateRequest,
                                            responses(GenerateApiResponses),
                                            tag = "Energie · Ressourcen & Redispatch"
                                        )]
        pub(crate) async fn $name(
            payload: GeneratePayload,
        ) -> Result<Json<GenerateResponse>, ApiError> {
            generate_batch($kind, payload, generate_resource_item)
        }
    };
}

resource_generate_handler!(
    handle_cr_generate,
    "/api/v1/energy/resources/cr-id/generate",
    "energyClusterResourceIdGenerate",
    IdentifierKind::ClusterResourceId
);

#[utoipa::path(
    post,
    path = "/api/v1/energy/resources/package-id/generate",
    operation_id = "energyPackageIdGenerate",
    request_body = id_core::catalog::GenerateRequest,
    responses(GenerateApiResponses),
    tag = "Energie · Ressourcen & Redispatch"
)]
pub(crate) async fn handle_package_generate(
    payload: GeneratePayload,
) -> Result<Json<GenerateResponse>, ApiError> {
    generate_batch(IdentifierKind::PackageId, payload, generate_package_item)
}
resource_generate_handler!(
    handle_sg_generate,
    "/api/v1/energy/resources/sg-id/generate",
    "energySteeringGroupIdGenerate",
    IdentifierKind::SteeringGroupId
);
resource_generate_handler!(
    handle_sr_generate,
    "/api/v1/energy/resources/sr-id/generate",
    "energyControllableResourceIdGenerate",
    IdentifierKind::ControllableResourceId
);
resource_generate_handler!(
    handle_tr_generate,
    "/api/v1/energy/resources/tr-id/generate",
    "energyTechnicalResourceIdGenerate",
    IdentifierKind::TechnicalResourceId
);

#[utoipa::path(
    post,
    path = "/api/v1/energy/locations/malo/validate",
    operation_id = "energyMaloValidate",
    request_body = ValidateRequest,
    responses(ValidationApiResponses),
    tag = "Energie · Lokationen"
)]
pub(crate) async fn handle_malo_validate(
    payload: ValidatePayload,
) -> Result<Json<ValidationReport>, ApiError> {
    let request = parse_validate_payload(payload)?;
    let input = request.id;
    let report = match validate_malo(&input) {
        Ok(info) => ValidationReport {
            kind: IdentifierKind::Malo,
            input: input.clone(),
            normalized: Some(info.id),
            valid: true,
            checks: central_checks(CheckStatus::Valid),
            allocation_status: AllocationStatus::Unknown,
            synthetic: None,
            production_usable: None,
            parts: vec![
                IdentifierPart::new("check_digit", info.checksum.to_string()),
                IdentifierPart::new("issuing_authority", info.issuer),
            ],
            reference_data: Some(bdew_reference()),
            warnings: vec![CENTRAL_VALIDATION_WARNING.to_string()],
            errors: Vec::new(),
        },
        Err(error) => invalid_report(
            IdentifierKind::Malo,
            input,
            error.clone(),
            if error.starts_with("Invalid checksum") {
                CheckStatus::Invalid
            } else {
                CheckStatus::NotChecked
            },
        ),
    };
    Ok(Json(report))
}

#[utoipa::path(
    post,
    path = "/api/v1/energy/locations/melo/validate",
    operation_id = "energyMeloValidate",
    request_body = ValidateRequest,
    responses(ValidationApiResponses),
    tag = "Energie · Lokationen"
)]
pub(crate) async fn handle_melo_validate(
    payload: ValidatePayload,
) -> Result<Json<ValidationReport>, ApiError> {
    let request = parse_validate_payload(payload)?;
    let input = request.id;
    let report = match validate_melo(&input) {
        Ok(()) => ValidationReport {
            kind: IdentifierKind::Melo,
            input: input.clone(),
            normalized: Some(input.clone()),
            valid: true,
            checks: central_checks(CheckStatus::NotApplicable),
            allocation_status: AllocationStatus::Unknown,
            synthetic: None,
            production_usable: None,
            parts: vec![
                IdentifierPart::new("country", &input[0..2]),
                IdentifierPart::new("network_operator", &input[2..8]),
                IdentifierPart::new("postal_code", &input[8..13]),
                IdentifierPart::new("meter_point", &input[13..33]),
            ],
            reference_data: Some(bdew_reference()),
            warnings: vec![CENTRAL_VALIDATION_WARNING.to_string()],
            errors: Vec::new(),
        },
        Err(error) => invalid_report(
            IdentifierKind::Melo,
            input,
            error,
            CheckStatus::NotApplicable,
        ),
    };
    Ok(Json(report))
}

#[utoipa::path(
    post,
    path = "/api/v1/energy/locations/nelo/validate",
    operation_id = "energyNeloValidate",
    request_body = ValidateRequest,
    responses(ValidationApiResponses),
    tag = "Energie · Lokationen"
)]
pub(crate) async fn handle_nelo_validate(
    payload: ValidatePayload,
) -> Result<Json<ValidationReport>, ApiError> {
    let request = parse_validate_payload(payload)?;
    let input = request.id;
    let report = match validate_nelo(&input) {
        Ok(()) => ValidationReport {
            kind: IdentifierKind::Nelo,
            input: input.clone(),
            normalized: Some(input.clone()),
            valid: true,
            checks: central_checks(CheckStatus::Valid),
            allocation_status: AllocationStatus::Unknown,
            synthetic: None,
            production_usable: None,
            parts: vec![
                IdentifierPart::new("code_type", "E"),
                IdentifierPart::new("check_digit", &input[10..11]),
            ],
            reference_data: Some(bdew_reference()),
            warnings: vec![CENTRAL_VALIDATION_WARNING.to_string()],
            errors: Vec::new(),
        },
        Err(error) => invalid_report(
            IdentifierKind::Nelo,
            input,
            error.clone(),
            if error.starts_with("Invalid checksum") {
                CheckStatus::Invalid
            } else {
                CheckStatus::NotChecked
            },
        ),
    };
    Ok(Json(report))
}

#[utoipa::path(
    post,
    path = "/api/v1/energy/locations/nebe/validate",
    operation_id = "energyNebeValidate",
    request_body = ValidateRequest,
    responses(ValidationApiResponses),
    tag = "Energie · Lokationen"
)]
pub(crate) async fn handle_nebe_validate(
    payload: ValidatePayload,
) -> Result<Json<ValidationReport>, ApiError> {
    let request = parse_validate_payload(payload)?;
    let input = request.id;
    let report = match validate_nebe_id(&input) {
        Ok(info) => ValidationReport {
            kind: IdentifierKind::Nebe,
            input: input.clone(),
            normalized: Some(info.value),
            valid: true,
            checks: central_checks(CheckStatus::Valid),
            allocation_status: AllocationStatus::Unknown,
            synthetic: None,
            production_usable: None,
            parts: vec![
                IdentifierPart::new("code_type", "F"),
                IdentifierPart::new("check_digit", info.check_digit.to_string()),
            ],
            reference_data: Some(bdew_reference()),
            warnings: vec![CENTRAL_VALIDATION_WARNING.to_string()],
            errors: Vec::new(),
        },
        Err(error) => {
            let checksum = if matches!(error, NetworkIdentifierError::ChecksumMismatch { .. }) {
                CheckStatus::Invalid
            } else {
                CheckStatus::NotChecked
            };
            invalid_report(IdentifierKind::Nebe, input, error.to_string(), checksum)
        }
    };
    Ok(Json(report))
}

#[utoipa::path(
    post,
    path = "/api/v1/energy/market-partners/mp-id/validate",
    operation_id = "energyMarketPartnerIdValidate",
    request_body = ValidateRequest,
    responses(ValidationApiResponses),
    tag = "Energie · Marktpartner"
)]
pub(crate) async fn handle_market_partner_validate(
    payload: ValidatePayload,
) -> Result<Json<ValidationReport>, ApiError> {
    let request = parse_validate_payload(payload)?;
    let input = request.id;
    let report = match validate_market_partner_id(&input) {
        Ok(info) => {
            let (issuer, sector) = match info.kind {
                MarketPartnerIdKind::BdewElectricity => ("BDEW", "electricity"),
                MarketPartnerIdKind::DvgwGas => ("DVGW", "gas"),
            };
            ValidationReport {
                kind: IdentifierKind::MarketPartnerId,
                input: input.clone(),
                normalized: Some(info.value),
                valid: true,
                checks: central_checks(CheckStatus::Valid),
                allocation_status: AllocationStatus::Unknown,
                synthetic: None,
                production_usable: None,
                parts: vec![
                    IdentifierPart::new("issuing_authority", issuer),
                    IdentifierPart::new("sector", sector),
                    IdentifierPart::new("check_digit", info.check_digit.to_string()),
                ],
                reference_data: Some(bdew_reference()),
                warnings: vec![CENTRAL_VALIDATION_WARNING.to_string()],
                errors: Vec::new(),
            }
        }
        Err(error) => {
            let checksum = if matches!(error, MarketPartnerIdError::ChecksumMismatch { .. }) {
                CheckStatus::Invalid
            } else {
                CheckStatus::NotChecked
            };
            invalid_report(
                IdentifierKind::MarketPartnerId,
                input,
                error.to_string(),
                checksum,
            )
        }
    };
    Ok(Json(report))
}
