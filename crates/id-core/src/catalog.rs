//! Central metadata catalog and transport-neutral models for supported identifiers.
//!
//! This module deliberately contains no HTTP or OpenAPI routing logic.  The API
//! crate can use [`IDENTIFIER_CATALOG`] as the single source of truth for routes,
//! documentation metadata, and the public `/api/v1/catalog` representation.

use serde::{Deserialize, Serialize};
use std::{error::Error, fmt};

/// Version of the catalog contract (independent from individual generators).
pub const CATALOG_VERSION: &str = "3";

/// Identifier types known to NRG.
///
/// The serialized representation is deliberately identical to the stable public
/// catalog slug.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "api-schema", derive(utoipa::ToSchema))]
pub enum IdentifierKind {
    #[serde(rename = "malo")]
    Malo,
    #[serde(rename = "melo")]
    Melo,
    #[serde(rename = "nelo")]
    Nelo,
    #[serde(rename = "nebe")]
    Nebe,
    #[serde(rename = "iban")]
    Iban,
    #[serde(rename = "bic")]
    Bic,
    #[serde(rename = "creditor-id")]
    CreditorId,
    #[serde(rename = "mandate-reference")]
    MandateReference,
    #[serde(rename = "end-to-end-id")]
    EndToEndId,
    #[serde(rename = "rf-reference")]
    RfReference,
    #[serde(rename = "uetr")]
    Uetr,
    #[serde(rename = "vat-id")]
    VatId,
    #[serde(rename = "lei")]
    Lei,
    #[serde(rename = "mastr")]
    Mastr,
    #[serde(rename = "eic")]
    Eic,
    #[serde(rename = "obis")]
    Obis,
    #[serde(rename = "din-43849")]
    Din43849,
    #[serde(rename = "mp-id")]
    MarketPartnerId,
    #[serde(rename = "cr-id")]
    ClusterResourceId,
    #[serde(rename = "sg-id")]
    SteeringGroupId,
    #[serde(rename = "sr-id")]
    ControllableResourceId,
    #[serde(rename = "tr-id")]
    TechnicalResourceId,
    #[serde(rename = "package-id")]
    PackageId,
}

impl IdentifierKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Malo => "malo",
            Self::Melo => "melo",
            Self::Nelo => "nelo",
            Self::Nebe => "nebe",
            Self::Iban => "iban",
            Self::Bic => "bic",
            Self::CreditorId => "creditor-id",
            Self::MandateReference => "mandate-reference",
            Self::EndToEndId => "end-to-end-id",
            Self::RfReference => "rf-reference",
            Self::Uetr => "uetr",
            Self::VatId => "vat-id",
            Self::Lei => "lei",
            Self::Mastr => "mastr",
            Self::Eic => "eic",
            Self::Obis => "obis",
            Self::Din43849 => "din-43849",
            Self::MarketPartnerId => "mp-id",
            Self::ClusterResourceId => "cr-id",
            Self::SteeringGroupId => "sg-id",
            Self::ControllableResourceId => "sr-id",
            Self::TechnicalResourceId => "tr-id",
            Self::PackageId => "package-id",
        }
    }
}

/// Stable functional grouping used by catalog clients and OpenAPI extensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "api-schema", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum Domain {
    EnergyMarketPartners,
    EnergyLocations,
    EnergyResourcesRedispatch,
    EnergyRegistersAssets,
    MeteringDevicesValues,
    PaymentsAccounts,
    PaymentsInstitutions,
    PaymentsSepaReferences,
    BusinessOrganizations,
    TestDataScenarios,
    SystemCatalog,
}

impl Domain {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EnergyMarketPartners => "energy.market_partners",
            Self::EnergyLocations => "energy.locations",
            Self::EnergyResourcesRedispatch => "energy.resources_redispatch",
            Self::EnergyRegistersAssets => "energy.registers_assets",
            Self::MeteringDevicesValues => "metering.devices_values",
            Self::PaymentsAccounts => "payments.accounts",
            Self::PaymentsInstitutions => "payments.institutions",
            Self::PaymentsSepaReferences => "payments.sepa_references",
            Self::BusinessOrganizations => "business.organizations",
            Self::TestDataScenarios => "test_data.scenarios",
            Self::SystemCatalog => "system.catalog",
        }
    }
}

/// Market-role facets.  A descriptor may apply to several concrete roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "api-schema", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum MarketRole {
    MarketPartner,
    Supplier,
    GridOperator,
    MeteringPointOperator,
    BalancingResponsibleParty,
    AssetOperator,
}

impl MarketRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MarketPartner => "market_partner",
            Self::Supplier => "supplier",
            Self::GridOperator => "grid_operator",
            Self::MeteringPointOperator => "metering_point_operator",
            Self::BalancingResponsibleParty => "balancing_responsible_party",
            Self::AssetOperator => "asset_operator",
        }
    }
}

/// Energy-sector facets.  `CrossSector` is expected to match both electricity
/// and gas filters in clients.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "api-schema", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum Sector {
    Electricity,
    Gas,
    CrossSector,
}

impl Sector {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Electricity => "electricity",
            Self::Gas => "gas",
            Self::CrossSector => "cross_sector",
        }
    }
}

/// Operations supported by an identifier implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "api-schema", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Generate,
    Validate,
    Parse,
    Lookup,
    List,
    ScenarioGenerate,
    NegativeFixture,
}

impl Capability {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Generate => "generate",
            Self::Validate => "validate",
            Self::Parse => "parse",
            Self::Lookup => "lookup",
            Self::List => "list",
            Self::ScenarioGenerate => "scenario_generate",
            Self::NegativeFixture => "negative_fixture",
        }
    }
}

/// Checksum algorithm, if an identifier has one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "api-schema", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum ChecksumScheme {
    Mod97,
    BdewLokWaggon,
    BdewAscii,
    EanMod10,
    EicCheckCharacter,
}

impl ChecksumScheme {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mod97 => "mod97",
            Self::BdewLokWaggon => "bdew_lok_waggon",
            Self::BdewAscii => "bdew_ascii",
            Self::EanMod10 => "ean_mod10",
            Self::EicCheckCharacter => "eic_check_character",
        }
    }
}

/// How values of an identifier are assigned in production.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "api-schema", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum AllocationModel {
    CentrallyAllocated,
    DirectoryBacked,
    IssuerAssigned,
    SelfAssigned,
    NotApplicable,
}

impl AllocationModel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CentrallyAllocated => "centrally_allocated",
            Self::DirectoryBacked => "directory_backed",
            Self::IssuerAssigned => "issuer_assigned",
            Self::SelfAssigned => "self_assigned",
            Self::NotApplicable => "not_applicable",
        }
    }
}

/// Explicit semantic guarantees offered by a generator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "api-schema", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum GenerationProfile {
    OfficialTestFixture,
    SyntheticNonRoutable,
    DirectoryPlausible,
    ChecksumOnly,
    TestTrainingPattern,
    SyntaxOnly,
    DirectoryValue,
    OfficialExample,
}

impl GenerationProfile {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OfficialTestFixture => "official_test_fixture",
            Self::SyntheticNonRoutable => "synthetic_non_routable",
            Self::DirectoryPlausible => "directory_plausible",
            Self::ChecksumOnly => "checksum_only",
            Self::TestTrainingPattern => "test_training_pattern",
            Self::SyntaxOnly => "syntax_only",
            Self::DirectoryValue => "directory_value",
            Self::OfficialExample => "official_example",
        }
    }
}

/// Result of an individual validation check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "api-schema", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Valid,
    Invalid,
    Found,
    NotFound,
    NotChecked,
    NotApplicable,
    Unknown,
}

/// Knowledge about real-world assignment, separate from format validity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "api-schema", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum AllocationStatus {
    Allocated,
    NotAllocated,
    Unknown,
    NotApplicable,
}

/// Knowledge about the existence of an underlying payment account.
///
/// This is intentionally separate from format, checksum, directory and
/// allocation evidence: none of those proves that an account exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "api-schema", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum AccountExistenceStatus {
    Unknown,
    NotApplicable,
}

/// Collision guarantee made for one generated fixture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "api-schema", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum CollisionGuarantee {
    /// No collision guarantee is made beyond the documented format rules.
    None,
    /// The API guarantees uniqueness among items returned in the same batch.
    WithinBatch,
    /// Collision semantics do not apply to this value.
    NotApplicable,
}

/// Output representation requested from a generator.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "api-schema", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum IdentifierFormat {
    #[default]
    Electronic,
    Formatted,
}

/// HTTP method metadata.  Kept transport-neutral by avoiding an HTTP crate type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ApiMethod {
    Get,
    Post,
}

impl ApiMethod {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "get",
            Self::Post => "post",
        }
    }
}

/// Metadata for one explicit public API operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ApiOperationDescriptor {
    pub path: &'static str,
    pub method: ApiMethod,
    pub capability: Capability,
    pub operation_id: &'static str,
    /// Exactly one primary navigation tag. Roles and sectors remain facets.
    pub primary_tag: &'static str,
    pub deprecated: bool,
}

