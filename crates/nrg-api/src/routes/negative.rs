//! Explicit negative-fixture endpoints for generated identifier kinds.
//!
//! A negative fixture is always derived from a valid, reproducible base value.
//! The requested defect is then injected and checked with the same core
//! validator that backs the public validation endpoint.  This keeps malformed
//! test data intentional and prevents a mutation from accidentally remaining
//! valid.

use axum::{extract::rejection::JsonRejection, Json};
use id_core::{
    catalog::{
        AccountExistenceStatus, AllocationStatus, CheckStatus, Checks, CollisionGuarantee,
        GenerateRequest, GeneratedIdentifier, GenerationProfile, IdentifierFormat, IdentifierKind,
        IdentifierPart, Sector,
    },
    identifiers::{
        business::{lei::validate_lei, vat_id::validate_german_vat_id},
        energy::{
            validate_cr_id, validate_market_partner_id, validate_nebe_id, validate_package_id,
            validate_sg_id, validate_sr_id, validate_tr_id,
        },
        metering::{validate_din_43849, validate_obis},
        payments::{
            bic::validate_bic, creditor_id::validate_german_creditor_id,
            end_to_end_id::validate_end_to_end_id, international_iban::validate_international_iban,
            mandate_reference::validate_mandate_reference, rf_reference::validate_rf_reference,
            uetr::validate_uetr,
        },
        registers::{validate_eic, validate_mastr},
    },
    validate_malo, validate_melo, validate_nelo, GENERATOR_VERSION,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{generate_identifier_item, prepare_generation};
use crate::{ApiError, ErrorResponse};

/// Deliberate fault to inject into an otherwise valid fixture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum NegativeMutation {
    /// Change the value's length so it no longer matches the identifier rules.
    Length,
    /// Insert a non-ASCII character outside every supported identifier alphabet.
    CharacterSet,
    /// Change a standardized check digit without changing the payload.
    Checksum,
}

/// Options shared by all explicit negative-fixture endpoints.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, ToSchema)]
pub(crate) struct NegativeFixtureRequest {
    pub mutation: NegativeMutation,
    /// Reproducible seed for the valid base value.
    #[serde(default)]
    pub fixture_seed: Option<String>,
    /// Optional generation profile for the valid base value.
    #[serde(default)]
    pub profile: Option<GenerationProfile>,
    /// Optional ISO 3166 country selector, currently used by IBAN.
    #[serde(default)]
    pub country: Option<String>,
    /// Optional market sector, currently used by BDEW/DVGW MP-ID generation.
    #[serde(default)]
    pub sector: Option<Sector>,
}

/// A verified invalid value together with the valid fixture it was derived from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub(crate) struct NegativeFixtureResponse {
    pub generator_version: String,
    pub fixture_seed: String,
    pub kind: IdentifierKind,
    pub mutation: NegativeMutation,
    pub original: GeneratedIdentifier,
    pub mutated_value: String,
    /// Negative fixtures are expected to fail the matching validator.
    pub expected_valid: bool,
    /// Always true for a successful response; generation fails closed otherwise.
    pub validator_rejected: bool,
}

pub(crate) type NegativeFixturePayload = Result<Json<NegativeFixtureRequest>, JsonRejection>;

// Documentation-only enum; it is never instantiated at runtime.
#[allow(dead_code, clippy::large_enum_variant)]
#[derive(utoipa::IntoResponses)]
pub(crate) enum NegativeFixtureApiResponses {
    /// Verified malformed fixture and its valid source value.
    #[response(status = 200)]
    Success(NegativeFixtureResponse),
    /// Malformed JSON request body.
    #[response(status = 400)]
    MalformedJson(ErrorResponse),
    /// Request body exceeds the configured limit.
    #[response(status = 413)]
    PayloadTooLarge(ErrorResponse),
    /// Content-Type is not application/json.
    #[response(status = 415)]
    UnsupportedMediaType(ErrorResponse),
    /// Invalid options or a mutation which does not apply to this identifier.
    #[response(status = 422)]
    InvalidOptions(ErrorResponse),
    /// The generated source or mutated result violated an internal invariant.
    #[response(status = 500)]
    InternalInvariant(ErrorResponse),
}

