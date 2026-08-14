use axum::{extract::rejection::JsonRejection, Json};
use id_core::{
    catalog::{
        AccountExistenceStatus, AllocationStatus, CheckStatus, Checks, CollisionGuarantee,
        GenerateRequest, GeneratedIdentifier, GenerationProfile, IdentifierKind, IdentifierPart,
        ReferenceData, ValidationReport,
    },
    identifiers::payments::{
        bic::{
            generate_bic_directory_value_with_branch, generate_bic_syntax_only,
            generate_bic_test_training_pattern, validate_bic,
        },
        creditor_id::{
            generate_creditor_id_official_test_fixture, validate_german_creditor_id,
            CreditorIdError,
        },
        end_to_end_id::{generate_end_to_end_id, validate_end_to_end_id},
        iban::{
            generate_iban_checksum_only, generate_iban_directory_plausible,
            generate_iban_synthetic_non_routable, validate_german_iban_with_directory,
            IbanDirectoryStatus, IbanError,
        },
        international_iban::{
            generate_international_iban_checksum_only, international_iban_official_example,
            validate_international_iban, InternationalIbanError, IBAN_REGISTRY_DATA_SHA256,
            IBAN_REGISTRY_NAME, IBAN_REGISTRY_PUBLISHED, IBAN_REGISTRY_RELEASE,
        },
        mandate_reference::{generate_mandate_reference, validate_mandate_reference},
        rf_reference::{
            build_rf_reference, generate_rf_reference, validate_rf_reference, RfReferenceError,
        },
        uetr::{generate_uetr, validate_uetr},
    },
    reference_data::{
        BundesbankBlzDirectory, BUNDESBANK_BLZ_NAME, BUNDESBANK_BLZ_SOURCE_SHA256,
        BUNDESBANK_BLZ_VALID_FROM, BUNDESBANK_BLZ_VALID_TO,
    },
};
use serde::Deserialize;
use utoipa::ToSchema;

use super::{
    generate_batch, generate_prepared_batch, prepare_generation, rendered_value,
    GenerateApiResponses, GeneratePayload, GenerateResponse, PreparedGeneration,
    ValidationApiResponses,
};
use crate::{parse_validate_payload, ApiError, ValidatePayload, ValidateRequest};

const PAYMENT_GENERATION_WARNING: &str =
    "Test value only; validation does not prove account, institution, or assignment existence.";
const PAYMENT_VALIDATION_WARNING: &str = "Syntax, checksum and directory checks do not prove account existence, routability or real-world assignment.";

fn bundesbank_reference() -> ReferenceData {
    ReferenceData {
        name: BUNDESBANK_BLZ_NAME.to_string(),
        version: Some(format!(
            "{BUNDESBANK_BLZ_VALID_FROM}_{BUNDESBANK_BLZ_VALID_TO}"
        )),
        valid_from: Some(BUNDESBANK_BLZ_VALID_FROM.to_string()),
        valid_to: Some(BUNDESBANK_BLZ_VALID_TO.to_string()),
        sha256: Some(BUNDESBANK_BLZ_SOURCE_SHA256.to_string()),
    }
}

fn iban_registry_reference() -> ReferenceData {
    ReferenceData {
        name: IBAN_REGISTRY_NAME.to_string(),
        version: Some(format!("release-{IBAN_REGISTRY_RELEASE}")),
        valid_from: Some(IBAN_REGISTRY_PUBLISHED.to_string()),
        valid_to: None,
        sha256: Some(IBAN_REGISTRY_DATA_SHA256.to_string()),
    }
}

fn creditor_id_reference() -> ReferenceData {
    ReferenceData {
        name: "bundesbank_creditor_id".to_string(),
        version: Some("official-test-fixture-and-format-rules; checked-2026-08-14".to_string()),
        valid_from: None,
        valid_to: None,
        sha256: None,
    }
}

fn invalid_report(
    kind: IdentifierKind,
    input: String,
    error: String,
    checksum: CheckStatus,
    directory: CheckStatus,
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
            directory,
            assignment: CheckStatus::Unknown,
        },
        allocation_status: AllocationStatus::Unknown,
        synthetic: None,
        production_usable: None,
        parts: Vec::new(),
        reference_data: None,
        warnings: Vec::new(),
        errors: vec![error],
    }
}

fn invalid_non_allocated_report(
    kind: IdentifierKind,
    input: String,
    error: String,
    checksum: CheckStatus,
) -> ValidationReport {
    let mut report = invalid_report(kind, input, error, checksum, CheckStatus::NotApplicable);
    report.checks.assignment = CheckStatus::NotApplicable;
    report.allocation_status = AllocationStatus::NotApplicable;
    report
}