/// Reviewed example shown by catalog-driven clients.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct IdentifierExample {
    pub value: &'static str,
    pub label: &'static str,
}

/// Primary or authoritative documentation source for an identifier rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct IdentifierSource {
    pub label: &'static str,
    pub url: &'static str,
}

/// Complete static description of one identifier type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct IdentifierDescriptor {
    pub kind: IdentifierKind,
    pub slug: &'static str,
    pub label: &'static str,
    /// Short, user-facing explanation of the identifier's purpose and limits.
    pub description: &'static str,
    /// Human-readable structural rule, separate from assignment semantics.
    pub format_description: &'static str,
    pub examples: &'static [IdentifierExample],
    pub sources: &'static [IdentifierSource],
    pub domain: Domain,
    pub roles: &'static [MarketRole],
    pub sectors: &'static [Sector],
    pub capabilities: &'static [Capability],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum_scheme: Option<ChecksumScheme>,
    pub allocation_model: AllocationModel,
    pub generation_profiles: &'static [GenerationProfile],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_profile: Option<GenerationProfile>,
    pub operations: &'static [ApiOperationDescriptor],
}

/// Metadata for public operations which are not owned by a single identifier,
/// for example the catalog and cross-identifier scenario generator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ServiceOperationDescriptor {
    pub domain: Domain,
    pub roles: &'static [MarketRole],
    pub sectors: &'static [Sector],
    pub allocation_model: AllocationModel,
    pub generation_profiles: &'static [GenerationProfile],
    pub operation: ApiOperationDescriptor,
}

impl IdentifierDescriptor {
    pub fn supports(&self, capability: Capability) -> bool {
        self.capabilities.contains(&capability)
    }

    pub fn supports_profile(&self, profile: GenerationProfile) -> bool {
        self.generation_profiles.contains(&profile)
    }
}

/// Batch generation request shared by all generator endpoints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "api-schema", derive(utoipa::ToSchema))]
#[serde(default)]
pub struct GenerateRequest {
    /// Omitted means the descriptor's `default_profile`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<GenerationProfile>,
    /// Number of values to generate; must be within [`MIN_GENERATION_COUNT`]
    /// and [`MAX_GENERATION_COUNT`].
    #[cfg_attr(feature = "api-schema", schema(minimum = 1, maximum = 100))]
    pub count: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fixture_seed: Option<String>,
    pub format: IdentifierFormat,
    /// Optional sector selector for identifier kinds with sector-specific
    /// formation rules, currently BDEW/DVGW market-partner IDs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sector: Option<Sector>,
    /// Optional ISO 3166 country selector for international identifiers such
    /// as IBAN. Omitted IBAN country defaults to Germany.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
}

pub const MIN_GENERATION_COUNT: u8 = 1;
pub const MAX_GENERATION_COUNT: u8 = 100;

impl Default for GenerateRequest {
    fn default() -> Self {
        Self {
            profile: None,
            count: MIN_GENERATION_COUNT,
            fixture_seed: None,
            format: IdentifierFormat::Electronic,
            sector: None,
            country: None,
        }
    }
}

impl GenerateRequest {
    /// Returns a checked batch size suitable for loop bounds and allocation.
    pub fn validated_count(&self) -> Result<u8, GenerateRequestError> {
        if (MIN_GENERATION_COUNT..=MAX_GENERATION_COUNT).contains(&self.count) {
            Ok(self.count)
        } else {
            Err(GenerateRequestError::CountOutOfRange {
                actual: self.count,
                min: MIN_GENERATION_COUNT,
                max: MAX_GENERATION_COUNT,
            })
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenerateRequestError {
    CountOutOfRange { actual: u8, min: u8, max: u8 },
}

impl fmt::Display for GenerateRequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CountOutOfRange { actual, min, max } => {
                write!(
                    formatter,
                    "count must be between {min} and {max}, got {actual}"
                )
            }
        }
    }
}

impl Error for GenerateRequestError {}

/// Named, parsed component of an identifier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "api-schema", derive(utoipa::ToSchema))]
pub struct IdentifierPart {
    pub name: String,
    pub value: String,
}