fn parse_payload(payload: NegativeFixturePayload) -> Result<NegativeFixtureRequest, ApiError> {
    payload
        .map(|Json(request)| request)
        .map_err(ApiError::invalid_generate_json)
}

fn build_negative_fixture(
    kind: IdentifierKind,
    request: NegativeFixtureRequest,
) -> Result<NegativeFixtureResponse, ApiError> {
    if request.mutation == NegativeMutation::Checksum && !has_checksum(kind) {
        return Err(ApiError::invalid_request(format!(
            "Mutation 'checksum' is not applicable to {} because it has no standardized checksum",
            kind.as_str()
        )));
    }

    let mutation = request.mutation;
    let (fixture_seed, original) = if is_validator_only(kind) {
        validator_only_source(kind, &request)?
    } else {
        let prepared = prepare_generation(
            kind,
            GenerateRequest {
                profile: request.profile,
                count: 1,
                fixture_seed: request.fixture_seed,
                // Negative fixtures always mutate the canonical representation.
                format: IdentifierFormat::Electronic,
                sector: request.sector,
                country: request.country,
            },
        )?;
        let fixture_seed = prepared.fixture_seed.clone();
        let original = generate_identifier_item(&prepared, 0)
            .map_err(|detail| ApiError::generation_failed_with_message(kind.as_str(), detail))?;
        (fixture_seed, original)
    };

    if !validator_accepts(kind, &original.value)? {
        return Err(ApiError::generation_failed_with_message(
            kind.as_str(),
            "negative-fixture source was not accepted by its validator".to_string(),
        ));
    }

    let mut mutated_value = mutate(kind, mutation, &original.value)?;
    if validator_accepts(kind, &mutated_value)? {
        // A length mutation can very occasionally leave a variable-length,
        // checksum-bearing reference valid. Empty is invalid for every
        // supported identifier while preserving the requested fault category.
        if mutation == NegativeMutation::Length {
            mutated_value.clear();
        }
    }
    if validator_accepts(kind, &mutated_value)? {
        return Err(ApiError::generation_failed_with_message(
            kind.as_str(),
            format!(
                "the '{}' mutation was unexpectedly accepted by the validator",
                mutation.as_str()
            ),
        ));
    }

    Ok(NegativeFixtureResponse {
        generator_version: GENERATOR_VERSION.to_string(),
        fixture_seed,
        kind,
        mutation,
        original,
        mutated_value,
        expected_valid: false,
        validator_rejected: true,
    })
}

fn is_validator_only(kind: IdentifierKind) -> bool {
    matches!(
        kind,
        IdentifierKind::VatId
            | IdentifierKind::Lei
            | IdentifierKind::Eic
            | IdentifierKind::Obis
            | IdentifierKind::Din43849
    )
}