fn iban_item(prepared: &PreparedGeneration, index: u32) -> Result<GeneratedIdentifier, String> {
    let country = prepared
        .country
        .as_deref()
        .unwrap_or("DE")
        .trim()
        .to_ascii_uppercase();
    if country != "DE" || prepared.profile == GenerationProfile::OfficialExample {
        return international_iban_item(prepared, index, &country);
    }

    let directory = BundesbankBlzDirectory;
    let (
        generated,
        directory_status,
        assignment_status,
        allocation_status,
        reference_data,
        warning,
    ) = match prepared.profile {
        GenerationProfile::SyntheticNonRoutable => (
            generate_iban_synthetic_non_routable(&prepared.fixture_seed, index, &directory)
                .map_err(|error| error.to_string())?,
            CheckStatus::NotFound,
            CheckStatus::NotApplicable,
            AllocationStatus::NotApplicable,
            Some(bundesbank_reference()),
            "Bank code is absent from the embedded snapshot; re-check after every snapshot update.",
        ),
        GenerationProfile::DirectoryPlausible => (
            generate_iban_directory_plausible(&prepared.fixture_seed, index, &directory)
                .map_err(|error| error.to_string())?,
            CheckStatus::Found,
            CheckStatus::Unknown,
            AllocationStatus::Unknown,
            Some(bundesbank_reference()),
            "Uses a real directory bank code; account existence and collision status are unknown.",
        ),
        GenerationProfile::ChecksumOnly => (
            generate_iban_checksum_only(&prepared.fixture_seed, index)
                .map_err(|error| error.to_string())?,
            CheckStatus::NotChecked,
            CheckStatus::Unknown,
            AllocationStatus::Unknown,
            None,
            "Checksum-only value; no bank or account existence claim is made.",
        ),
        _ => return Err("unsupported IBAN generation profile".to_string()),
    };

    let mut parts = vec![
        IdentifierPart::new("country", generated.parts.country_code),
        IdentifierPart::new("check_digits", generated.parts.check_digits),
        IdentifierPart::new("bank_code", generated.parts.bank_code),
        IdentifierPart::new("account_number", generated.parts.account_number),
    ];
    if let Some(bic) = generated.directory_bic {
        parts.push(IdentifierPart::new("bic", bic));
    }
    Ok(GeneratedIdentifier {
        value: rendered_value(
            &generated.value,
            Some(&generated.formatted),
            prepared.format,
        ),
        formatted: Some(generated.formatted),
        kind: prepared.kind,
        profile: prepared.profile,
        synthetic: true,
        production_usable: false,
        checks: Checks {
            syntax: CheckStatus::Valid,
            checksum: CheckStatus::Valid,
            directory: directory_status,
            assignment: assignment_status,
        },
        allocation_status,
        account_existence: AccountExistenceStatus::Unknown,
        collision_guarantee: CollisionGuarantee::None,
        parts,
        reference_data,
        generator_version: id_core::GENERATOR_VERSION.to_string(),
        warnings: vec![warning.to_string(), PAYMENT_GENERATION_WARNING.to_string()],
    })
}

fn international_iban_item(
    prepared: &PreparedGeneration,
    index: u32,
    country: &str,
) -> Result<GeneratedIdentifier, String> {
    let (generated, synthetic, warning) = match prepared.profile {
        GenerationProfile::ChecksumOnly => (
            generate_international_iban_checksum_only(
                country,
                &prepared.fixture_seed,
                index,
            )
            .map_err(|error| error.to_string())?,
            true,
            "Checksum-only international IBAN; national account checks, bank presence and account existence are not checked.",
        ),
        GenerationProfile::OfficialExample => (
            international_iban_official_example(country).map_err(|error| error.to_string())?,
            false,
            "Official SWIFT registry example; it is not a non-routable or sandbox guarantee.",
        ),
        _ => {
            return Err(format!(
                "profile '{}' is available only for German IBANs; use checksum_only or official_example for {country}",
                prepared.profile.as_str()
            ))
        }
    };
    let mut parts = vec![
        IdentifierPart::new("country", generated.parts.country_code),
        IdentifierPart::new("country_name", generated.parts.country_name),
        IdentifierPart::new("check_digits", generated.parts.check_digits),
        IdentifierPart::new("bban", generated.parts.bban),
        IdentifierPart::new("sepa", generated.parts.sepa.to_string()),
    ];
    if let Some(bank_identifier) = generated.parts.bank_identifier {
        parts.push(IdentifierPart::new("bank_identifier", bank_identifier));
    }
    if let Some(branch_identifier) = generated.parts.branch_identifier {
        parts.push(IdentifierPart::new("branch_identifier", branch_identifier));
    }
    Ok(GeneratedIdentifier {
        value: rendered_value(
            &generated.value,
            Some(&generated.formatted),
            prepared.format,
        ),
        formatted: Some(generated.formatted),
        kind: prepared.kind,
        profile: prepared.profile,
        synthetic,
        production_usable: false,
        checks: Checks {
            syntax: CheckStatus::Valid,
            checksum: CheckStatus::Valid,
            directory: CheckStatus::NotChecked,
            assignment: CheckStatus::Unknown,
        },
        allocation_status: AllocationStatus::Unknown,
        account_existence: AccountExistenceStatus::Unknown,
        collision_guarantee: CollisionGuarantee::None,
        parts,
        reference_data: Some(iban_registry_reference()),
        generator_version: id_core::GENERATOR_VERSION.to_string(),
        warnings: vec![warning.to_string(), PAYMENT_GENERATION_WARNING.to_string()],
    })
}

