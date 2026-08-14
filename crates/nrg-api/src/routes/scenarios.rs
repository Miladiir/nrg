use axum::{extract::rejection::JsonRejection, Json};
use id_core::{
    catalog::{
        descriptor, AccountExistenceStatus, AllocationStatus, CheckStatus, Checks,
        CollisionGuarantee, GenerateRequest, GeneratedIdentifier, GenerationProfile,
        IdentifierFormat, IdentifierKind, IdentifierPart, MarketRole, ReferenceData, Sector,
    },
    identifiers::payments::bic::validate_bic,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{generate_identifier_item, prepare_generation};
use crate::{ApiError, ErrorResponse};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ScenarioKind {
    SupplierBasic,
    SupplierDirectDebit,
    GridOperatorElectricity,
    GridOperatorGas,
    MeteringPointOperator,
    RedispatchResourceBundle,
}

impl ScenarioKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::SupplierBasic => "supplier_basic",
            Self::SupplierDirectDebit => "supplier_direct_debit",
            Self::GridOperatorElectricity => "grid_operator_electricity",
            Self::GridOperatorGas => "grid_operator_gas",
            Self::MeteringPointOperator => "metering_point_operator",
            Self::RedispatchResourceBundle => "redispatch_resource_bundle",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ScenarioRequirement {
    kind: IdentifierKind,
    default_profile: GenerationProfile,
    depends_on: &'static [IdentifierKind],
}

#[derive(Debug, Clone, Copy)]
struct ScenarioDefinition {
    scenario: ScenarioKind,
    label: &'static str,
    description: &'static str,
    role: MarketRole,
    sectors: &'static [Sector],
    requirements: &'static [ScenarioRequirement],
}

const NO_DEPENDENCIES: &[IdentifierKind] = &[];
const DEPENDS_ON_IBAN: &[IdentifierKind] = &[IdentifierKind::Iban];

const SUPPLIER_BASIC_REQUIREMENTS: &[ScenarioRequirement] = &[
    ScenarioRequirement {
        kind: IdentifierKind::MarketPartnerId,
        default_profile: GenerationProfile::ChecksumOnly,
        depends_on: NO_DEPENDENCIES,
    },
    ScenarioRequirement {
        kind: IdentifierKind::Iban,
        default_profile: GenerationProfile::SyntheticNonRoutable,
        depends_on: NO_DEPENDENCIES,
    },
    ScenarioRequirement {
        kind: IdentifierKind::Bic,
        default_profile: GenerationProfile::TestTrainingPattern,
        depends_on: DEPENDS_ON_IBAN,
    },
];

const SUPPLIER_DIRECT_DEBIT_REQUIREMENTS: &[ScenarioRequirement] = &[
    ScenarioRequirement {
        kind: IdentifierKind::MarketPartnerId,
        default_profile: GenerationProfile::ChecksumOnly,
        depends_on: NO_DEPENDENCIES,
    },
    ScenarioRequirement {
        kind: IdentifierKind::Mastr,
        default_profile: GenerationProfile::ChecksumOnly,
        depends_on: NO_DEPENDENCIES,
    },
    ScenarioRequirement {
        kind: IdentifierKind::Iban,
        default_profile: GenerationProfile::SyntheticNonRoutable,
        depends_on: NO_DEPENDENCIES,
    },
    ScenarioRequirement {
        kind: IdentifierKind::Bic,
        default_profile: GenerationProfile::TestTrainingPattern,
        depends_on: DEPENDS_ON_IBAN,
    },
    ScenarioRequirement {
        kind: IdentifierKind::CreditorId,
        default_profile: GenerationProfile::OfficialTestFixture,
        depends_on: NO_DEPENDENCIES,
    },
    ScenarioRequirement {
        kind: IdentifierKind::MandateReference,
        default_profile: GenerationProfile::SyntaxOnly,
        depends_on: &[IdentifierKind::CreditorId],
    },
    ScenarioRequirement {
        kind: IdentifierKind::EndToEndId,
        default_profile: GenerationProfile::SyntaxOnly,
        depends_on: NO_DEPENDENCIES,
    },
    ScenarioRequirement {
        kind: IdentifierKind::RfReference,
        default_profile: GenerationProfile::ChecksumOnly,
        depends_on: NO_DEPENDENCIES,
    },
];

const GRID_OPERATOR_ELECTRICITY_REQUIREMENTS: &[ScenarioRequirement] = &[
    ScenarioRequirement {
        kind: IdentifierKind::MarketPartnerId,
        default_profile: GenerationProfile::ChecksumOnly,
        depends_on: NO_DEPENDENCIES,
    },
    ScenarioRequirement {
        kind: IdentifierKind::Malo,
        default_profile: GenerationProfile::ChecksumOnly,
        depends_on: NO_DEPENDENCIES,
    },
    ScenarioRequirement {
        kind: IdentifierKind::Melo,
        default_profile: GenerationProfile::SyntaxOnly,
        depends_on: &[IdentifierKind::Malo],
    },
    ScenarioRequirement {
        kind: IdentifierKind::Nelo,
        default_profile: GenerationProfile::ChecksumOnly,
        depends_on: &[IdentifierKind::Malo],
    },
    ScenarioRequirement {
        kind: IdentifierKind::Nebe,
        default_profile: GenerationProfile::ChecksumOnly,
        depends_on: &[IdentifierKind::Nelo],
    },
    ScenarioRequirement {
        kind: IdentifierKind::TechnicalResourceId,
        default_profile: GenerationProfile::ChecksumOnly,
        depends_on: &[IdentifierKind::Nelo],
    },
    ScenarioRequirement {
        kind: IdentifierKind::ControllableResourceId,
        default_profile: GenerationProfile::ChecksumOnly,
        depends_on: &[IdentifierKind::TechnicalResourceId],
    },
];

const GRID_OPERATOR_GAS_REQUIREMENTS: &[ScenarioRequirement] = &[
    ScenarioRequirement {
        kind: IdentifierKind::MarketPartnerId,
        default_profile: GenerationProfile::ChecksumOnly,
        depends_on: NO_DEPENDENCIES,
    },
    ScenarioRequirement {
        kind: IdentifierKind::Malo,
        default_profile: GenerationProfile::ChecksumOnly,
        depends_on: NO_DEPENDENCIES,
    },
    ScenarioRequirement {
        kind: IdentifierKind::Melo,
        default_profile: GenerationProfile::SyntaxOnly,
        depends_on: &[IdentifierKind::Malo],
    },
];

const METERING_POINT_OPERATOR_REQUIREMENTS: &[ScenarioRequirement] = &[
    ScenarioRequirement {
        kind: IdentifierKind::MarketPartnerId,
        default_profile: GenerationProfile::ChecksumOnly,
        depends_on: NO_DEPENDENCIES,
    },
    ScenarioRequirement {
        kind: IdentifierKind::Malo,
        default_profile: GenerationProfile::ChecksumOnly,
        depends_on: NO_DEPENDENCIES,
    },
    ScenarioRequirement {
        kind: IdentifierKind::Melo,
        default_profile: GenerationProfile::SyntaxOnly,
        depends_on: &[IdentifierKind::Malo],
    },
];

const REDISPATCH_REQUIREMENTS: &[ScenarioRequirement] = &[
    ScenarioRequirement {
        kind: IdentifierKind::MarketPartnerId,
        default_profile: GenerationProfile::ChecksumOnly,
        depends_on: NO_DEPENDENCIES,
    },
    ScenarioRequirement {
        kind: IdentifierKind::Nelo,
        default_profile: GenerationProfile::ChecksumOnly,
        depends_on: NO_DEPENDENCIES,
    },
    ScenarioRequirement {
        kind: IdentifierKind::Nebe,
        default_profile: GenerationProfile::ChecksumOnly,
        depends_on: &[IdentifierKind::Nelo],
    },
    ScenarioRequirement {
        kind: IdentifierKind::ClusterResourceId,
        default_profile: GenerationProfile::ChecksumOnly,
        depends_on: &[IdentifierKind::Nebe],
    },
    ScenarioRequirement {
        kind: IdentifierKind::SteeringGroupId,
        default_profile: GenerationProfile::ChecksumOnly,
        depends_on: &[IdentifierKind::ClusterResourceId],
    },
    ScenarioRequirement {
        kind: IdentifierKind::TechnicalResourceId,
        default_profile: GenerationProfile::ChecksumOnly,
        depends_on: &[IdentifierKind::Nelo],
    },
    ScenarioRequirement {
        kind: IdentifierKind::ControllableResourceId,
        default_profile: GenerationProfile::ChecksumOnly,
        depends_on: &[IdentifierKind::TechnicalResourceId],
    },
    ScenarioRequirement {
        kind: IdentifierKind::PackageId,
        default_profile: GenerationProfile::ChecksumOnly,
        depends_on: &[IdentifierKind::ClusterResourceId],
    },
];

const ELECTRICITY_AND_GAS: &[Sector] = &[Sector::Electricity, Sector::Gas];
const ELECTRICITY: &[Sector] = &[Sector::Electricity];
const GAS: &[Sector] = &[Sector::Gas];

const SCENARIOS: &[ScenarioDefinition] = &[
    ScenarioDefinition {
        scenario: ScenarioKind::SupplierBasic,
        label: "Lieferant · Basis",
        description: "Marktpartner- und Zahlungsstammdaten eines Lieferanten.",
        role: MarketRole::Supplier,
        sectors: ELECTRICITY_AND_GAS,
        requirements: SUPPLIER_BASIC_REQUIREMENTS,
    },
    ScenarioDefinition {
        scenario: ScenarioKind::SupplierDirectDebit,
        label: "Lieferant · SEPA-Lastschrift",
        description: "Zusammenhängende Lieferanten-, Konto-, Mandats- und Zahlungsreferenzen.",
        role: MarketRole::Supplier,
        sectors: ELECTRICITY_AND_GAS,
        requirements: SUPPLIER_DIRECT_DEBIT_REQUIREMENTS,
    },
    ScenarioDefinition {
        scenario: ScenarioKind::GridOperatorElectricity,
        label: "Stromnetzbetreiber",
        description: "Lokationen und steuerbare Ressourcen eines Stromnetzbetreibers.",
        role: MarketRole::GridOperator,
        sectors: ELECTRICITY,
        requirements: GRID_OPERATOR_ELECTRICITY_REQUIREMENTS,
    },
    ScenarioDefinition {
        scenario: ScenarioKind::GridOperatorGas,
        label: "Gasnetzbetreiber",
        description: "Marktpartner- und Lokationskennungen eines Gasnetzbetreibers.",
        role: MarketRole::GridOperator,
        sectors: GAS,
        requirements: GRID_OPERATOR_GAS_REQUIREMENTS,
    },
    ScenarioDefinition {
        scenario: ScenarioKind::MeteringPointOperator,
        label: "Messstellenbetreiber",
        description: "Marktpartner-, Markt- und Messlokationskennungen.",
        role: MarketRole::MeteringPointOperator,
        sectors: ELECTRICITY_AND_GAS,
        requirements: METERING_POINT_OPERATOR_REQUIREMENTS,
    },
    ScenarioDefinition {
        scenario: ScenarioKind::RedispatchResourceBundle,
        label: "Redispatch-Ressourcenbündel",
        description: "Netz-, Cluster-, Steuergruppen- und Ressourcenkennungen mit Abhängigkeiten.",
        role: MarketRole::GridOperator,
        sectors: ELECTRICITY,
        requirements: REDISPATCH_REQUIREMENTS,
    },
];

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct ScenarioCatalogResponse {
    generator_version: String,
    scenarios: Vec<ScenarioDescriptor>,
}

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct ScenarioDescriptor {
    scenario: ScenarioKind,
    label: String,
    description: String,
    role: MarketRole,
    sectors: Vec<Sector>,
    identifiers: Vec<ScenarioIdentifierDescriptor>,
}

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct ScenarioIdentifierDescriptor {
    kind: IdentifierKind,
    default_profile: GenerationProfile,
    depends_on: Vec<IdentifierKind>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub(crate) struct ScenarioGenerateRequest {
    scenario: ScenarioKind,
    sector: Sector,
    #[serde(default)]
    profile: Option<GenerationProfile>,
    #[serde(default)]
    fixture_seed: Option<String>,
}

type ScenarioPayload = Result<Json<ScenarioGenerateRequest>, JsonRejection>;

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct ScenarioGenerateResponse {
    scenario: ScenarioKind,
    sector: Sector,
    requested_profile: Option<GenerationProfile>,
    fixture_seed: String,
    generator_version: String,
    items: Vec<ScenarioGeneratedItem>,
    warnings: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct ScenarioGeneratedItem {
    key: String,
    depends_on: Vec<IdentifierKind>,
    identifier: GeneratedIdentifier,
}

#[allow(dead_code)]
#[derive(utoipa::IntoResponses)]
pub(crate) enum ScenarioApiResponses {
    #[response(status = 200)]
    Success(ScenarioGenerateResponse),
    #[response(status = 400)]
    MalformedJson(ErrorResponse),
    #[response(status = 413)]
    PayloadTooLarge(ErrorResponse),
    #[response(status = 415)]
    UnsupportedMediaType(ErrorResponse),
    #[response(status = 422)]
    InvalidOptions(ErrorResponse),
    #[response(status = 500)]
    InternalInvariant(ErrorResponse),
}

#[utoipa::path(
    get,
    path = "/api/v1/scenarios",
    operation_id = "testDataScenarioCatalog",
    responses((status = 200, body = ScenarioCatalogResponse)),
    tag = "Testdaten · Szenarien"
)]
pub(crate) async fn handle_scenarios() -> Json<ScenarioCatalogResponse> {
    Json(ScenarioCatalogResponse {
        generator_version: id_core::GENERATOR_VERSION.to_string(),
        scenarios: SCENARIOS.iter().map(scenario_descriptor).collect(),
    })
}

#[utoipa::path(
    post,
    path = "/api/v1/scenarios/generate",
    operation_id = "testDataScenarioGenerate",
    request_body = ScenarioGenerateRequest,
    responses(ScenarioApiResponses),
    tag = "Testdaten · Szenarien"
)]
pub(crate) async fn handle_scenario_generate(
    payload: ScenarioPayload,
) -> Result<Json<ScenarioGenerateResponse>, ApiError> {
    let Json(request) = payload.map_err(ApiError::invalid_generate_json)?;
    if request.sector == Sector::CrossSector {
        return Err(ApiError::invalid_request(
            "Scenario sector must be 'electricity' or 'gas'".to_string(),
        ));
    }
    let definition = SCENARIOS
        .iter()
        .find(|definition| definition.scenario == request.scenario)
        .ok_or_else(|| ApiError::invalid_request("Unknown scenario".to_string()))?;
    if !definition.sectors.contains(&request.sector) {
        return Err(ApiError::invalid_request(format!(
            "Scenario '{}' does not support sector '{}'",
            request.scenario.as_str(),
            request.sector.as_str()
        )));
    }

    let fixture_seed = request.fixture_seed.unwrap_or_else(id_core::generate_melo);
    let mut items = Vec::with_capacity(definition.requirements.len());
    let mut warnings = Vec::new();
    for (index, requirement) in definition.requirements.iter().enumerate() {
        let profile = resolve_profile(requirement, request.profile);
        if request
            .profile
            .is_some_and(|requested| requested != profile)
        {
            warnings.push(format!(
                "{} uses profile '{}' because requested profile '{}' is not semantically available.",
                requirement.kind.as_str(),
                profile.as_str(),
                request.profile.expect("checked above").as_str()
            ));
        }
        let prepared = prepare_generation(
            requirement.kind,
            GenerateRequest {
                profile: Some(profile),
                count: 1,
                fixture_seed: Some(fixture_seed.clone()),
                format: IdentifierFormat::Electronic,
                sector: Some(request.sector),
                country: None,
            },
        )?;
        let mut identifier =
            generate_identifier_item(&prepared, index as u32).map_err(|message| {
                ApiError::generation_failed_with_message(requirement.kind.as_str(), message)
            })?;
        if requirement.kind == IdentifierKind::Bic {
            if let Some(directory_bic) = iban_directory_bic(&items) {
                identifier = related_directory_bic(directory_bic, &items)?;
            }
        }
        items.push(ScenarioGeneratedItem {
            key: requirement.kind.as_str().to_string(),
            depends_on: requirement.depends_on.to_vec(),
            identifier,
        });
    }

    Ok(Json(ScenarioGenerateResponse {
        scenario: request.scenario,
        sector: request.sector,
        requested_profile: request.profile,
        fixture_seed,
        generator_version: id_core::GENERATOR_VERSION.to_string(),
        items,
        warnings,
    }))
}