/// Returns a fixed, reviewed source value for negative-fixture derivation.
///
/// These identifiers deliberately have no public generator.  Keeping their
/// baseline values here avoids turning a negative-test convenience endpoint
/// into an unqualified generator for centrally allocated identifiers.
fn validator_only_source(
    kind: IdentifierKind,
    request: &NegativeFixtureRequest,
) -> Result<(String, GeneratedIdentifier), ApiError> {
    if request.profile.is_some() || request.country.is_some() || request.sector.is_some() {
        return Err(ApiError::invalid_request(format!(
            "{} negative fixtures use a fixed reviewed source; profile, country and sector are not applicable",
            kind.as_str()
        )));
    }

    let fixture_seed = request
        .fixture_seed
        .clone()
        .unwrap_or_else(|| "reviewed-validator-fixture-v1".to_string());
    let (value, profile, synthetic, checks, allocation_status, parts, warning) = match kind {
        IdentifierKind::VatId => (
            "DE000000000",
            GenerationProfile::SyntaxOnly,
            true,
            Checks {
                syntax: CheckStatus::Valid,
                checksum: CheckStatus::NotApplicable,
                directory: CheckStatus::NotChecked,
                assignment: CheckStatus::Unknown,
            },
            AllocationStatus::Unknown,
            vec![
                IdentifierPart::new("country", "DE"),
                IdentifierPart::new("national_identifier", "000000000"),
            ],
            "Constructed format-only source; BZSt/VIES assignment and validity are not checked.",
        ),
        IdentifierKind::Lei => (
            "506700GE1G29325QX363",
            GenerationProfile::ChecksumOnly,
            false,
            Checks {
                syntax: CheckStatus::Valid,
                checksum: CheckStatus::Valid,
                directory: CheckStatus::NotChecked,
                assignment: CheckStatus::Unknown,
            },
            AllocationStatus::Unknown,
            vec![
                IdentifierPart::new("issuer_prefix", "5067"),
                IdentifierPart::new("check_digits", "63"),
            ],
            "Published GLEIF reference value; registry status is not asserted by this fixture endpoint.",
        ),
        IdentifierKind::Eic => (
            "11Y123456789012T",
            GenerationProfile::ChecksumOnly,
            false,
            Checks {
                syntax: CheckStatus::Valid,
                checksum: CheckStatus::Valid,
                directory: CheckStatus::NotChecked,
                assignment: CheckStatus::Unknown,
            },
            AllocationStatus::Unknown,
            vec![
                IdentifierPart::new("lio_code", "11"),
                IdentifierPart::new("object_type_code", "Y"),
                IdentifierPart::new("check_character", "T"),
            ],
            "Published EIC example; current allocation is not asserted by this fixture endpoint.",
        ),
        IdentifierKind::Obis => (
            "1-0:1.8.0*255",
            GenerationProfile::SyntaxOnly,
            false,
            Checks {
                syntax: CheckStatus::Valid,
                checksum: CheckStatus::NotApplicable,
                directory: CheckStatus::NotChecked,
                assignment: CheckStatus::NotApplicable,
            },
            AllocationStatus::NotApplicable,
            vec![IdentifierPart::new("notation", "display")],
            "Standard structural OBIS value; catalog membership is independent of syntax validation.",
        ),
        IdentifierKind::Din43849 => (
            "7QDS0111223344",
            GenerationProfile::SyntaxOnly,
            false,
            Checks {
                syntax: CheckStatus::Valid,
                checksum: CheckStatus::NotApplicable,
                directory: CheckStatus::NotChecked,
                assignment: CheckStatus::Unknown,
            },
            AllocationStatus::Unknown,
            vec![IdentifierPart::new("manufacturer_id", "QDS")],
            "Published OMS structure example; manufacturer registration and device existence are not checked.",
        ),
        _ => {
            return Err(ApiError::invalid_request(format!(
                "{} is not a validator-only fixture kind",
                kind.as_str()
            )))
        }
    };

    Ok((
        fixture_seed,
        GeneratedIdentifier {
            value: value.to_string(),
            formatted: None,
            kind,
            profile,
            synthetic,
            production_usable: false,
            checks,
            allocation_status,
            account_existence: AccountExistenceStatus::NotApplicable,
            collision_guarantee: CollisionGuarantee::None,
            parts,
            reference_data: None,
            generator_version: GENERATOR_VERSION.to_string(),
            warnings: vec![
                warning.to_string(),
                "This source exists only to derive a deliberately invalid fixture; it is not a public identifier generator."
                    .to_string(),
            ],
        },
    ))
}

impl NegativeMutation {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Length => "length",
            Self::CharacterSet => "character_set",
            Self::Checksum => "checksum",
        }
    }
}

fn has_checksum(kind: IdentifierKind) -> bool {
    matches!(
        kind,
        IdentifierKind::Malo
            | IdentifierKind::Nelo
            | IdentifierKind::Nebe
            | IdentifierKind::Iban
            | IdentifierKind::CreditorId
            | IdentifierKind::RfReference
            | IdentifierKind::Mastr
            | IdentifierKind::Lei
            | IdentifierKind::Eic
            | IdentifierKind::MarketPartnerId
            | IdentifierKind::ClusterResourceId
            | IdentifierKind::SteeringGroupId
            | IdentifierKind::ControllableResourceId
            | IdentifierKind::TechnicalResourceId
            | IdentifierKind::PackageId
    )
}