fn bic_item(
    prepared: &PreparedGeneration,
    index: u32,
    include_branch: bool,
) -> Result<GeneratedIdentifier, String> {
    let directory = BundesbankBlzDirectory;
    let generated = match prepared.profile {
        GenerationProfile::TestTrainingPattern => {
            generate_bic_test_training_pattern(&prepared.fixture_seed, index, include_branch)
        }
        GenerationProfile::SyntaxOnly => {
            generate_bic_syntax_only(&prepared.fixture_seed, index, include_branch)
        }
        GenerationProfile::DirectoryValue => generate_bic_directory_value_with_branch(
            &prepared.fixture_seed,
            index,
            &directory,
            include_branch,
        ),
        _ => return Err("unsupported BIC generation profile".to_string()),
    }
    .map_err(|error| error.to_string())?;
    let directory_status = if prepared.profile == GenerationProfile::DirectoryValue {
        CheckStatus::Found
    } else {
        CheckStatus::NotChecked
    };
    let reference_data =
        (prepared.profile == GenerationProfile::DirectoryValue).then(bundesbank_reference);
    let warning = match prepared.profile {
        GenerationProfile::TestTrainingPattern => {
            "T&T syntax pattern only; SWIFT registration is unknown."
        }
        GenerationProfile::DirectoryValue => {
            "Actual Bundesbank directory value; no collision or sandbox guarantee."
        }
        _ => "Syntax-only BIC; SWIFT registration is unknown.",
    };
    let mut parts = vec![
        IdentifierPart::new(
            "business_party_prefix",
            generated.parts.business_party_prefix,
        ),
        IdentifierPart::new("country", generated.parts.country_code),
        IdentifierPart::new("location", generated.parts.location_code),
        IdentifierPart::new(
            "test_training_pattern",
            generated.parts.test_training_pattern.to_string(),
        ),
    ];
    if let Some(branch) = generated.parts.branch_code {
        parts.push(IdentifierPart::new("branch", branch));
    }
    if let Some(bank_code) = generated.directory_bank_code {
        parts.push(IdentifierPart::new("bank_code", bank_code));
    }
    Ok(GeneratedIdentifier {
        value: generated.value,
        formatted: None,
        kind: prepared.kind,
        profile: prepared.profile,
        synthetic: generated.synthetic,
        production_usable: false,
        checks: Checks {
            syntax: CheckStatus::Valid,
            checksum: CheckStatus::NotApplicable,
            directory: directory_status,
            assignment: CheckStatus::Unknown,
        },
        allocation_status: AllocationStatus::Unknown,
        account_existence: AccountExistenceStatus::NotApplicable,
        collision_guarantee: CollisionGuarantee::None,
        parts,
        reference_data,
        generator_version: id_core::GENERATOR_VERSION.to_string(),
        warnings: vec![warning.to_string()],
    })
}

fn creditor_id_item(
    prepared: &PreparedGeneration,
    index: u32,
) -> Result<GeneratedIdentifier, String> {
    let generated = match prepared.profile {
        GenerationProfile::OfficialTestFixture => {
            generate_creditor_id_official_test_fixture(&prepared.fixture_seed, index)
        }
        _ => return Err("unsupported creditor-ID generation profile".to_string()),
    }
    .map_err(|error| error.to_string())?;
    Ok(GeneratedIdentifier {
        value: generated.value,
        formatted: None,
        kind: prepared.kind,
        profile: prepared.profile,
        synthetic: generated.synthetic,
        production_usable: false,
        checks: Checks {
            syntax: CheckStatus::Valid,
            checksum: CheckStatus::Valid,
            directory: CheckStatus::NotApplicable,
            assignment: CheckStatus::NotApplicable,
        },
        allocation_status: AllocationStatus::NotApplicable,
        account_existence: AccountExistenceStatus::NotApplicable,
        collision_guarantee: CollisionGuarantee::None,
        parts: vec![
            IdentifierPart::new("country", generated.parts.country_code),
            IdentifierPart::new("check_digits", generated.parts.check_digits),
            IdentifierPart::new("business_code", generated.parts.business_code),
            IdentifierPart::new("national_identifier", generated.parts.national_identifier),
        ],
        reference_data: Some(creditor_id_reference()),
        generator_version: id_core::GENERATOR_VERSION.to_string(),
        warnings: vec!["Official Bundesbank test fixture; not for production use.".to_string()],
    })
}

fn mandate_item(prepared: &PreparedGeneration, index: u32) -> Result<GeneratedIdentifier, String> {
    let generated = generate_mandate_reference(&prepared.fixture_seed, index);
    Ok(self_assigned_item(
        prepared,
        generated.value,
        CheckStatus::NotApplicable,
    ))
}

fn end_to_end_item(
    prepared: &PreparedGeneration,
    index: u32,
) -> Result<GeneratedIdentifier, String> {
    let generated = generate_end_to_end_id(&prepared.fixture_seed, index);
    Ok(self_assigned_item(
        prepared,
        generated.value,
        CheckStatus::NotApplicable,
    ))
}