impl IdentifierPart {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

/// Version and validity metadata for an embedded or queried reference dataset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "api-schema", derive(utoipa::ToSchema))]
pub struct ReferenceData {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valid_from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valid_to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

/// Orthogonal check results.  In particular, checksum validity never implies a
/// directory hit or a real-world assignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "api-schema", derive(utoipa::ToSchema))]
pub struct Checks {
    pub syntax: CheckStatus,
    pub checksum: CheckStatus,
    pub directory: CheckStatus,
    pub assignment: CheckStatus,
}

impl Default for Checks {
    fn default() -> Self {
        Self {
            syntax: CheckStatus::Unknown,
            checksum: CheckStatus::NotChecked,
            directory: CheckStatus::NotChecked,
            assignment: CheckStatus::NotChecked,
        }
    }
}

/// One generated value and the precise guarantees made about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "api-schema", derive(utoipa::ToSchema))]
pub struct GeneratedIdentifier {
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formatted: Option<String>,
    pub kind: IdentifierKind,
    pub profile: GenerationProfile,
    pub synthetic: bool,
    pub production_usable: bool,
    pub checks: Checks,
    pub allocation_status: AllocationStatus,
    pub account_existence: AccountExistenceStatus,
    pub collision_guarantee: CollisionGuarantee,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<IdentifierPart>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_data: Option<ReferenceData>,
    pub generator_version: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

/// Unified validation and parsing result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "api-schema", derive(utoipa::ToSchema))]
pub struct ValidationReport {
    pub kind: IdentifierKind,
    pub input: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalized: Option<String>,
    pub valid: bool,
    pub checks: Checks,
    pub allocation_status: AllocationStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub synthetic: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub production_usable: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<IdentifierPart>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_data: Option<ReferenceData>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
}

const ALL_MARKET_ROLES: &[MarketRole] = &[
    MarketRole::MarketPartner,
    MarketRole::Supplier,
    MarketRole::GridOperator,
    MarketRole::MeteringPointOperator,
    MarketRole::BalancingResponsibleParty,
    MarketRole::AssetOperator,
];

const LOCATION_ROLES: &[MarketRole] = &[
    MarketRole::Supplier,
    MarketRole::GridOperator,
    MarketRole::MeteringPointOperator,
];

const RESOURCE_ROLES: &[MarketRole] = &[MarketRole::GridOperator, MarketRole::AssetOperator];
const GRID_OPERATOR_ROLES: &[MarketRole] = &[MarketRole::GridOperator];
const ELECTRICITY_AND_GAS: &[Sector] = &[Sector::Electricity, Sector::Gas];
const CROSS_SECTOR: &[Sector] = &[Sector::CrossSector];
const ELECTRICITY: &[Sector] = &[Sector::Electricity];

const GENERATE_VALIDATE_PARSE: &[Capability] = &[
    Capability::Generate,
    Capability::Validate,
    Capability::Parse,
    Capability::NegativeFixture,
];
const GENERATE_VALIDATE: &[Capability] = &[
    Capability::Generate,
    Capability::Validate,
    Capability::NegativeFixture,
];
const GENERATE_ONLY: &[Capability] = &[Capability::Generate, Capability::NegativeFixture];
const VALIDATE_PARSE: &[Capability] = &[
    Capability::Validate,
    Capability::Parse,
    Capability::NegativeFixture,
];
const VALIDATE_PARSE_LOOKUP: &[Capability] = &[
    Capability::Validate,
    Capability::Parse,
    Capability::Lookup,
    Capability::NegativeFixture,
];

const CHECKSUM_ONLY: &[GenerationProfile] = &[GenerationProfile::ChecksumOnly];
const SYNTAX_ONLY: &[GenerationProfile] = &[GenerationProfile::SyntaxOnly];

const BDEW_IDENTIFIER_SOURCES: &[IdentifierSource] = &[IdentifierSource {
    label: "BDEW · Identifikatoren in der Marktkommunikation, Version 1.2",
    url: "https://www.bdew.de/media/documents/AWH_Identifikatoren-in-der-Marktkommunikation_Version.1.2.pdf",
}];
const IBAN_SOURCES: &[IdentifierSource] = &[
    IdentifierSource {
        label: "SWIFT · IBAN Registry, Release 102",
        url: "https://www.swift.com/swift-resource/9606/download",
    },
    IdentifierSource {
        label: "Deutsche Bundesbank · IBAN-Regeln",
        url: "https://www.bundesbank.de/de/aufgaben/unbarer-zahlungsverkehr/serviceangebot/iban-regeln",
    },
];
const BIC_SOURCES: &[IdentifierSource] = &[IdentifierSource {
    label: "SWIFT · ISO 9362 BIC Implementation",
    url: "https://www.swift.com/swift-resource/14256/download?language=en",
}];
const CREDITOR_ID_SOURCES: &[IdentifierSource] = &[IdentifierSource {
    label: "Deutsche Bundesbank · Gläubiger-Identifikationsnummer",
    url: "https://www.bundesbank.de/dynamic/action/de/aufgaben/unbarer-zahlungsverkehr/serviceangebot/sepa/glaeubiger-identifikationsnummer/642684/haeufig-gestellte-fragen-zu-der-glaeubiger-identifikationsnummer?contentId=640170&firstLetter=W",
}];
const MANDATE_SOURCES: &[IdentifierSource] = &[IdentifierSource {
    label: "European Payments Council · SEPA Direct Debit e-Mandate Guidelines 2025",
    url: "https://www.europeanpaymentscouncil.eu/sites/default/files/kb/file/2025-10/EPC002-09%20SDD%20Core%20e-Mandate%20Service%20IG%202025%20V1.0.pdf",
}];
const END_TO_END_SOURCES: &[IdentifierSource] = &[IdentifierSource {
    label: "European Payments Council · SEPA Credit Transfer Guidelines 2025",
    url: "https://www.europeanpaymentscouncil.eu/sites/default/files/kb/file/2025-10/EPC115-06%20SCT%20Inter-PSP%20IG%202025%20V1.0.pdf",
}];
const RF_SOURCES: &[IdentifierSource] = &[IdentifierSource {
    label: "ISO · ISO 11649 Structured creditor reference",
    url: "https://www.iso.org/standard/50649.html",
}];
const UETR_SOURCES: &[IdentifierSource] = &[IdentifierSource {
    label: "SWIFT · Unique End-to-end Transaction Reference",
    url: "https://www.swift.com/payments/what-unique-end-end-transaction-reference-uetr",
}];
const VAT_SOURCES: &[IdentifierSource] = &[IdentifierSource {
    label: "BZSt · Aufbau der Umsatzsteuer-Identifikationsnummer",
    url: "https://www.bzst.de/SharedDocs/Downloads/DE/Merkblaetter/ust_idnr_aufbau.pdf?__blob=publicationFile&v=2",
}];
const LEI_SOURCES: &[IdentifierSource] = &[IdentifierSource {
    label: "GLEIF · LEI-Daten und API",
    url: "https://www.gleif.org/en/lei-data/gleif-api/",
}];
const MASTR_SOURCES: &[IdentifierSource] = &[IdentifierSource {
    label: "Bundesnetzagentur · MaStR-Nummernkonzept",
    url:
        "https://www.marktstammdatenregister.de/MaStRHilfe/files/regHilfen/MaStR-Nummernkonzept.pdf",
}];
const EIC_SOURCES: &[IdentifierSource] = &[IdentifierSource {
    label: "ENTSO-E · Energy Identification Codes",
    url: "https://www.entsoe.eu/data/energy-identification-codes-eic/",
}];
const OBIS_SOURCES: &[IdentifierSource] = &[IdentifierSource {
    label: "DLMS UA · Blue Book Part 1, Edition 17",
    url: "https://www.dlms.com/wp-content/uploads/2025/06/Excerpts-DLMS-Blue-Book-Ed-17-part-1-V1.0.pdf",
}];
const DIN_43849_SOURCES: &[IdentifierSource] = &[IdentifierSource {
    label: "DIN Media · DIN 43849:2024-05",
    url: "https://www.dinmedia.de/de/norm/din-43849/377951610",
}];

macro_rules! negative_operation {
    ($slug:literal, $operation_id:literal) => {
        ApiOperationDescriptor {
            path: concat!("/api/v1/test-data/negative/", $slug, "/generate"),
            method: ApiMethod::Post,
            capability: Capability::NegativeFixture,
            operation_id: $operation_id,
            primary_tag: "Testdaten · Szenarien",
            deprecated: false,
        }
    };
}

const MALO_OPERATIONS: &[ApiOperationDescriptor] = &[
    ApiOperationDescriptor {
        path: "/api/malo/generate",
        method: ApiMethod::Get,
        capability: Capability::Generate,
        operation_id: "legacyMaloGenerate",
        primary_tag: "Energie · Lokationen",
        deprecated: true,
    },
    ApiOperationDescriptor {
        path: "/api/malo/validate",
        method: ApiMethod::Post,
        capability: Capability::Validate,
        operation_id: "legacyMaloValidate",
        primary_tag: "Energie · Lokationen",
        deprecated: true,
    },
    ApiOperationDescriptor {
        path: "/api/v1/energy/locations/malo/generate",
        method: ApiMethod::Post,
        capability: Capability::Generate,
        operation_id: "energyMaloGenerate",
        primary_tag: "Energie · Lokationen",
        deprecated: false,
    },
    ApiOperationDescriptor {
        path: "/api/v1/energy/locations/malo/validate",
        method: ApiMethod::Post,
        capability: Capability::Validate,
        operation_id: "energyMaloValidate",
        primary_tag: "Energie · Lokationen",
        deprecated: false,
    },
    negative_operation!("malo", "testDataNegativeMaloGenerate"),
];

const MELO_OPERATIONS: &[ApiOperationDescriptor] = &[
    ApiOperationDescriptor {
        path: "/api/melo/generate",
        method: ApiMethod::Get,
        capability: Capability::Generate,
        operation_id: "legacyMeloGenerate",
        primary_tag: "Energie · Lokationen",
        deprecated: true,
    },
    ApiOperationDescriptor {
        path: "/api/melo/validate",
        method: ApiMethod::Post,
        capability: Capability::Validate,
        operation_id: "legacyMeloValidate",
        primary_tag: "Energie · Lokationen",
        deprecated: true,
    },
    ApiOperationDescriptor {
        path: "/api/v1/energy/locations/melo/generate",
        method: ApiMethod::Post,
        capability: Capability::Generate,
        operation_id: "energyMeloGenerate",
        primary_tag: "Energie · Lokationen",
        deprecated: false,
    },
    ApiOperationDescriptor {
        path: "/api/v1/energy/locations/melo/validate",
        method: ApiMethod::Post,
        capability: Capability::Validate,
        operation_id: "energyMeloValidate",
        primary_tag: "Energie · Lokationen",
        deprecated: false,
    },
    negative_operation!("melo", "testDataNegativeMeloGenerate"),
];

const NELO_OPERATIONS: &[ApiOperationDescriptor] = &[
    ApiOperationDescriptor {
        path: "/api/nelo/generate",
        method: ApiMethod::Get,
        capability: Capability::Generate,
        operation_id: "legacyNeloGenerate",
        primary_tag: "Energie · Lokationen",
        deprecated: true,
    },
    ApiOperationDescriptor {
        path: "/api/nelo/validate",
        method: ApiMethod::Post,
        capability: Capability::Validate,
        operation_id: "legacyNeloValidate",
        primary_tag: "Energie · Lokationen",
        deprecated: true,
    },
    ApiOperationDescriptor {
        path: "/api/v1/energy/locations/nelo/generate",
        method: ApiMethod::Post,
        capability: Capability::Generate,
        operation_id: "energyNeloGenerate",
        primary_tag: "Energie · Lokationen",
        deprecated: false,
    },
    ApiOperationDescriptor {
        path: "/api/v1/energy/locations/nelo/validate",
        method: ApiMethod::Post,
        capability: Capability::Validate,
        operation_id: "energyNeloValidate",
        primary_tag: "Energie · Lokationen",
        deprecated: false,
    },
    negative_operation!("nelo", "testDataNegativeNeloGenerate"),
];

const NEBE_OPERATIONS: &[ApiOperationDescriptor] = &[
    ApiOperationDescriptor {
        path: "/api/v1/energy/locations/nebe/generate",
        method: ApiMethod::Post,
        capability: Capability::Generate,
        operation_id: "energyNebeGenerate",
        primary_tag: "Energie · Lokationen",
        deprecated: false,
    },
    ApiOperationDescriptor {
        path: "/api/v1/energy/locations/nebe/validate",
        method: ApiMethod::Post,
        capability: Capability::Validate,
        operation_id: "energyNebeValidate",
        primary_tag: "Energie · Lokationen",
        deprecated: false,
    },
    negative_operation!("nebe", "testDataNegativeNebeGenerate"),
];

const IBAN_OPERATIONS: &[ApiOperationDescriptor] = &[
    ApiOperationDescriptor {
        path: "/api/v1/payments/accounts/iban/generate",
        method: ApiMethod::Post,
        capability: Capability::Generate,
        operation_id: "paymentsIbanGenerate",
        primary_tag: "Zahlungsverkehr · Konten & Banken",
        deprecated: false,
    },
    ApiOperationDescriptor {
        path: "/api/v1/payments/accounts/iban/validate",
        method: ApiMethod::Post,
        capability: Capability::Validate,
        operation_id: "paymentsIbanValidate",
        primary_tag: "Zahlungsverkehr · Konten & Banken",
        deprecated: false,
    },
    negative_operation!("iban", "testDataNegativeIbanGenerate"),
];

const BIC_OPERATIONS: &[ApiOperationDescriptor] = &[
    ApiOperationDescriptor {
        path: "/api/v1/payments/institutions/bic/generate",
        method: ApiMethod::Post,
        capability: Capability::Generate,
        operation_id: "paymentsBicGenerate",
        primary_tag: "Zahlungsverkehr · Konten & Banken",
        deprecated: false,
    },
    ApiOperationDescriptor {
        path: "/api/v1/payments/institutions/bic/validate",
        method: ApiMethod::Post,
        capability: Capability::Validate,
        operation_id: "paymentsBicValidate",
        primary_tag: "Zahlungsverkehr · Konten & Banken",
        deprecated: false,
    },
    negative_operation!("bic", "testDataNegativeBicGenerate"),
];

const CREDITOR_ID_OPERATIONS: &[ApiOperationDescriptor] = &[
    ApiOperationDescriptor {
        path: "/api/v1/payments/sepa/creditor-id/generate",
        method: ApiMethod::Post,
        capability: Capability::Generate,
        operation_id: "paymentsCreditorIdGenerate",
        primary_tag: "Zahlungsverkehr · SEPA & Referenzen",
        deprecated: false,
    },
    ApiOperationDescriptor {
        path: "/api/v1/payments/sepa/creditor-id/validate",
        method: ApiMethod::Post,
        capability: Capability::Validate,
        operation_id: "paymentsCreditorIdValidate",
        primary_tag: "Zahlungsverkehr · SEPA & Referenzen",
        deprecated: false,
    },
    negative_operation!("creditor-id", "testDataNegativeCreditorIdGenerate"),
];

const MANDATE_REFERENCE_OPERATIONS: &[ApiOperationDescriptor] = &[
    ApiOperationDescriptor {
        path: "/api/v1/payments/sepa/mandate-reference/generate",
        method: ApiMethod::Post,
        capability: Capability::Generate,
        operation_id: "paymentsMandateReferenceGenerate",
        primary_tag: "Zahlungsverkehr · SEPA & Referenzen",
        deprecated: false,
    },
    ApiOperationDescriptor {
        path: "/api/v1/payments/sepa/mandate-reference/validate",
        method: ApiMethod::Post,
        capability: Capability::Validate,
        operation_id: "paymentsMandateReferenceValidate",
        primary_tag: "Zahlungsverkehr · SEPA & Referenzen",
        deprecated: false,
    },
    negative_operation!(
        "mandate-reference",
        "testDataNegativeMandateReferenceGenerate"
    ),
];

const END_TO_END_ID_OPERATIONS: &[ApiOperationDescriptor] = &[
    ApiOperationDescriptor {
        path: "/api/v1/payments/sepa/end-to-end-id/generate",
        method: ApiMethod::Post,
        capability: Capability::Generate,
        operation_id: "paymentsEndToEndIdGenerate",
        primary_tag: "Zahlungsverkehr · SEPA & Referenzen",
        deprecated: false,
    },
    ApiOperationDescriptor {
        path: "/api/v1/payments/sepa/end-to-end-id/validate",
        method: ApiMethod::Post,
        capability: Capability::Validate,
        operation_id: "paymentsEndToEndIdValidate",
        primary_tag: "Zahlungsverkehr · SEPA & Referenzen",
        deprecated: false,
    },
    negative_operation!("end-to-end-id", "testDataNegativeEndToEndIdGenerate"),
];

const RF_REFERENCE_OPERATIONS: &[ApiOperationDescriptor] = &[
    ApiOperationDescriptor {
        path: "/api/v1/payments/sepa/rf-reference/generate",
        method: ApiMethod::Post,
        capability: Capability::Generate,
        operation_id: "paymentsRfReferenceGenerate",
        primary_tag: "Zahlungsverkehr · SEPA & Referenzen",
        deprecated: false,
    },
    ApiOperationDescriptor {
        path: "/api/v1/payments/sepa/rf-reference/validate",
        method: ApiMethod::Post,
        capability: Capability::Validate,
        operation_id: "paymentsRfReferenceValidate",
        primary_tag: "Zahlungsverkehr · SEPA & Referenzen",
        deprecated: false,
    },
    negative_operation!("rf-reference", "testDataNegativeRfReferenceGenerate"),
];

const MARKET_PARTNER_OPERATIONS: &[ApiOperationDescriptor] = &[
    ApiOperationDescriptor {
        path: "/api/v1/energy/market-partners/mp-id/generate",
        method: ApiMethod::Post,
        capability: Capability::Generate,
        operation_id: "energyMarketPartnerIdGenerate",
        primary_tag: "Energie · Marktpartner",
        deprecated: false,
    },
    ApiOperationDescriptor {
        path: "/api/v1/energy/market-partners/mp-id/validate",
        method: ApiMethod::Post,
        capability: Capability::Validate,
        operation_id: "energyMarketPartnerIdValidate",
        primary_tag: "Energie · Marktpartner",
        deprecated: false,
    },
    negative_operation!("mp-id", "testDataNegativeMarketPartnerIdGenerate"),
];

const CLUSTER_RESOURCE_OPERATIONS: &[ApiOperationDescriptor] = &[
    ApiOperationDescriptor {
        path: "/api/v1/energy/resources/cr-id/generate",
        method: ApiMethod::Post,
        capability: Capability::Generate,
        operation_id: "energyClusterResourceIdGenerate",
        primary_tag: "Energie · Ressourcen & Redispatch",
        deprecated: false,
    },
    negative_operation!("cr-id", "testDataNegativeClusterResourceIdGenerate"),
];

const STEERING_GROUP_OPERATIONS: &[ApiOperationDescriptor] = &[
    ApiOperationDescriptor {
        path: "/api/v1/energy/resources/sg-id/generate",
        method: ApiMethod::Post,
        capability: Capability::Generate,
        operation_id: "energySteeringGroupIdGenerate",
        primary_tag: "Energie · Ressourcen & Redispatch",
        deprecated: false,
    },
    negative_operation!("sg-id", "testDataNegativeSteeringGroupIdGenerate"),
];

const CONTROLLABLE_RESOURCE_OPERATIONS: &[ApiOperationDescriptor] = &[
    ApiOperationDescriptor {
        path: "/api/v1/energy/resources/sr-id/generate",
        method: ApiMethod::Post,
        capability: Capability::Generate,
        operation_id: "energyControllableResourceIdGenerate",
        primary_tag: "Energie · Ressourcen & Redispatch",
        deprecated: false,
    },
    negative_operation!("sr-id", "testDataNegativeControllableResourceIdGenerate"),
];

const TECHNICAL_RESOURCE_OPERATIONS: &[ApiOperationDescriptor] = &[
    ApiOperationDescriptor {
        path: "/api/v1/energy/resources/tr-id/generate",
        method: ApiMethod::Post,
        capability: Capability::Generate,
        operation_id: "energyTechnicalResourceIdGenerate",
        primary_tag: "Energie · Ressourcen & Redispatch",
        deprecated: false,
    },
    negative_operation!("tr-id", "testDataNegativeTechnicalResourceIdGenerate"),
];

const PACKAGE_OPERATIONS: &[ApiOperationDescriptor] = &[
    ApiOperationDescriptor {
        path: "/api/v1/energy/resources/package-id/generate",
        method: ApiMethod::Post,
        capability: Capability::Generate,
        operation_id: "energyPackageIdGenerate",
        primary_tag: "Energie · Ressourcen & Redispatch",
        deprecated: false,
    },
    negative_operation!("package-id", "testDataNegativePackageIdGenerate"),
];

const UETR_OPERATIONS: &[ApiOperationDescriptor] = &[
    ApiOperationDescriptor {
        path: "/api/v1/payments/sepa/uetr/generate",
        method: ApiMethod::Post,
        capability: Capability::Generate,
        operation_id: "paymentsUetrGenerate",
        primary_tag: "Zahlungsverkehr · SEPA & Referenzen",
        deprecated: false,
    },
    ApiOperationDescriptor {
        path: "/api/v1/payments/sepa/uetr/validate",
        method: ApiMethod::Post,
        capability: Capability::Validate,
        operation_id: "paymentsUetrValidate",
        primary_tag: "Zahlungsverkehr · SEPA & Referenzen",
        deprecated: false,
    },
    negative_operation!("uetr", "testDataNegativeUetrGenerate"),
];

const VAT_ID_OPERATIONS: &[ApiOperationDescriptor] = &[
    ApiOperationDescriptor {
        path: "/api/v1/business/tax/vat-id/validate",
        method: ApiMethod::Post,
        capability: Capability::Validate,
        operation_id: "businessVatIdValidate",
        primary_tag: "Unternehmen · Stammdaten & Register",
        deprecated: false,
    },
    negative_operation!("vat-id", "testDataNegativeVatIdGenerate"),
];

const LEI_OPERATIONS: &[ApiOperationDescriptor] = &[
    ApiOperationDescriptor {
        path: "/api/v1/business/organizations/lei/validate",
        method: ApiMethod::Post,
        capability: Capability::Validate,
        operation_id: "businessLeiValidate",
        primary_tag: "Unternehmen · Stammdaten & Register",
        deprecated: false,
    },
    ApiOperationDescriptor {
        path: "/api/v1/business/organizations/lei/lookup",
        method: ApiMethod::Post,
        capability: Capability::Lookup,
        operation_id: "businessLeiLookup",
        primary_tag: "Unternehmen · Stammdaten & Register",
        deprecated: false,
    },
    negative_operation!("lei", "testDataNegativeLeiGenerate"),
];

const MASTR_OPERATIONS: &[ApiOperationDescriptor] = &[
    ApiOperationDescriptor {
        path: "/api/v1/energy/registers/mastr/generate",
        method: ApiMethod::Post,
        capability: Capability::Generate,
        operation_id: "energyMastrGenerate",
        primary_tag: "Energie · Register & Anlagen",
        deprecated: false,
    },
    ApiOperationDescriptor {
        path: "/api/v1/energy/registers/mastr/validate",
        method: ApiMethod::Post,
        capability: Capability::Validate,
        operation_id: "energyMastrValidate",
        primary_tag: "Energie · Register & Anlagen",
        deprecated: false,
    },
    negative_operation!("mastr", "testDataNegativeMastrGenerate"),
];

const EIC_OPERATIONS: &[ApiOperationDescriptor] = &[
    ApiOperationDescriptor {
        path: "/api/v1/energy/registers/eic/validate",
        method: ApiMethod::Post,
        capability: Capability::Validate,
        operation_id: "energyEicValidate",
        primary_tag: "Energie · Register & Anlagen",
        deprecated: false,
    },
    ApiOperationDescriptor {
        path: "/api/v1/energy/registers/eic/lookup",
        method: ApiMethod::Post,
        capability: Capability::Lookup,
        operation_id: "energyEicLookup",
        primary_tag: "Energie · Register & Anlagen",
        deprecated: false,
    },
    negative_operation!("eic", "testDataNegativeEicGenerate"),
];

const OBIS_OPERATIONS: &[ApiOperationDescriptor] = &[
    ApiOperationDescriptor {
        path: "/api/v1/metering/values/obis/validate",
        method: ApiMethod::Post,
        capability: Capability::Validate,
        operation_id: "meteringObisValidate",
        primary_tag: "Messwesen · Geräte & Werte",
        deprecated: false,
    },
    ApiOperationDescriptor {
        path: "/api/v1/metering/values/obis/lookup",
        method: ApiMethod::Post,
        capability: Capability::Lookup,
        operation_id: "meteringObisLookup",
        primary_tag: "Messwesen · Geräte & Werte",
        deprecated: false,
    },
    negative_operation!("obis", "testDataNegativeObisGenerate"),
];

const DIN_43849_OPERATIONS: &[ApiOperationDescriptor] = &[
    ApiOperationDescriptor {
        path: "/api/v1/metering/devices/din-43849/validate",
        method: ApiMethod::Post,
        capability: Capability::Validate,
        operation_id: "meteringDin43849Validate",
        primary_tag: "Messwesen · Geräte & Werte",
        deprecated: false,
    },
    negative_operation!("din-43849", "testDataNegativeDin43849Generate"),
];

pub const SERVICE_OPERATIONS: &[ServiceOperationDescriptor] = &[
    ServiceOperationDescriptor {
        domain: Domain::SystemCatalog,
        roles: &[],
        sectors: &[],
        allocation_model: AllocationModel::NotApplicable,
        generation_profiles: &[],
        operation: ApiOperationDescriptor {
            path: "/api/v1/catalog",
            method: ApiMethod::Get,
            capability: Capability::List,
            operation_id: "systemCatalogList",
            primary_tag: "System · Katalog",
            deprecated: false,
        },
    },
    ServiceOperationDescriptor {
        domain: Domain::TestDataScenarios,
        roles: ALL_MARKET_ROLES,
        sectors: ELECTRICITY_AND_GAS,
        allocation_model: AllocationModel::NotApplicable,
        generation_profiles: &[],
        operation: ApiOperationDescriptor {
            path: "/api/v1/scenarios",
            method: ApiMethod::Get,
            capability: Capability::List,
            operation_id: "testDataScenarioCatalog",
            primary_tag: "Testdaten · Szenarien",
            deprecated: false,
        },
    },
    ServiceOperationDescriptor {
        domain: Domain::TestDataScenarios,
        roles: ALL_MARKET_ROLES,
        sectors: ELECTRICITY_AND_GAS,
        allocation_model: AllocationModel::NotApplicable,
        generation_profiles: &[
            GenerationProfile::OfficialTestFixture,
            GenerationProfile::SyntheticNonRoutable,
            GenerationProfile::DirectoryPlausible,
            GenerationProfile::ChecksumOnly,
        ],
        operation: ApiOperationDescriptor {
            path: "/api/v1/scenarios/generate",
            method: ApiMethod::Post,
            capability: Capability::ScenarioGenerate,
            operation_id: "testDataScenarioGenerate",
            primary_tag: "Testdaten · Szenarien",
            deprecated: false,
        },
    },
];

/// Static catalog used by the API and clients.  No identifier list should be
/// duplicated in the frontend or OpenAPI layer.
pub const IDENTIFIER_CATALOG: &[IdentifierDescriptor] = &[
    IdentifierDescriptor {
        kind: IdentifierKind::Malo,
        slug: "malo",
        label: "Marktlokations-ID (MaLo-ID)",
        description: "Identifiziert eine Marktlokation in der deutschen Energie-Marktkommunikation.",
        format_description: "Elf Ziffern: zehnstelliger Stamm und eine Prüfziffer nach dem BDEW-Lok-und-Waggon-Verfahren.",
        examples: &[IdentifierExample {
            value: "41373559241",
            label: "Prüfziffergültiges Formatbeispiel; keine Vergabeaussage",
        }],
        sources: BDEW_IDENTIFIER_SOURCES,
        domain: Domain::EnergyLocations,
        roles: LOCATION_ROLES,
        sectors: ELECTRICITY_AND_GAS,
        capabilities: GENERATE_VALIDATE,
        checksum_scheme: Some(ChecksumScheme::BdewLokWaggon),
        allocation_model: AllocationModel::CentrallyAllocated,
        generation_profiles: CHECKSUM_ONLY,
        default_profile: Some(GenerationProfile::ChecksumOnly),
        operations: MALO_OPERATIONS,
    },
    IdentifierDescriptor {
        kind: IdentifierKind::Melo,
        slug: "melo",
        label: "Messlokations-ID (MeLo-ID)",
        description: "Kennzeichnet den Ort, an dem Energie gemessen und Messwerte gebildet werden.",
        format_description: "33 Zeichen: Länderpräfix DE und 31 Großbuchstaben oder Ziffern; keine standardisierte Prüfziffer.",
        examples: &[IdentifierExample {
            value: "DE00056266802AO6G56M11SN51G21M24S",
            label: "Formatbeispiel; keine Vergabeaussage",
        }],
        sources: BDEW_IDENTIFIER_SOURCES,
        domain: Domain::EnergyLocations,
        roles: LOCATION_ROLES,
        sectors: ELECTRICITY_AND_GAS,
        capabilities: GENERATE_VALIDATE,
        checksum_scheme: None,
        allocation_model: AllocationModel::CentrallyAllocated,
        generation_profiles: SYNTAX_ONLY,
        default_profile: Some(GenerationProfile::SyntaxOnly),
        operations: MELO_OPERATIONS,
    },
    IdentifierDescriptor {
        kind: IdentifierKind::Nelo,
        slug: "nelo",
        label: "Netzlokations-ID (NeLo-ID)",
        description: "Identifiziert die Zuordnung einer Marktlokation zu einem Stromnetz.",
        format_description: "Elf Großbuchstaben oder Ziffern mit typgebundenem Präfix und abschließendem BDEW-ASCII-Prüfzeichen.",
        examples: &[IdentifierExample {
            value: "EABC123DEF8",
            label: "Prüfziffergültiges Formatbeispiel; keine Vergabeaussage",
        }],
        sources: BDEW_IDENTIFIER_SOURCES,
        domain: Domain::EnergyLocations,
        roles: LOCATION_ROLES,
        sectors: ELECTRICITY,
        capabilities: GENERATE_VALIDATE,
        checksum_scheme: Some(ChecksumScheme::BdewAscii),
        allocation_model: AllocationModel::CentrallyAllocated,
        generation_profiles: CHECKSUM_ONLY,
        default_profile: Some(GenerationProfile::ChecksumOnly),
        operations: NELO_OPERATIONS,
    },
    IdentifierDescriptor {
        kind: IdentifierKind::Nebe,
        slug: "nebe",
        label: "Netzbereichs-ID (NeBe-ID)",
        description: "Kennzeichnet einen Netzbereich für energiewirtschaftliche Prozesse.",
        format_description: "Elf Zeichen mit Präfix F, neun alphanumerischen Nutzzeichen und abschließendem BDEW-ASCII-Prüfzeichen.",
        examples: &[],
        sources: BDEW_IDENTIFIER_SOURCES,
        domain: Domain::EnergyLocations,
        roles: GRID_OPERATOR_ROLES,
        sectors: ELECTRICITY,
        capabilities: GENERATE_VALIDATE,
        checksum_scheme: Some(ChecksumScheme::BdewAscii),
        allocation_model: AllocationModel::CentrallyAllocated,
        generation_profiles: CHECKSUM_ONLY,
        default_profile: Some(GenerationProfile::ChecksumOnly),
        operations: NEBE_OPERATIONS,
    },
    IdentifierDescriptor {
        kind: IdentifierKind::Iban,
        slug: "iban",
        label: "International Bank Account Number (IBAN)",
        description: "Adressiert Zahlungskonten; NRG prüft Format, BBAN und MOD 97 sowie für Deutschland optional die BLZ, niemals aber die Kontoexistenz.",
        format_description: "Zwei Buchstaben für das Land, zwei MOD-97-Prüfziffern und eine länderspezifische BBAN; Länge und BBAN-Struktur stammen aus dem versionierten SWIFT-Register.",
        examples: &[IdentifierExample {
            value: "DE89370400440532013000",
            label: "Offizielles SWIFT-Registerbeispiel; keine Sandbox-Garantie",
        }],
        sources: IBAN_SOURCES,
        domain: Domain::PaymentsAccounts,
        roles: ALL_MARKET_ROLES,
        sectors: CROSS_SECTOR,
        capabilities: GENERATE_VALIDATE_PARSE,
        checksum_scheme: Some(ChecksumScheme::Mod97),
        allocation_model: AllocationModel::DirectoryBacked,
        generation_profiles: &[
            GenerationProfile::SyntheticNonRoutable,
            GenerationProfile::DirectoryPlausible,
            GenerationProfile::ChecksumOnly,
            GenerationProfile::OfficialExample,
        ],
        default_profile: Some(GenerationProfile::SyntheticNonRoutable),
        operations: IBAN_OPERATIONS,
    },
    IdentifierDescriptor {
        kind: IdentifierKind::Bic,
        slug: "bic",
        label: "Business Identifier Code (BIC)",
        description: "Identifiziert ein Zahlungsinstitut oder eine Filiale; der BIC besitzt keine Prüfziffer.",
        format_description: "Acht Zeichen aus Business-Party-Präfix, ISO-Land und Ortskennung; optional folgen drei Filialzeichen. Position 8 mit 0 kennzeichnet nur ein Test-&-Training-Muster.",
        examples: &[IdentifierExample {
            value: "NRGXDE10XXX",
            label: "Syntaktisches T&T-Muster; keine SWIFT-Registrierungsaussage",
        }],
        sources: BIC_SOURCES,
        domain: Domain::PaymentsInstitutions,
        roles: ALL_MARKET_ROLES,
        sectors: CROSS_SECTOR,
        capabilities: GENERATE_VALIDATE_PARSE,
        checksum_scheme: None,
        allocation_model: AllocationModel::DirectoryBacked,
        generation_profiles: &[
            GenerationProfile::TestTrainingPattern,
            GenerationProfile::SyntaxOnly,
            GenerationProfile::DirectoryValue,
        ],
        default_profile: Some(GenerationProfile::TestTrainingPattern),
        operations: BIC_OPERATIONS,
    },
    IdentifierDescriptor {
        kind: IdentifierKind::CreditorId,
        slug: "creditor-id",
        label: "SEPA-Gläubiger-Identifikationsnummer",
        description: "Identifiziert den Einreicher von SEPA-Lastschriften.",
        format_description: "Deutschland: genau 18 Zeichen aus DE, zwei MOD-97-Prüfziffern, dreistelligem Business Code und nationalem Merkmal.",
        examples: &[IdentifierExample {
            value: "DE98ZZZ09999999999",
            label: "Offizielles Bundesbank-Testfixture",
        }],
        sources: CREDITOR_ID_SOURCES,
        domain: Domain::PaymentsSepaReferences,
        roles: ALL_MARKET_ROLES,
        sectors: CROSS_SECTOR,
        capabilities: GENERATE_VALIDATE_PARSE,
        checksum_scheme: Some(ChecksumScheme::Mod97),
        allocation_model: AllocationModel::CentrallyAllocated,
        generation_profiles: &[GenerationProfile::OfficialTestFixture],
        default_profile: Some(GenerationProfile::OfficialTestFixture),
        operations: CREDITOR_ID_OPERATIONS,
    },
    IdentifierDescriptor {
        kind: IdentifierKind::MandateReference,
        slug: "mandate-reference",
        label: "SEPA-Mandatsreferenz",
        description: "Vom Gläubiger vergebene Referenz eines SEPA-Lastschriftmandats.",
        format_description: "Ein bis 35 zulässige SEPA-Zeichen; keine standardisierte Prüfziffer und keine zentrale Vergabe.",
        examples: &[IdentifierExample {
            value: "NRG-MND-01K2T6FMH3K1A4Q9Z5AB12CD34",
            label: "Synthetisches Formatbeispiel",
        }],
        sources: MANDATE_SOURCES,
        domain: Domain::PaymentsSepaReferences,
        roles: ALL_MARKET_ROLES,
        sectors: CROSS_SECTOR,
        capabilities: GENERATE_VALIDATE_PARSE,
        checksum_scheme: None,
        allocation_model: AllocationModel::IssuerAssigned,
        generation_profiles: SYNTAX_ONLY,
        default_profile: Some(GenerationProfile::SyntaxOnly),
        operations: MANDATE_REFERENCE_OPERATIONS,
    },
    IdentifierDescriptor {
        kind: IdentifierKind::EndToEndId,
        slug: "end-to-end-id",
        label: "End-to-End-ID",
        description: "Durchgängige Zahlungsreferenz der initiierenden Partei.",
        format_description: "Ein bis 35 zulässige SEPA-Zeichen ohne Prüfziffer; der explizite Sentinel NOTPROVIDED ist gültig.",
        examples: &[IdentifierExample {
            value: "NOTPROVIDED",
            label: "Expliziter SEPA-Sentinel, nicht Generatorstandard",
        }],
        sources: END_TO_END_SOURCES,
        domain: Domain::PaymentsSepaReferences,
        roles: ALL_MARKET_ROLES,
        sectors: CROSS_SECTOR,
        capabilities: GENERATE_VALIDATE_PARSE,
        checksum_scheme: None,
        allocation_model: AllocationModel::IssuerAssigned,
        generation_profiles: SYNTAX_ONLY,
        default_profile: Some(GenerationProfile::SyntaxOnly),
        operations: END_TO_END_ID_OPERATIONS,
    },
    IdentifierDescriptor {
        kind: IdentifierKind::RfReference,
        slug: "rf-reference",
        label: "RF-Gläubigerreferenz",
        description: "Prüfziffergesicherte strukturierte Referenz zur automatischen Zahlungszuordnung.",
        format_description: "RF, zwei MOD-97-Prüfziffern und bis zu 21 alphanumerische Referenzzeichen; Druckdarstellung in Vierergruppen.",
        examples: &[IdentifierExample {
            value: "RF18539007547034",
            label: "Veröffentlichtes ISO-11649-Formatbeispiel",
        }],
        sources: RF_SOURCES,
        domain: Domain::PaymentsSepaReferences,
        roles: ALL_MARKET_ROLES,
        sectors: CROSS_SECTOR,
        capabilities: GENERATE_VALIDATE_PARSE,
        checksum_scheme: Some(ChecksumScheme::Mod97),
        allocation_model: AllocationModel::SelfAssigned,
        generation_profiles: CHECKSUM_ONLY,
        default_profile: Some(GenerationProfile::ChecksumOnly),
        operations: RF_REFERENCE_OPERATIONS,
    },
    IdentifierDescriptor {
        kind: IdentifierKind::MarketPartnerId,
        slug: "mp-id",
        label: "BDEW-/DVGW-Marktpartner-ID",
        description: "Identifiziert BDEW-Strom- oder DVGW-Gasmarktpartner.",
        format_description: "13 Ziffern mit Herausgeber-/Vergabekennzeichen und abschließender BDEW-Lok-und-Waggon-Prüfziffer; Strom und Gas besitzen getrennte Präfixregeln.",
        examples: &[IdentifierExample {
            value: "9979425000005",
            label: "Im BDEW-Leitfaden veröffentlichter Wert; kann produktiv zugeordnet sein",
        }],
        sources: BDEW_IDENTIFIER_SOURCES,
        domain: Domain::EnergyMarketPartners,
        roles: ALL_MARKET_ROLES,
        sectors: ELECTRICITY_AND_GAS,
        capabilities: GENERATE_VALIDATE,
        checksum_scheme: Some(ChecksumScheme::BdewLokWaggon),
        allocation_model: AllocationModel::CentrallyAllocated,
        generation_profiles: CHECKSUM_ONLY,
        default_profile: Some(GenerationProfile::ChecksumOnly),
        operations: MARKET_PARTNER_OPERATIONS,
    },
    IdentifierDescriptor {
        kind: IdentifierKind::ClusterResourceId,
        slug: "cr-id",
        label: "Cluster-Ressourcen-ID",
        description: "Bündelt technische Ressourcen für Redispatch- und Steuerungsprozesse.",
        format_description: "Elf Zeichen mit Präfix A und abschließendem BDEW-ASCII-Prüfzeichen.",
        examples: &[IdentifierExample {
            value: "A1137355925",
            label: "Offizielles BDEW-Prüfrechenbeispiel; keine Vergabeaussage",
        }],
        sources: BDEW_IDENTIFIER_SOURCES,
        domain: Domain::EnergyResourcesRedispatch,
        roles: RESOURCE_ROLES,
        sectors: ELECTRICITY,
        capabilities: GENERATE_ONLY,
        checksum_scheme: Some(ChecksumScheme::BdewAscii),
        allocation_model: AllocationModel::CentrallyAllocated,
        generation_profiles: CHECKSUM_ONLY,
        default_profile: Some(GenerationProfile::ChecksumOnly),
        operations: CLUSTER_RESOURCE_OPERATIONS,
    },
    IdentifierDescriptor {
        kind: IdentifierKind::SteeringGroupId,
        slug: "sg-id",
        label: "Steuergruppen-ID",
        description: "Kennzeichnet eine Gruppe gemeinsam angesteuerter Ressourcen.",
        format_description: "Elf Zeichen mit Präfix B und abschließendem BDEW-ASCII-Prüfzeichen.",
        examples: &[],
        sources: BDEW_IDENTIFIER_SOURCES,
        domain: Domain::EnergyResourcesRedispatch,
        roles: RESOURCE_ROLES,
        sectors: ELECTRICITY,
        capabilities: GENERATE_ONLY,
        checksum_scheme: Some(ChecksumScheme::BdewAscii),
        allocation_model: AllocationModel::CentrallyAllocated,
        generation_profiles: CHECKSUM_ONLY,
        default_profile: Some(GenerationProfile::ChecksumOnly),
        operations: STEERING_GROUP_OPERATIONS,
    },
    IdentifierDescriptor {
        kind: IdentifierKind::ControllableResourceId,
        slug: "sr-id",
        label: "Steuerbare-Ressourcen-ID",
        description: "Identifiziert eine steuerbare Ressource in energiewirtschaftlichen Prozessen.",
        format_description: "Elf Zeichen mit Präfix C und abschließendem BDEW-ASCII-Prüfzeichen.",
        examples: &[],
        sources: BDEW_IDENTIFIER_SOURCES,
        domain: Domain::EnergyResourcesRedispatch,
        roles: RESOURCE_ROLES,
        sectors: ELECTRICITY,
        capabilities: GENERATE_ONLY,
        checksum_scheme: Some(ChecksumScheme::BdewAscii),
        allocation_model: AllocationModel::CentrallyAllocated,
        generation_profiles: CHECKSUM_ONLY,
        default_profile: Some(GenerationProfile::ChecksumOnly),
        operations: CONTROLLABLE_RESOURCE_OPERATIONS,
    },
    IdentifierDescriptor {
        kind: IdentifierKind::TechnicalResourceId,
        slug: "tr-id",
        label: "Technische-Ressourcen-ID",
        description: "Identifiziert eine technische Ressource für Redispatch und Anlagensteuerung.",
        format_description: "Elf Zeichen mit Präfix D und abschließendem BDEW-ASCII-Prüfzeichen.",
        examples: &[],
        sources: BDEW_IDENTIFIER_SOURCES,
        domain: Domain::EnergyResourcesRedispatch,
        roles: RESOURCE_ROLES,
        sectors: ELECTRICITY,
        capabilities: GENERATE_ONLY,
        checksum_scheme: Some(ChecksumScheme::BdewAscii),
        allocation_model: AllocationModel::CentrallyAllocated,
        generation_profiles: CHECKSUM_ONLY,
        default_profile: Some(GenerationProfile::ChecksumOnly),
        operations: TECHNICAL_RESOURCE_OPERATIONS,
    },
    IdentifierDescriptor {
        kind: IdentifierKind::PackageId,
        slug: "package-id",
        label: "Paket-Identifikationsnummer",
        description: "Kennzeichnet ein Datenpaket für Netzänderungs- und Redispatch-Prozesse.",
        format_description: "Elf Zeichen mit Herausgeberpräfix P9 und abschließendem BDEW-ASCII-Prüfzeichen.",
        examples: &[],
        sources: BDEW_IDENTIFIER_SOURCES,
        domain: Domain::EnergyResourcesRedispatch,
        roles: GRID_OPERATOR_ROLES,
        sectors: ELECTRICITY,
        capabilities: GENERATE_ONLY,
        checksum_scheme: Some(ChecksumScheme::BdewAscii),
        allocation_model: AllocationModel::CentrallyAllocated,
        generation_profiles: CHECKSUM_ONLY,
        default_profile: Some(GenerationProfile::ChecksumOnly),
        operations: PACKAGE_OPERATIONS,
    },
    IdentifierDescriptor {
        kind: IdentifierKind::Uetr,
        slug: "uetr",
        label: "Unique End-to-End Transaction Reference (UETR)",
        description: "UUID-v4-Referenz für eindeutige Zahlungs- und Nachrichtenflüsse.",
        format_description: "36 Zeichen in kanonischer UUID-v4-Schreibweise; Versionsnibble 4 und RFC-4122-Variantennibble 8, 9, a oder b.",
        examples: &[IdentifierExample {
            value: "123e4567-e89b-42d3-a456-426614174000",
            label: "UUID-v4-Formatbeispiel",
        }],
        sources: UETR_SOURCES,
        domain: Domain::PaymentsSepaReferences,
        roles: ALL_MARKET_ROLES,
        sectors: CROSS_SECTOR,
        capabilities: GENERATE_VALIDATE_PARSE,
        checksum_scheme: None,
        allocation_model: AllocationModel::SelfAssigned,
        generation_profiles: SYNTAX_ONLY,
        default_profile: Some(GenerationProfile::SyntaxOnly),
        operations: UETR_OPERATIONS,
    },
    IdentifierDescriptor {
        kind: IdentifierKind::VatId,
        slug: "vat-id",
        label: "Deutsche Umsatzsteuer-Identifikationsnummer",
        description: "Kennung für Rechnungen und Geschäftspartner; Formatprüfung ersetzt keine Abfrage bei BZSt oder VIES.",
        format_description: "DE gefolgt von neun Dezimalziffern. NRG behauptet keine öffentliche deutsche Prüfzifferregel und führt bei der Offline-Validierung keine Registerabfrage aus.",
        examples: &[IdentifierExample {
            value: "DE000000000",
            label: "Reines Formatbeispiel; keine Vergabe- oder Gültigkeitsaussage",
        }],
        sources: VAT_SOURCES,
        domain: Domain::BusinessOrganizations,
        roles: ALL_MARKET_ROLES,
        sectors: CROSS_SECTOR,
        capabilities: VALIDATE_PARSE,
        checksum_scheme: None,
        allocation_model: AllocationModel::DirectoryBacked,
        generation_profiles: &[],
        default_profile: None,
        operations: VAT_ID_OPERATIONS,
    },
    IdentifierDescriptor {
        kind: IdentifierKind::Lei,
        slug: "lei",
        label: "Legal Entity Identifier (LEI)",
        description: "Globale Unternehmenskennung für Finanz- und Energiehandelsaktivitäten.",
        format_description: "20 Großbuchstaben oder Ziffern; vierstelliger Herausgeberpräfix, 14 herausgeberseitige Zeichen und zwei MOD-97-Prüfziffern.",
        examples: &[IdentifierExample {
            value: "506700GE1G29325QX363",
            label: "Veröffentlichter GLEIF-Wert; Registerstatus kann sich ändern",
        }],
        sources: LEI_SOURCES,
        domain: Domain::BusinessOrganizations,
        roles: ALL_MARKET_ROLES,
        sectors: CROSS_SECTOR,
        capabilities: VALIDATE_PARSE_LOOKUP,
        checksum_scheme: Some(ChecksumScheme::Mod97),
        allocation_model: AllocationModel::DirectoryBacked,
        generation_profiles: &[],
        default_profile: None,
        operations: LEI_OPERATIONS,
    },
    IdentifierDescriptor {
        kind: IdentifierKind::Mastr,
        slug: "mastr",
        label: "Marktstammdatenregister-Kennung (MaStR)",
        description: "Objekt- und Rollenkennung des Marktstammdatenregisters.",
        format_description: "Dreistelliger Objekttyp-Präfix, Versions-/Zufallskörper und EAN-Mod-10-Prüfziffer; bei zulässigen Rollen optional ein zweistelliges Rollensuffix.",
        examples: &[IdentifierExample {
            value: "ABR919283764526",
            label: "Im MaStR-Nummernkonzept veröffentlichtes Beispiel",
        }],
        sources: MASTR_SOURCES,
        domain: Domain::EnergyRegistersAssets,
        roles: ALL_MARKET_ROLES,
        sectors: ELECTRICITY_AND_GAS,
        capabilities: GENERATE_VALIDATE_PARSE,
        checksum_scheme: Some(ChecksumScheme::EanMod10),
        allocation_model: AllocationModel::CentrallyAllocated,
        generation_profiles: CHECKSUM_ONLY,
        default_profile: Some(GenerationProfile::ChecksumOnly),
        operations: MASTR_OPERATIONS,
    },
    IdentifierDescriptor {
        kind: IdentifierKind::Eic,
        slug: "eic",
        label: "Energy Identification Code (EIC)",
        description: "Europäische Energiekennung für Parteien, Gebiete, Messpunkte und Ressourcen.",
        format_description: "16 Zeichen: zweistelliger LIO-Code, Objektart, zwölf lokale Zeichen und ein EIC-Prüfzeichen.",
        examples: &[IdentifierExample {
            value: "10X---ENTSOE---L",
            label: "Offizielles ENTSO-E-Beispiel",
        }],
        sources: EIC_SOURCES,
        domain: Domain::EnergyRegistersAssets,
        roles: ALL_MARKET_ROLES,
        sectors: ELECTRICITY_AND_GAS,
        capabilities: VALIDATE_PARSE_LOOKUP,
        checksum_scheme: Some(ChecksumScheme::EicCheckCharacter),
        allocation_model: AllocationModel::CentrallyAllocated,
        generation_profiles: &[],
        default_profile: None,
        operations: EIC_OPERATIONS,
    },
    IdentifierDescriptor {
        kind: IdentifierKind::Obis,
        slug: "obis",
        label: "OBIS-Kennzahl",
        description: "Strukturierte Kennzahl für Messgrößen und Registerwerte.",
        format_description: "Sechs Wertebereiche A bis F in vollständiger, reduzierter oder sechs Byte langer Logical-Name-Darstellung; keine Prüfziffer.",
        examples: &[IdentifierExample {
            value: "1-0:1.8.0*255",
            label: "Gebräuchlicher Strom-Bezugszählerstand",
        }],
        sources: OBIS_SOURCES,
        domain: Domain::MeteringDevicesValues,
        roles: LOCATION_ROLES,
        sectors: ELECTRICITY_AND_GAS,
        capabilities: VALIDATE_PARSE_LOOKUP,
        checksum_scheme: None,
        allocation_model: AllocationModel::NotApplicable,
        generation_profiles: &[],
        default_profile: None,
        operations: OBIS_OPERATIONS,
    },
    IdentifierDescriptor {
        kind: IdentifierKind::Din43849,
        slug: "din-43849",
        label: "DIN-43849-Gerätekennung",
        description: "Herstellerübergreifende Gerätekennung nach der öffentlich belegten DIN-43849-Struktur.",
        format_description: "14 elektronische Zeichen: ein Gerätekategoriezeichen, drei Herstellerbuchstaben, zweistelliger Fertigungsblock und achtstellige Fertigungsnummer; keine öffentlich belegte Prüfziffer.",
        examples: &[IdentifierExample {
            value: "7QDS0111223344",
            label: "In der OMS-Spezifikation veröffentlichtes Strukturbeispiel",
        }],
        sources: DIN_43849_SOURCES,
        domain: Domain::MeteringDevicesValues,
        roles: &[MarketRole::MeteringPointOperator],
        sectors: CROSS_SECTOR,
        capabilities: VALIDATE_PARSE,
        checksum_scheme: None,
        allocation_model: AllocationModel::IssuerAssigned,
        generation_profiles: &[],
        default_profile: None,
        operations: DIN_43849_OPERATIONS,
    },
];

pub const fn identifier_catalog() -> &'static [IdentifierDescriptor] {
    IDENTIFIER_CATALOG
}

pub const fn service_operations() -> &'static [ServiceOperationDescriptor] {
    SERVICE_OPERATIONS
}

pub fn descriptor(kind: IdentifierKind) -> Option<&'static IdentifierDescriptor> {
    IDENTIFIER_CATALOG.iter().find(|entry| entry.kind == kind)
}