fn mutate(
    kind: IdentifierKind,
    mutation: NegativeMutation,
    original: &str,
) -> Result<String, ApiError> {
    match mutation {
        NegativeMutation::Length => {
            if matches!(
                kind,
                IdentifierKind::MandateReference | IdentifierKind::EndToEndId
            ) {
                let mut value = original.to_string();
                while value.len() <= 35 {
                    value.push('X');
                }
                Ok(value)
            } else {
                let mut value = original.to_string();
                value.pop();
                Ok(value)
            }
        }
        NegativeMutation::CharacterSet => {
            let tail = original.chars().skip(1).collect::<String>();
            Ok(format!("Ä{tail}"))
        }
        NegativeMutation::Checksum => mutate_checksum(kind, original),
    }
}

fn mutate_checksum(kind: IdentifierKind, original: &str) -> Result<String, ApiError> {
    let offset = match kind {
        IdentifierKind::Iban | IdentifierKind::CreditorId | IdentifierKind::RfReference => 2,
        // MaStR: 3-character prefix + 11-digit numeric base + check digit.
        // A possible role suffix follows the check digit.
        IdentifierKind::Mastr => 14,
        // LEI checksum occupies the final two numeric positions; mutating the
        // first digit retains valid syntax while invalidating MOD-97.
        IdentifierKind::Lei => 18,
        // The EIC check character is the final alphanumeric character.
        IdentifierKind::Eic => original.len().saturating_sub(1),
        IdentifierKind::Malo
        | IdentifierKind::Nelo
        | IdentifierKind::Nebe
        | IdentifierKind::MarketPartnerId
        | IdentifierKind::ClusterResourceId
        | IdentifierKind::SteeringGroupId
        | IdentifierKind::ControllableResourceId
        | IdentifierKind::TechnicalResourceId
        | IdentifierKind::PackageId => original.len().saturating_sub(1),
        _ => {
            return Err(ApiError::invalid_request(format!(
            "Mutation 'checksum' is not applicable to {} because it has no standardized checksum",
            kind.as_str()
        )))
        }
    };

    let mut bytes = original.as_bytes().to_vec();
    let byte = bytes.get_mut(offset).ok_or_else(|| {
        ApiError::generation_failed_with_message(
            kind.as_str(),
            "generated value does not contain the expected checksum position".to_string(),
        )
    })?;
    if kind != IdentifierKind::Eic && !byte.is_ascii_digit() {
        return Err(ApiError::generation_failed_with_message(
            kind.as_str(),
            "generated checksum character is not numeric".to_string(),
        ));
    }
    *byte = if *byte == b'0' { b'1' } else { b'0' };
    String::from_utf8(bytes).map_err(|_| {
        ApiError::generation_failed_with_message(
            kind.as_str(),
            "generated value stopped being UTF-8 after checksum mutation".to_string(),
        )
    })
}

fn validator_accepts(kind: IdentifierKind, value: &str) -> Result<bool, ApiError> {
    let valid = match kind {
        IdentifierKind::Malo => validate_malo(value).is_ok(),
        IdentifierKind::Melo => validate_melo(value).is_ok(),
        IdentifierKind::Nelo => validate_nelo(value).is_ok(),
        IdentifierKind::Nebe => validate_nebe_id(value).is_ok(),
        IdentifierKind::Iban => validate_international_iban(value).is_ok(),
        IdentifierKind::Bic => validate_bic(value).is_ok(),
        IdentifierKind::CreditorId => validate_german_creditor_id(value).is_ok(),
        IdentifierKind::MandateReference => validate_mandate_reference(value).is_ok(),
        IdentifierKind::EndToEndId => validate_end_to_end_id(value).is_ok(),
        IdentifierKind::RfReference => validate_rf_reference(value).is_ok(),
        IdentifierKind::Uetr => validate_uetr(value).is_ok(),
        IdentifierKind::Mastr => validate_mastr(value).is_ok(),
        IdentifierKind::VatId => validate_german_vat_id(value).is_ok(),
        IdentifierKind::Lei => validate_lei(value).is_ok(),
        IdentifierKind::Eic => validate_eic(value).is_ok(),
        IdentifierKind::Obis => validate_obis(value).is_ok(),
        IdentifierKind::Din43849 => validate_din_43849(value).is_ok(),
        IdentifierKind::MarketPartnerId => validate_market_partner_id(value).is_ok(),
        IdentifierKind::ClusterResourceId => validate_cr_id(value).is_ok(),
        IdentifierKind::SteeringGroupId => validate_sg_id(value).is_ok(),
        IdentifierKind::ControllableResourceId => validate_sr_id(value).is_ok(),
        IdentifierKind::TechnicalResourceId => validate_tr_id(value).is_ok(),
        IdentifierKind::PackageId => validate_package_id(value).is_ok(),
    };
    Ok(valid)
}