fn uetr_item(prepared: &PreparedGeneration, index: u32) -> Result<GeneratedIdentifier, String> {
    let generated = generate_uetr(&prepared.fixture_seed, index);
    Ok(GeneratedIdentifier {
        value: generated.value,
        formatted: None,
        kind: prepared.kind,
        profile: prepared.profile,
        synthetic: generated.synthetic,
        production_usable: generated.production_usable,
        checks: Checks {
            syntax: CheckStatus::Valid,
            checksum: CheckStatus::NotApplicable,
            directory: CheckStatus::NotApplicable,
            assignment: CheckStatus::NotApplicable,
        },
        allocation_status: AllocationStatus::NotApplicable,
        account_existence: AccountExistenceStatus::NotApplicable,
        collision_guarantee: CollisionGuarantee::WithinBatch,
        parts: vec![
            IdentifierPart::new("uuid_version", generated.parts.version.to_string()),
            IdentifierPart::new("uuid_variant", "rfc4122"),
        ],
        reference_data: None,
        generator_version: id_core::GENERATOR_VERSION.to_string(),
        warnings: vec!["Deterministic UUID-v4 fixture; not cryptographically random.".to_string()],
    })
}

fn self_assigned_item(
    prepared: &PreparedGeneration,
    value: String,
    checksum: CheckStatus,
) -> GeneratedIdentifier {
    GeneratedIdentifier {
        value,
        formatted: None,
        kind: prepared.kind,
        profile: prepared.profile,
        synthetic: true,
        production_usable: false,
        checks: Checks {
            syntax: CheckStatus::Valid,
            checksum,
            directory: CheckStatus::NotApplicable,
            assignment: CheckStatus::NotApplicable,
        },
        allocation_status: AllocationStatus::NotApplicable,
        account_existence: AccountExistenceStatus::NotApplicable,
        collision_guarantee: CollisionGuarantee::WithinBatch,
        parts: Vec::new(),
        reference_data: None,
        generator_version: id_core::GENERATOR_VERSION.to_string(),
        warnings: vec!["Synthetic self-assigned test reference.".to_string()],
    }
}

fn rf_item(prepared: &PreparedGeneration, index: u32) -> Result<GeneratedIdentifier, String> {
    let generated =
        generate_rf_reference(&prepared.fixture_seed, index).map_err(|error| error.to_string())?;
    Ok(rf_generated_item(prepared, generated))
}

pub(super) fn generate_item(
    prepared: &PreparedGeneration,
    index: u32,
) -> Result<GeneratedIdentifier, String> {
    match prepared.kind {
        IdentifierKind::Iban => iban_item(prepared, index),
        IdentifierKind::Bic => bic_item(prepared, index, false),
        IdentifierKind::CreditorId => creditor_id_item(prepared, index),
        IdentifierKind::MandateReference => mandate_item(prepared, index),
        IdentifierKind::EndToEndId => end_to_end_item(prepared, index),
        IdentifierKind::RfReference => rf_item(prepared, index),
        IdentifierKind::Uetr => uetr_item(prepared, index),
        _ => Err("unsupported payment identifier kind".to_string()),
    }
}

fn rf_generated_item(
    prepared: &PreparedGeneration,
    generated: id_core::identifiers::payments::rf_reference::GeneratedRfReference,
) -> GeneratedIdentifier {
    let value = rendered_value(
        &generated.value,
        Some(&generated.formatted),
        prepared.format,
    );
    GeneratedIdentifier {
        value,
        formatted: Some(generated.formatted),
        kind: prepared.kind,
        profile: prepared.profile,
        synthetic: true,
        production_usable: false,
        checks: Checks {
            syntax: CheckStatus::Valid,
            checksum: CheckStatus::Valid,
            directory: CheckStatus::NotApplicable,
            assignment: CheckStatus::NotApplicable,
        },
        allocation_status: AllocationStatus::NotApplicable,
        account_existence: AccountExistenceStatus::NotApplicable,
        collision_guarantee: CollisionGuarantee::WithinBatch,
        parts: vec![
            IdentifierPart::new("check_digits", generated.parts.check_digits),
            IdentifierPart::new("reference_body", generated.parts.reference_body),
        ],
        reference_data: None,
        generator_version: id_core::GENERATOR_VERSION.to_string(),
        warnings: vec!["Synthetic structured payment reference.".to_string()],
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub(crate) struct BicGenerateRequest {
    #[serde(flatten)]
    pub request: GenerateRequest,
    /// Generate an 11-character BIC with branch identifier instead of BIC8.
    #[serde(default)]
    pub include_branch: bool,
}

type BicGeneratePayload = Result<Json<BicGenerateRequest>, JsonRejection>;

#[derive(Debug, Deserialize, ToSchema)]
pub(crate) struct RfGenerateRequest {
    #[serde(flatten)]
    pub request: GenerateRequest,
    /// Optional reference body, for example `NRG202600001234`. Explicit bodies
    /// are accepted only for a single-value request.
    pub invoice_reference: Option<String>,
}

type RfGeneratePayload = Result<Json<RfGenerateRequest>, JsonRejection>;

#[utoipa::path(
    post,
    path = "/api/v1/payments/accounts/iban/generate",
    operation_id = "paymentsIbanGenerate",
    request_body = GenerateRequest,
    responses(GenerateApiResponses),
    tag = "Zahlungsverkehr · Konten & Banken"
)]
pub(crate) async fn handle_iban_generate(
    payload: GeneratePayload,
) -> Result<Json<GenerateResponse>, ApiError> {
    generate_batch(IdentifierKind::Iban, payload, iban_item)
}