fn scenario_descriptor(definition: &ScenarioDefinition) -> ScenarioDescriptor {
    ScenarioDescriptor {
        scenario: definition.scenario,
        label: definition.label.to_string(),
        description: definition.description.to_string(),
        role: definition.role,
        sectors: definition.sectors.to_vec(),
        identifiers: definition
            .requirements
            .iter()
            .map(|requirement| ScenarioIdentifierDescriptor {
                kind: requirement.kind,
                default_profile: requirement.default_profile,
                depends_on: requirement.depends_on.to_vec(),
            })
            .collect(),
    }
}

fn resolve_profile(
    requirement: &ScenarioRequirement,
    requested: Option<GenerationProfile>,
) -> GenerationProfile {
    let Some(requested) = requested else {
        return requirement.default_profile;
    };
    if descriptor(requirement.kind).is_some_and(|descriptor| descriptor.supports_profile(requested))
    {
        return requested;
    }
    match (requirement.kind, requested) {
        (IdentifierKind::Bic, GenerationProfile::DirectoryPlausible) => {
            GenerationProfile::DirectoryValue
        }
        (IdentifierKind::Bic, GenerationProfile::SyntheticNonRoutable) => {
            GenerationProfile::TestTrainingPattern
        }
        _ => requirement.default_profile,
    }
}