fn generate_negative(
    kind: IdentifierKind,
    payload: NegativeFixturePayload,
) -> Result<Json<NegativeFixtureResponse>, ApiError> {
    let request = parse_payload(payload)?;
    build_negative_fixture(kind, request).map(Json)
}

macro_rules! negative_fixture_handler {
    ($name:ident, $path:literal, $operation_id:literal, $kind:expr) => {
        #[utoipa::path(
                                                            post,
                                                            path = $path,
                                                            operation_id = $operation_id,
                                                            request_body = NegativeFixtureRequest,
                                                            responses(NegativeFixtureApiResponses),
                                                            tag = "Testdaten · Szenarien"
                                                        )]
        pub(crate) async fn $name(
            payload: NegativeFixturePayload,
        ) -> Result<Json<NegativeFixtureResponse>, ApiError> {
            generate_negative($kind, payload)
        }
    };
}

negative_fixture_handler!(
    handle_malo_negative_generate,
    "/api/v1/test-data/negative/malo/generate",
    "testDataNegativeMaloGenerate",
    IdentifierKind::Malo
);
negative_fixture_handler!(
    handle_melo_negative_generate,
    "/api/v1/test-data/negative/melo/generate",
    "testDataNegativeMeloGenerate",
    IdentifierKind::Melo
);
negative_fixture_handler!(
    handle_nelo_negative_generate,
    "/api/v1/test-data/negative/nelo/generate",
    "testDataNegativeNeloGenerate",
    IdentifierKind::Nelo
);
negative_fixture_handler!(
    handle_nebe_negative_generate,
    "/api/v1/test-data/negative/nebe/generate",
    "testDataNegativeNebeGenerate",
    IdentifierKind::Nebe
);
negative_fixture_handler!(
    handle_market_partner_negative_generate,
    "/api/v1/test-data/negative/mp-id/generate",
    "testDataNegativeMarketPartnerIdGenerate",
    IdentifierKind::MarketPartnerId
);
negative_fixture_handler!(
    handle_cr_negative_generate,
    "/api/v1/test-data/negative/cr-id/generate",
    "testDataNegativeClusterResourceIdGenerate",
    IdentifierKind::ClusterResourceId
);
negative_fixture_handler!(
    handle_sg_negative_generate,
    "/api/v1/test-data/negative/sg-id/generate",
    "testDataNegativeSteeringGroupIdGenerate",
    IdentifierKind::SteeringGroupId
);
negative_fixture_handler!(
    handle_sr_negative_generate,
    "/api/v1/test-data/negative/sr-id/generate",
    "testDataNegativeControllableResourceIdGenerate",
    IdentifierKind::ControllableResourceId
);
negative_fixture_handler!(
    handle_tr_negative_generate,
    "/api/v1/test-data/negative/tr-id/generate",
    "testDataNegativeTechnicalResourceIdGenerate",
    IdentifierKind::TechnicalResourceId
);
negative_fixture_handler!(
    handle_package_negative_generate,
    "/api/v1/test-data/negative/package-id/generate",
    "testDataNegativePackageIdGenerate",
    IdentifierKind::PackageId
);
negative_fixture_handler!(
    handle_iban_negative_generate,
    "/api/v1/test-data/negative/iban/generate",
    "testDataNegativeIbanGenerate",
    IdentifierKind::Iban
);
negative_fixture_handler!(
    handle_bic_negative_generate,
    "/api/v1/test-data/negative/bic/generate",
    "testDataNegativeBicGenerate",
    IdentifierKind::Bic
);
negative_fixture_handler!(
    handle_creditor_id_negative_generate,
    "/api/v1/test-data/negative/creditor-id/generate",
    "testDataNegativeCreditorIdGenerate",
    IdentifierKind::CreditorId
);
negative_fixture_handler!(
    handle_mandate_reference_negative_generate,
    "/api/v1/test-data/negative/mandate-reference/generate",
    "testDataNegativeMandateReferenceGenerate",
    IdentifierKind::MandateReference
);
negative_fixture_handler!(
    handle_end_to_end_id_negative_generate,
    "/api/v1/test-data/negative/end-to-end-id/generate",
    "testDataNegativeEndToEndIdGenerate",
    IdentifierKind::EndToEndId
);
negative_fixture_handler!(
    handle_rf_reference_negative_generate,
    "/api/v1/test-data/negative/rf-reference/generate",
    "testDataNegativeRfReferenceGenerate",
    IdentifierKind::RfReference
);
negative_fixture_handler!(
    handle_uetr_negative_generate,
    "/api/v1/test-data/negative/uetr/generate",
    "testDataNegativeUetrGenerate",
    IdentifierKind::Uetr
);
negative_fixture_handler!(
    handle_mastr_negative_generate,
    "/api/v1/test-data/negative/mastr/generate",
    "testDataNegativeMastrGenerate",
    IdentifierKind::Mastr
);
negative_fixture_handler!(
    handle_vat_id_negative_generate,
    "/api/v1/test-data/negative/vat-id/generate",
    "testDataNegativeVatIdGenerate",
    IdentifierKind::VatId
);
negative_fixture_handler!(
    handle_lei_negative_generate,
    "/api/v1/test-data/negative/lei/generate",
    "testDataNegativeLeiGenerate",
    IdentifierKind::Lei
);
negative_fixture_handler!(
    handle_eic_negative_generate,
    "/api/v1/test-data/negative/eic/generate",
    "testDataNegativeEicGenerate",
    IdentifierKind::Eic
);
negative_fixture_handler!(
    handle_obis_negative_generate,
    "/api/v1/test-data/negative/obis/generate",
    "testDataNegativeObisGenerate",
    IdentifierKind::Obis
);
negative_fixture_handler!(
    handle_din_43849_negative_generate,
    "/api/v1/test-data/negative/din-43849/generate",
    "testDataNegativeDin43849Generate",
    IdentifierKind::Din43849
);