#[utoipa::path(
    post,
    path = "/api/v1/payments/institutions/bic/generate",
    operation_id = "paymentsBicGenerate",
    request_body = BicGenerateRequest,
    responses(GenerateApiResponses),
    tag = "Zahlungsverkehr · Konten & Banken"
)]
pub(crate) async fn handle_bic_generate(
    payload: BicGeneratePayload,
) -> Result<Json<GenerateResponse>, ApiError> {
    let Json(request) = payload.map_err(ApiError::invalid_generate_json)?;
    let include_branch = request.include_branch;
    let prepared = prepare_generation(IdentifierKind::Bic, request.request)?;
    generate_prepared_batch(prepared, |prepared, index| {
        bic_item(prepared, index, include_branch)
    })
}

#[utoipa::path(
    post,
    path = "/api/v1/payments/sepa/creditor-id/generate",
    operation_id = "paymentsCreditorIdGenerate",
    request_body = GenerateRequest,
    responses(GenerateApiResponses),
    tag = "Zahlungsverkehr · SEPA & Referenzen"
)]
pub(crate) async fn handle_creditor_id_generate(
    payload: GeneratePayload,
) -> Result<Json<GenerateResponse>, ApiError> {
    generate_batch(IdentifierKind::CreditorId, payload, creditor_id_item)
}

#[utoipa::path(
    post,
    path = "/api/v1/payments/sepa/mandate-reference/generate",
    operation_id = "paymentsMandateReferenceGenerate",
    request_body = GenerateRequest,
    responses(GenerateApiResponses),
    tag = "Zahlungsverkehr · SEPA & Referenzen"
)]
pub(crate) async fn handle_mandate_reference_generate(
    payload: GeneratePayload,
) -> Result<Json<GenerateResponse>, ApiError> {
    generate_batch(IdentifierKind::MandateReference, payload, mandate_item)
}

#[utoipa::path(
    post,
    path = "/api/v1/payments/sepa/end-to-end-id/generate",
    operation_id = "paymentsEndToEndIdGenerate",
    request_body = GenerateRequest,
    responses(GenerateApiResponses),
    tag = "Zahlungsverkehr · SEPA & Referenzen"
)]
pub(crate) async fn handle_end_to_end_id_generate(
    payload: GeneratePayload,
) -> Result<Json<GenerateResponse>, ApiError> {
    generate_batch(IdentifierKind::EndToEndId, payload, end_to_end_item)
}

#[utoipa::path(
    post,
    path = "/api/v1/payments/sepa/mandate-reference/validate",
    operation_id = "paymentsMandateReferenceValidate",
    request_body = ValidateRequest,
    responses(ValidationApiResponses),
    tag = "Zahlungsverkehr · SEPA & Referenzen"
)]
pub(crate) async fn handle_mandate_reference_validate(
    payload: ValidatePayload,
) -> Result<Json<ValidationReport>, ApiError> {
    let request = parse_validate_payload(payload)?;
    let input = request.id;
    let report = match validate_mandate_reference(&input) {
        Ok(reference) => ValidationReport {
            kind: IdentifierKind::MandateReference,
            input: input.clone(),
            normalized: Some(reference.value.clone()),
            valid: true,
            checks: Checks {
                syntax: CheckStatus::Valid,
                checksum: CheckStatus::NotApplicable,
                directory: CheckStatus::NotApplicable,
                assignment: CheckStatus::NotApplicable,
            },
            allocation_status: AllocationStatus::NotApplicable,
            synthetic: None,
            production_usable: None,
            parts: vec![IdentifierPart::new(
                "length",
                reference.value.chars().count().to_string(),
            )],
            reference_data: None,
            warnings: vec![
                "Format validation does not prove uniqueness within the creditor's mandates."
                    .to_string(),
            ],
            errors: Vec::new(),
        },
        Err(error) => invalid_non_allocated_report(
            IdentifierKind::MandateReference,
            input,
            error.to_string(),
            CheckStatus::NotApplicable,
        ),
    };
    Ok(Json(report))
}