fn iban_directory_bic(items: &[ScenarioGeneratedItem]) -> Option<&str> {
    items
        .iter()
        .find(|item| item.identifier.kind == IdentifierKind::Iban)?
        .identifier
        .parts
        .iter()
        .find(|part| part.name == "bic")
        .map(|part| part.value.as_str())
}

fn related_directory_bic(
    value: &str,
    items: &[ScenarioGeneratedItem],
) -> Result<GeneratedIdentifier, ApiError> {
    let parts = validate_bic(value)
        .map_err(|error| ApiError::generation_failed_with_message("bic", error.to_string()))?;
    let iban = items
        .iter()
        .find(|item| item.identifier.kind == IdentifierKind::Iban)
        .expect("directory BIC was obtained from an IBAN item");
    let bank_code = iban
        .identifier
        .parts
        .iter()
        .find(|part| part.name == "bank_code")
        .map(|part| part.value.clone());
    let mut identifier_parts = vec![
        IdentifierPart::new("business_party_prefix", parts.business_party_prefix),
        IdentifierPart::new("country", parts.country_code),
        IdentifierPart::new("location", parts.location_code),
    ];
    if let Some(branch) = parts.branch_code {
        identifier_parts.push(IdentifierPart::new("branch", branch));
    }
    if let Some(bank_code) = bank_code {
        identifier_parts.push(IdentifierPart::new("bank_code", bank_code));
    }
    Ok(GeneratedIdentifier {
        value: parts.electronic,
        formatted: None,
        kind: IdentifierKind::Bic,
        profile: GenerationProfile::DirectoryValue,
        synthetic: false,
        production_usable: false,
        checks: Checks {
            syntax: CheckStatus::Valid,
            checksum: CheckStatus::NotApplicable,
            directory: CheckStatus::Found,
            assignment: CheckStatus::Unknown,
        },
        allocation_status: AllocationStatus::Unknown,
        account_existence: AccountExistenceStatus::NotApplicable,
        collision_guarantee: CollisionGuarantee::None,
        parts: identifier_parts,
        reference_data: iban.identifier.reference_data.clone().or_else(|| {
            Some(ReferenceData {
                name: "bundesbank_blz".to_string(),
                version: None,
                valid_from: None,
                valid_to: None,
                sha256: None,
            })
        }),
        generator_version: id_core::GENERATOR_VERSION.to_string(),
        warnings: vec![
            "BIC and bank code are linked by the embedded directory; account existence remains unknown."
                .to_string(),
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_contains_all_required_scenarios_and_dependencies() {
        assert_eq!(SCENARIOS.len(), 6);
        assert!(SCENARIOS
            .iter()
            .all(|scenario| !scenario.requirements.is_empty()));
        let direct_debit = SCENARIOS
            .iter()
            .find(|scenario| scenario.scenario == ScenarioKind::SupplierDirectDebit)
            .unwrap();
        assert!(direct_debit
            .requirements
            .iter()
            .any(|requirement| requirement.kind == IdentifierKind::CreditorId));
        assert!(direct_debit
            .requirements
            .iter()
            .any(|requirement| requirement.kind == IdentifierKind::Mastr));
    }
}