#[cfg(test)]
mod tests {
    use super::*;

    const GENERATABLE_KINDS: &[IdentifierKind] = &[
        IdentifierKind::Malo,
        IdentifierKind::Melo,
        IdentifierKind::Nelo,
        IdentifierKind::Nebe,
        IdentifierKind::MarketPartnerId,
        IdentifierKind::ClusterResourceId,
        IdentifierKind::SteeringGroupId,
        IdentifierKind::ControllableResourceId,
        IdentifierKind::TechnicalResourceId,
        IdentifierKind::PackageId,
        IdentifierKind::Iban,
        IdentifierKind::Bic,
        IdentifierKind::CreditorId,
        IdentifierKind::MandateReference,
        IdentifierKind::EndToEndId,
        IdentifierKind::RfReference,
        IdentifierKind::Uetr,
        IdentifierKind::Mastr,
    ];

    const CHECKSUM_KINDS: &[IdentifierKind] = &[
        IdentifierKind::Malo,
        IdentifierKind::Nelo,
        IdentifierKind::Nebe,
        IdentifierKind::MarketPartnerId,
        IdentifierKind::ClusterResourceId,
        IdentifierKind::SteeringGroupId,
        IdentifierKind::ControllableResourceId,
        IdentifierKind::TechnicalResourceId,
        IdentifierKind::PackageId,
        IdentifierKind::Iban,
        IdentifierKind::CreditorId,
        IdentifierKind::RfReference,
        IdentifierKind::Mastr,
        IdentifierKind::Lei,
        IdentifierKind::Eic,
    ];