#[utoipa::path(
    post,
    path = "/api/v1/payments/sepa/end-to-end-id/validate",
    operation_id = "paymentsEndToEndIdValidate",
    request_body = ValidateRequest,
    responses(ValidationApiResponses),
    tag = "Zahlungsverkehr · SEPA & Referenzen"
)]
pub(crate) async fn handle_end_to_end_id_validate(
    payload: ValidatePayload,
) -> Result<Json<ValidationReport>, ApiError> {
    let request = parse_validate_payload(payload)?;
    let input = request.id;
    let report = match validate_end_to_end_id(&input) {
        Ok(reference) => ValidationReport {
            kind: IdentifierKind::EndToEndId,
            input: input.clone(),
            normalized: Some(reference.value.clone()),
            valid: true,
            checks: Checks {
                syntax: CheckStatus::Valid,
                checksum: CheckStatus::NotApplicable,
                directory: CheckStatus::NotApplicable,
                assignment: CheckStatus::NotApplicable,
            },
            allocation_status: AllocationStatus::NotApplicable,
            synthetic: None,
            production_usable: None,
            parts: vec![
                IdentifierPart::new("length", reference.value.chars().count().to_string()),
                IdentifierPart::new("not_provided", reference.not_provided.to_string()),
            ],
            reference_data: None,
            warnings: if reference.not_provided {
                vec![
                    "NOTPROVIDED is the explicit SEPA sentinel and does not identify a concrete payment."
                        .to_string(),
                ]
            } else {
                vec![
                    "Format validation does not prove payment existence or uniqueness.".to_string(),
                ]
            },
            errors: Vec::new(),
        },
        Err(error) => invalid_non_allocated_report(
            IdentifierKind::EndToEndId,
            input,
            error.to_string(),
            CheckStatus::NotApplicable,
        ),
    };
    Ok(Json(report))
}

#[utoipa::path(
    post,
    path = "/api/v1/payments/sepa/uetr/generate",
    operation_id = "paymentsUetrGenerate",
    request_body = GenerateRequest,
    responses(GenerateApiResponses),
    tag = "Zahlungsverkehr · SEPA & Referenzen"
)]
pub(crate) async fn handle_uetr_generate(
    payload: GeneratePayload,
) -> Result<Json<GenerateResponse>, ApiError> {
    generate_batch(IdentifierKind::Uetr, payload, uetr_item)
}