pub fn descriptor_by_slug(slug: &str) -> Option<&'static IdentifierDescriptor> {
    IDENTIFIER_CATALOG.iter().find(|entry| entry.slug == slug)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn catalog_slugs_and_kinds_are_unique() {
        let mut slugs = HashSet::new();
        let mut kinds = HashSet::new();

        for entry in IDENTIFIER_CATALOG {
            assert!(slugs.insert(entry.slug), "duplicate slug: {}", entry.slug);
            assert!(kinds.insert(entry.kind), "duplicate kind: {:?}", entry.kind);
            assert_eq!(entry.slug, entry.kind.as_str());
            assert!(
                !entry.description.trim().is_empty(),
                "{} has no description",
                entry.slug
            );
            assert!(
                !entry.format_description.trim().is_empty(),
                "{} has no format description",
                entry.slug
            );
            assert!(!entry.sources.is_empty(), "{} has no source", entry.slug);
            for source in entry.sources {
                assert!(
                    !source.label.trim().is_empty(),
                    "{} source label",
                    entry.slug
                );
                assert!(
                    source.url.starts_with("https://"),
                    "{} has a non-HTTPS source",
                    entry.slug
                );
            }
            for example in entry.examples {
                assert!(
                    !example.value.trim().is_empty(),
                    "{} example value",
                    entry.slug
                );
                assert!(
                    !example.label.trim().is_empty(),
                    "{} example label",
                    entry.slug
                );
            }
        }
    }

    #[test]
    fn operation_ids_and_method_paths_are_unique() {
        let mut operation_ids = HashSet::new();
        let mut method_paths = HashSet::new();

        let identifier_operations = IDENTIFIER_CATALOG.iter().flat_map(|entry| entry.operations);
        let service_operations = SERVICE_OPERATIONS.iter().map(|entry| &entry.operation);
        for operation in identifier_operations.chain(service_operations) {
            assert!(
                operation_ids.insert(operation.operation_id),
                "duplicate operationId: {}",
                operation.operation_id
            );
            assert!(
                method_paths.insert((operation.method, operation.path)),
                "duplicate API operation: {:?} {}",
                operation.method,
                operation.path
            );
        }
    }

    #[test]
    fn service_operations_have_complete_facets_and_consistent_profiles() {
        for service in SERVICE_OPERATIONS {
            let operation = &service.operation;
            assert!(!operation.path.is_empty());
            assert!(!operation.operation_id.is_empty());
            assert!(!operation.primary_tag.is_empty());
            if operation.capability != Capability::List {
                assert!(
                    !service.roles.is_empty(),
                    "{} has no roles",
                    operation.operation_id
                );
                assert!(
                    !service.sectors.is_empty(),
                    "{} has no sectors",
                    operation.operation_id
                );
            }
        }
    }

    #[test]
    fn descriptors_have_consistent_operations_and_profiles() {
        for entry in IDENTIFIER_CATALOG {
            assert!(
                !entry.operations.is_empty(),
                "{} has no operation",
                entry.slug
            );
            assert!(!entry.roles.is_empty(), "{} has no role facet", entry.slug);
            assert!(
                !entry.sectors.is_empty(),
                "{} has no sector facet",
                entry.slug
            );

            if let Some(profile) = entry.default_profile {
                assert!(
                    entry.supports_profile(profile),
                    "{} has an unsupported default profile",
                    entry.slug
                );
            }

            for operation in entry.operations {
                assert!(
                    !operation.primary_tag.trim().is_empty(),
                    "{} has no primary tag",
                    operation.operation_id
                );
                assert!(
                    entry.supports(operation.capability),
                    "{} exposes a capability absent from {}",
                    operation.operation_id,
                    entry.slug
                );
            }
        }
    }

    #[test]
    fn legacy_location_operations_are_exactly_the_six_deprecated_aliases() {
        let legacy: HashSet<_> = IDENTIFIER_CATALOG
            .iter()
            .flat_map(|entry| entry.operations)
            .filter(|operation| operation.deprecated)
            .map(|operation| (operation.method, operation.path))
            .collect();

        assert_eq!(
            legacy,
            HashSet::from([
                (ApiMethod::Get, "/api/malo/generate"),
                (ApiMethod::Post, "/api/malo/validate"),
                (ApiMethod::Get, "/api/melo/generate"),
                (ApiMethod::Post, "/api/melo/validate"),
                (ApiMethod::Get, "/api/nelo/generate"),
                (ApiMethod::Post, "/api/nelo/validate"),
            ])
        );
    }

    #[test]
    fn generation_count_is_bounded() {
        let mut request = GenerateRequest::default();
        assert_eq!(request.validated_count(), Ok(1));

        request.count = 0;
        assert!(request.validated_count().is_err());

        request.count = 100;
        assert_eq!(request.validated_count(), Ok(100));

        request.count = 101;
        assert!(request.validated_count().is_err());
    }

    #[test]
    fn sector_facets_distinguish_electricity_only_and_cross_sector_identifiers() {
        assert_eq!(
            descriptor(IdentifierKind::Nelo).unwrap().sectors,
            ELECTRICITY
        );
        assert_eq!(
            descriptor(IdentifierKind::Nebe).unwrap().sectors,
            ELECTRICITY
        );
        assert_eq!(
            descriptor(IdentifierKind::PackageId).unwrap().sectors,
            ELECTRICITY
        );

        for kind in [
            IdentifierKind::Iban,
            IdentifierKind::Bic,
            IdentifierKind::CreditorId,
            IdentifierKind::MandateReference,
            IdentifierKind::EndToEndId,
            IdentifierKind::RfReference,
        ] {
            assert_eq!(descriptor(kind).unwrap().sectors, CROSS_SECTOR);
        }
    }
}