    const VALIDATOR_ONLY_KINDS: &[IdentifierKind] = &[
        IdentifierKind::VatId,
        IdentifierKind::Lei,
        IdentifierKind::Eic,
        IdentifierKind::Obis,
        IdentifierKind::Din43849,
    ];

    fn request(mutation: NegativeMutation) -> NegativeFixtureRequest {
        NegativeFixtureRequest {
            mutation,
            fixture_seed: Some("negative-fixture-test".to_string()),
            profile: None,
            country: None,
            sector: None,
        }
    }

    #[test]
    fn every_generatable_kind_yields_verified_length_and_character_set_fixtures() {
        for kind in GENERATABLE_KINDS {
            for mutation in [NegativeMutation::Length, NegativeMutation::CharacterSet] {
                let response = build_negative_fixture(*kind, request(mutation))
                    .unwrap_or_else(|_| panic!("{kind:?} {mutation:?} must generate"));
                assert!(!response.expected_valid);
                assert!(response.validator_rejected);
                assert!(validator_accepts(*kind, &response.original.value).unwrap());
                assert!(!validator_accepts(*kind, &response.mutated_value).unwrap());
            }
        }
    }

    #[test]
    fn checksum_fixtures_are_rejected_and_reproducible() {
        for kind in CHECKSUM_KINDS {
            let first = build_negative_fixture(*kind, request(NegativeMutation::Checksum))
                .unwrap_or_else(|_| panic!("{kind:?} checksum fixture must generate"));
            let second = build_negative_fixture(*kind, request(NegativeMutation::Checksum))
                .unwrap_or_else(|_| panic!("{kind:?} checksum fixture must reproduce"));
            assert_eq!(first.original.value, second.original.value);
            assert_eq!(first.mutated_value, second.mutated_value);
            assert!(!validator_accepts(*kind, &first.mutated_value).unwrap());
        }
    }

    #[test]
    fn checksum_mutation_is_rejected_for_checksumless_identifiers() {
        for kind in [
            IdentifierKind::Melo,
            IdentifierKind::Bic,
            IdentifierKind::MandateReference,
            IdentifierKind::EndToEndId,
            IdentifierKind::Uetr,
            IdentifierKind::VatId,
            IdentifierKind::Obis,
            IdentifierKind::Din43849,
        ] {
            assert!(build_negative_fixture(kind, request(NegativeMutation::Checksum)).is_err());
        }
    }

    #[test]
    fn validator_only_kinds_yield_verified_applicable_fixtures_without_becoming_generators() {
        for kind in VALIDATOR_ONLY_KINDS {
            for mutation in [NegativeMutation::Length, NegativeMutation::CharacterSet] {
                let response = build_negative_fixture(*kind, request(mutation))
                    .unwrap_or_else(|_| panic!("{kind:?} {mutation:?} must generate"));
                assert!(!response.expected_valid);
                assert!(response.validator_rejected);
                assert!(validator_accepts(*kind, &response.original.value).unwrap());
                assert!(!validator_accepts(*kind, &response.mutated_value).unwrap());
                assert!(!response.original.production_usable);
                assert!(response
                    .original
                    .warnings
                    .iter()
                    .any(|warning| { warning.contains("not a public identifier generator") }));
            }
        }
    }

    #[test]
    fn validator_only_checksum_fixtures_apply_only_to_lei_and_eic() {
        for kind in [IdentifierKind::Lei, IdentifierKind::Eic] {
            let response = build_negative_fixture(kind, request(NegativeMutation::Checksum))
                .unwrap_or_else(|_| panic!("{kind:?} checksum fixture must generate"));
            assert!(validator_accepts(kind, &response.original.value).unwrap());
            assert!(!validator_accepts(kind, &response.mutated_value).unwrap());
        }
    }
}