#[utoipa::path(
    post,
    path = "/api/v1/payments/sepa/rf-reference/generate",
    operation_id = "paymentsRfReferenceGenerate",
    request_body = RfGenerateRequest,
    responses(GenerateApiResponses),
    tag = "Zahlungsverkehr · SEPA & Referenzen"
)]
pub(crate) async fn handle_rf_reference_generate(
    payload: RfGeneratePayload,
) -> Result<Json<GenerateResponse>, ApiError> {
    let Json(request) = payload.map_err(ApiError::invalid_generate_json)?;
    let prepared = prepare_generation(IdentifierKind::RfReference, request.request)?;
    let items = if let Some(reference_body) = request.invoice_reference {
        if prepared.count != 1 {
            return Err(ApiError::invalid_request(
                "invoice_reference requires count = 1".to_string(),
            ));
        }
        let generated = build_rf_reference(&reference_body)
            .map_err(|error| ApiError::invalid_request(error.to_string()))?;
        vec![rf_generated_item(&prepared, generated)]
    } else {
        let mut items = Vec::with_capacity(usize::from(prepared.count));
        for index in 0..u32::from(prepared.count) {
            items.push(rf_item(&prepared, index).map_err(|message| {
                ApiError::generation_failed_with_message("rf-reference", message)
            })?);
        }
        items
    };
    Ok(Json(GenerateResponse {
        generator_version: id_core::GENERATOR_VERSION.to_string(),
        fixture_seed: prepared.fixture_seed,
        items,
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/payments/accounts/iban/validate",
    operation_id = "paymentsIbanValidate",
    request_body = ValidateRequest,
    responses(ValidationApiResponses),
    tag = "Zahlungsverkehr · Konten & Banken"
)]
pub(crate) async fn handle_iban_validate(
    payload: ValidatePayload,
) -> Result<Json<ValidationReport>, ApiError> {
    let request = parse_validate_payload(payload)?;
    let input = request.id;
    let report = match validate_international_iban(&input) {
        Ok(parts) if parts.country_code == "DE" => {
            let directory = BundesbankBlzDirectory;
            match validate_german_iban_with_directory(&parts.electronic, &directory) {
                Ok(result) => {
                    let directory_status = match result.directory_status {
                        IbanDirectoryStatus::Found => CheckStatus::Found,
                        IbanDirectoryStatus::NotFound => CheckStatus::NotFound,
                    };
                    ValidationReport {
                        kind: IdentifierKind::Iban,
                        input: input.clone(),
                        normalized: Some(result.parts.electronic.clone()),
                        valid: true,
                        checks: Checks {
                            syntax: CheckStatus::Valid,
                            checksum: CheckStatus::Valid,
                            directory: directory_status,
                            assignment: CheckStatus::Unknown,
                        },
                        allocation_status: AllocationStatus::Unknown,
                        synthetic: None,
                        production_usable: None,
                        parts: vec![
                            IdentifierPart::new("country", result.parts.country_code),
                            IdentifierPart::new("check_digits", result.parts.check_digits),
                            IdentifierPart::new("bank_code", result.parts.bank_code),
                            IdentifierPart::new("account_number", result.parts.account_number),
                        ],
                        reference_data: Some(bundesbank_reference()),
                        warnings: vec![
                            format!("Country structure checked against SWIFT IBAN Registry release {IBAN_REGISTRY_RELEASE}."),
                            PAYMENT_VALIDATION_WARNING.to_string(),
                        ],
                        errors: Vec::new(),
                    }
                }
                Err(error) => invalid_report(
                    IdentifierKind::Iban,
                    input.clone(),
                    error.to_string(),
                    if matches!(error, IbanError::ChecksumMismatch) {
                        CheckStatus::Invalid
                    } else {
                        CheckStatus::NotChecked
                    },
                    CheckStatus::NotChecked,
                ),
            }
        }
        Ok(parts) => {
            let mut identifier_parts = vec![
                IdentifierPart::new("country", parts.country_code),
                IdentifierPart::new("country_name", parts.country_name),
                IdentifierPart::new("check_digits", parts.check_digits),
                IdentifierPart::new("bban", parts.bban),
                IdentifierPart::new("sepa", parts.sepa.to_string()),
            ];
            if let Some(bank_identifier) = parts.bank_identifier {
                identifier_parts.push(IdentifierPart::new("bank_identifier", bank_identifier));
            }
            if let Some(branch_identifier) = parts.branch_identifier {
                identifier_parts.push(IdentifierPart::new("branch_identifier", branch_identifier));
            }
            ValidationReport {
                kind: IdentifierKind::Iban,
                input: input.clone(),
                normalized: Some(parts.electronic),
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
                parts: identifier_parts,
                reference_data: Some(iban_registry_reference()),
                warnings: vec![PAYMENT_VALIDATION_WARNING.to_string()],
                errors: Vec::new(),
            }
        }
        Err(error) => {
            let checksum = if matches!(error, InternationalIbanError::ChecksumMismatch) {
                CheckStatus::Invalid
            } else {
                CheckStatus::NotChecked
            };
            let mut report = invalid_report(
                IdentifierKind::Iban,
                input,
                error.to_string(),
                checksum,
                CheckStatus::NotChecked,
            );
            report.reference_data = Some(iban_registry_reference());
            report
        }
    };
    Ok(Json(report))
}

#[utoipa::path(
    post,
    path = "/api/v1/payments/sepa/uetr/validate",
    operation_id = "paymentsUetrValidate",
    request_body = ValidateRequest,
    responses(ValidationApiResponses),
    tag = "Zahlungsverkehr · SEPA & Referenzen"
)]
pub(crate) async fn handle_uetr_validate(
    payload: ValidatePayload,
) -> Result<Json<ValidationReport>, ApiError> {
    let request = parse_validate_payload(payload)?;
    let input = request.id;
    let report = match validate_uetr(&input) {
        Ok(parts) => ValidationReport {
            kind: IdentifierKind::Uetr,
            input: input.clone(),
            normalized: Some(parts.canonical),
            valid: true,
            checks: Checks {
                syntax: CheckStatus::Valid,
                checksum: CheckStatus::NotApplicable,
                directory: CheckStatus::NotApplicable,
                assignment: CheckStatus::NotApplicable,
            },
            allocation_status: AllocationStatus::NotApplicable,
            synthetic: None,
            production_usable: None,
            parts: vec![
                IdentifierPart::new("uuid_version", parts.version.to_string()),
                IdentifierPart::new("uuid_variant", "rfc4122"),
            ],
            reference_data: None,
            warnings: Vec::new(),
            errors: Vec::new(),
        },
        Err(error) => invalid_non_allocated_report(
            IdentifierKind::Uetr,
            input,
            error.to_string(),
            CheckStatus::NotApplicable,
        ),
    };
    Ok(Json(report))
}

#[utoipa::path(
    post,
    path = "/api/v1/payments/institutions/bic/validate",
    operation_id = "paymentsBicValidate",
    request_body = ValidateRequest,
    responses(ValidationApiResponses),
    tag = "Zahlungsverkehr · Konten & Banken"
)]
pub(crate) async fn handle_bic_validate(
    payload: ValidatePayload,
) -> Result<Json<ValidationReport>, ApiError> {
    let request = parse_validate_payload(payload)?;
    let input = request.id;
    let report = match validate_bic(&input) {
        Ok(parts) => {
            let directory_checked = parts.country_code == "DE";
            let directory_hit = directory_checked
                && BundesbankBlzDirectory.records().any(|record| {
                    record.bic.is_some_and(|directory_bic| {
                        if parts.electronic.len() == 8 {
                            directory_bic.get(..8) == Some(parts.electronic.as_str())
                        } else {
                            directory_bic == parts.electronic
                        }
                    })
                });
            let mut identifier_parts = vec![
                IdentifierPart::new("business_party_prefix", parts.business_party_prefix),
                IdentifierPart::new("country", parts.country_code),
                IdentifierPart::new("location", parts.location_code),
                IdentifierPart::new(
                    "test_training_pattern",
                    parts.test_training_pattern.to_string(),
                ),
            ];
            if let Some(branch) = parts.branch_code {
                identifier_parts.push(IdentifierPart::new("branch", branch));
            }
            ValidationReport {
                kind: IdentifierKind::Bic,
                input: input.clone(),
                normalized: Some(parts.electronic.clone()),
                valid: true,
                checks: Checks {
                    syntax: CheckStatus::Valid,
                    checksum: CheckStatus::NotApplicable,
                    directory: match (directory_checked, directory_hit) {
                        (_, true) => CheckStatus::Found,
                        (true, false) => CheckStatus::NotFound,
                        (false, false) => CheckStatus::NotChecked,
                    },
                    assignment: CheckStatus::Unknown,
                },
                allocation_status: AllocationStatus::Unknown,
                synthetic: None,
                production_usable: None,
                parts: identifier_parts,
                reference_data: directory_checked.then(bundesbank_reference),
                warnings: vec![
                    "Bundesbank directory presence is not proof of current SWIFT registration."
                        .to_string(),
                ],
                errors: Vec::new(),
            }
        }
        Err(error) => invalid_report(
            IdentifierKind::Bic,
            input,
            error.to_string(),
            CheckStatus::NotApplicable,
            CheckStatus::NotChecked,
        ),
    };
    Ok(Json(report))
}

#[utoipa::path(
    post,
    path = "/api/v1/payments/sepa/creditor-id/validate",
    operation_id = "paymentsCreditorIdValidate",
    request_body = ValidateRequest,
    responses(ValidationApiResponses),
    tag = "Zahlungsverkehr · SEPA & Referenzen"
)]
pub(crate) async fn handle_creditor_id_validate(
    payload: ValidatePayload,
) -> Result<Json<ValidationReport>, ApiError> {
    let request = parse_validate_payload(payload)?;
    let input = request.id;
    let report = match validate_german_creditor_id(&input) {
        Ok(parts) => ValidationReport {
            kind: IdentifierKind::CreditorId,
            input: input.clone(),
            normalized: Some(parts.electronic.clone()),
            valid: true,
            checks: Checks {
                syntax: CheckStatus::Valid,
                checksum: CheckStatus::Valid,
                directory: CheckStatus::NotApplicable,
                assignment: if parts.official_test_fixture {
                    CheckStatus::NotApplicable
                } else {
                    CheckStatus::Unknown
                },
            },
            allocation_status: if parts.official_test_fixture {
                AllocationStatus::NotApplicable
            } else {
                AllocationStatus::Unknown
            },
            synthetic: parts.official_test_fixture.then_some(true),
            production_usable: parts.official_test_fixture.then_some(false),
            parts: vec![
                IdentifierPart::new("country", parts.country_code),
                IdentifierPart::new("check_digits", parts.check_digits),
                IdentifierPart::new("business_code", parts.business_code),
                IdentifierPart::new("national_identifier", parts.national_identifier),
                IdentifierPart::new(
                    "official_test_fixture",
                    parts.official_test_fixture.to_string(),
                ),
            ],
            reference_data: Some(creditor_id_reference()),
            warnings: if parts.official_test_fixture {
                vec!["Official Bundesbank test fixture.".to_string()]
            } else {
                vec!["Allocation has not been checked.".to_string()]
            },
            errors: Vec::new(),
        },
        Err(error) => {
            let checksum = if matches!(error, CreditorIdError::ChecksumMismatch) {
                CheckStatus::Invalid
            } else {
                CheckStatus::NotChecked
            };
            invalid_report(
                IdentifierKind::CreditorId,
                input,
                error.to_string(),
                checksum,
                CheckStatus::NotApplicable,
            )
        }
    };
    Ok(Json(report))
}

#[utoipa::path(
    post,
    path = "/api/v1/payments/sepa/rf-reference/validate",
    operation_id = "paymentsRfReferenceValidate",
    request_body = ValidateRequest,
    responses(ValidationApiResponses),
    tag = "Zahlungsverkehr · SEPA & Referenzen"
)]
pub(crate) async fn handle_rf_reference_validate(
    payload: ValidatePayload,
) -> Result<Json<ValidationReport>, ApiError> {
    let request = parse_validate_payload(payload)?;
    let input = request.id;
    let report = match validate_rf_reference(&input) {
        Ok(parts) => ValidationReport {
            kind: IdentifierKind::RfReference,
            input: input.clone(),
            normalized: Some(parts.electronic.clone()),
            valid: true,
            checks: Checks {
                syntax: CheckStatus::Valid,
                checksum: CheckStatus::Valid,
                directory: CheckStatus::NotApplicable,
                assignment: CheckStatus::NotApplicable,
            },
            allocation_status: AllocationStatus::NotApplicable,
            synthetic: None,
            production_usable: None,
            parts: vec![
                IdentifierPart::new("check_digits", parts.check_digits),
                IdentifierPart::new("reference_body", parts.reference_body),
            ],
            reference_data: None,
            warnings: Vec::new(),
            errors: Vec::new(),
        },
        Err(error) => {
            let checksum = if matches!(error, RfReferenceError::ChecksumMismatch) {
                CheckStatus::Invalid
            } else {
                CheckStatus::NotChecked
            };
            invalid_non_allocated_report(
                IdentifierKind::RfReference,
                input,
                error.to_string(),
                checksum,
            )
        }
    };
    Ok(Json(report))
}
