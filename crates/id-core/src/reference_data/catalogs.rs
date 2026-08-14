//! Reviewed, machine-readable formation-rule catalogs.
//!
//! These files describe published identifier rules and prefix vocabularies.
//! They deliberately contain no registry allocations and cannot establish
//! whether a syntactically valid identifier has actually been assigned.

use std::{collections::BTreeSet, error::Error, fmt, sync::OnceLock};

use serde::{Deserialize, Serialize};

pub const BDEW_IDENTIFIERS_NAME: &str = "bdew_identifiers";
pub const BDEW_IDENTIFIERS_SCHEMA_VERSION: u16 = 1;
pub const BDEW_IDENTIFIERS_VERSION: &str = "1.2";
pub const BDEW_IDENTIFIERS_PUBLISHED: &str = "2025-02-07";
pub const BDEW_IDENTIFIERS_CHECKED_AT: &str = "2026-08-14";
pub const BDEW_IDENTIFIERS_SOURCE_URL: &str = "https://www.bdew.de/media/documents/AWH_Identifikatoren-in-der-Marktkommunikation_Version.1.2.pdf";
pub const BDEW_IDENTIFIERS_SOURCE_SHA256: &str =
    "8864853a19008c82267827436d5393b42dea7e3a44b3c7e4cdeecc8f92379820";
pub const BDEW_IDENTIFIERS_DATA_SHA256: &str =
    "1ad9317c886356364dd47ff6a152fdec375f21e9d5f8b61a68a459ab38a98690";

pub const MASTR_PREFIXES_NAME: &str = "mastr_prefixes";
pub const MASTR_PREFIXES_SCHEMA_VERSION: u16 = 1;
pub const MASTR_PREFIXES_VERSION: &str = "2019-05";
pub const MASTR_PREFIXES_PUBLISHED: &str = "2019-05";
pub const MASTR_PREFIXES_CHECKED_AT: &str = "2026-08-14";
pub const MASTR_PREFIXES_SOURCE_URL: &str =
    "https://www.marktstammdatenregister.de/MaStRHilfe/files/regHilfen/MaStR-Nummernkonzept.pdf";
pub const MASTR_PREFIXES_SOURCE_SHA256: &str =
    "e2154964b260c5d53274c065ae873114eb048d421bbba65a702f0ffbc56ba01c";
pub const MASTR_PREFIXES_DATA_SHA256: &str =
    "8e157d75ccc9e906235874b57d447d2fd0e0278a4e1bad142ab6f150fb61f2f0";

const BDEW_IDENTIFIERS_JSON: &str = include_str!("../../../../data/bdew_identifiers_v1.2.json");
const MASTR_PREFIXES_JSON: &str = include_str!("../../../../data/mastr_prefixes_2019-05.json");

/// Stable metadata that API and OpenAPI layers can expose without reparsing
/// the detailed rule catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ReferenceCatalogMetadata {
    pub name: &'static str,
    pub schema_version: u16,
    pub version: &'static str,
    pub published: &'static str,
    pub checked_at: &'static str,
    pub source_url: &'static str,
    /// SHA-256 of the exact canonical JSON bytes embedded by `id-core`.
    pub sha256: &'static str,
    /// SHA-256 of the reviewed upstream source document at `checked_at`.
    pub source_sha256: &'static str,
    pub contains_allocations: bool,
}

pub const BDEW_IDENTIFIERS_METADATA: ReferenceCatalogMetadata = ReferenceCatalogMetadata {
    name: BDEW_IDENTIFIERS_NAME,
    schema_version: BDEW_IDENTIFIERS_SCHEMA_VERSION,
    version: BDEW_IDENTIFIERS_VERSION,
    published: BDEW_IDENTIFIERS_PUBLISHED,
    checked_at: BDEW_IDENTIFIERS_CHECKED_AT,
    source_url: BDEW_IDENTIFIERS_SOURCE_URL,
    sha256: BDEW_IDENTIFIERS_DATA_SHA256,
    source_sha256: BDEW_IDENTIFIERS_SOURCE_SHA256,
    contains_allocations: false,
};

pub const MASTR_PREFIXES_METADATA: ReferenceCatalogMetadata = ReferenceCatalogMetadata {
    name: MASTR_PREFIXES_NAME,
    schema_version: MASTR_PREFIXES_SCHEMA_VERSION,
    version: MASTR_PREFIXES_VERSION,
    published: MASTR_PREFIXES_PUBLISHED,
    checked_at: MASTR_PREFIXES_CHECKED_AT,
    source_url: MASTR_PREFIXES_SOURCE_URL,
    sha256: MASTR_PREFIXES_DATA_SHA256,
    source_sha256: MASTR_PREFIXES_SOURCE_SHA256,
    contains_allocations: false,
};

pub const RULE_REFERENCE_CATALOGS: &[ReferenceCatalogMetadata] =
    &[BDEW_IDENTIFIERS_METADATA, MASTR_PREFIXES_METADATA];

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BdewIdentifiersSnapshot {
    pub schema_version: u16,
    pub name: String,
    pub authority: String,
    pub version: String,
    pub published: String,
    pub checked_at: String,
    pub source_url: String,
    pub source_sha256: String,
    pub contains_allocations: bool,
    pub rules: Vec<BdewIdentifierRule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BdewIdentifierRule {
    pub kind: BdewIdentifierKind,
    pub label_de: String,
    pub prefix: String,
    pub total_length: usize,
    pub body_character_set: BdewCharacterSet,
    pub checksum_scheme: BdewChecksumScheme,
    pub checksum_position: usize,
    pub allocation_mode: Option<BdewAllocationModeRule>,
    pub source_sections: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BdewIdentifierKind {
    ClusterResource,
    ControlGroup,
    ControllableResource,
    MarketPartnerBdew,
    MarketPartnerDvgw,
    NetworkArea,
    Package,
    TechnicalResource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BdewCharacterSet {
    Numeric,
    UppercaseAlphanumeric,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BdewChecksumScheme {
    BdewLokWaggon,
    BdewAscii,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BdewAllocationModeRule {
    pub position: usize,
    pub minimum_digit: u8,
    pub maximum_digit: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MastrPrefixesSnapshot {
    pub schema_version: u16,
    pub name: String,
    pub authority: String,
    pub version: String,
    pub published: String,
    pub checked_at: String,
    pub source_url: String,
    pub source_sha256: String,
    pub contains_allocations: bool,
    pub identifier_structure: MastrIdentifierStructure,
    pub prefixes: Vec<MastrPrefixRecord>,
    pub role_suffixes: Vec<MastrRoleSuffixRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MastrIdentifierStructure {
    pub prefix_length: usize,
    pub version_digit: u8,
    pub random_digit_count: usize,
    pub checksum_scheme: MastrChecksumScheme,
    pub role_suffix_length: usize,
    pub allocation_model: MastrAllocationModel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MastrChecksumScheme {
    EanMod10,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MastrAllocationModel {
    Central,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MastrPrefixRecord {
    pub code: String,
    pub label_de: String,
    pub sector: MastrCatalogSector,
    pub object_group: MastrCatalogObjectGroup,
    pub lifecycle: MastrCatalogLifecycle,
    pub role_suffixes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MastrCatalogSector {
    Electricity,
    Gas,
    CrossSector,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MastrCatalogObjectGroup {
    UnitGroupingOrApproval,
    MarketParticipant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MastrCatalogLifecycle {
    Current,
    LegacyMigratedUnit,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MastrRoleSuffixRecord {
    pub code: String,
    pub label_de: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceCatalogError(String);

impl ReferenceCatalogError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for ReferenceCatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for ReferenceCatalogError {}

static BDEW_IDENTIFIERS: OnceLock<Result<BdewIdentifiersSnapshot, ReferenceCatalogError>> =
    OnceLock::new();
static MASTR_PREFIXES: OnceLock<Result<MastrPrefixesSnapshot, ReferenceCatalogError>> =
    OnceLock::new();

pub fn bdew_identifiers() -> Result<&'static BdewIdentifiersSnapshot, ReferenceCatalogError> {
    BDEW_IDENTIFIERS
        .get_or_init(|| {
            let snapshot = parse_bdew_identifiers(BDEW_IDENTIFIERS_JSON.as_bytes())?;
            verify_embedded_bdew_metadata(&snapshot)?;
            Ok(snapshot)
        })
        .as_ref()
        .map_err(Clone::clone)
}

pub fn mastr_prefixes() -> Result<&'static MastrPrefixesSnapshot, ReferenceCatalogError> {
    MASTR_PREFIXES
        .get_or_init(|| {
            let snapshot = parse_mastr_prefixes(MASTR_PREFIXES_JSON.as_bytes())?;
            verify_embedded_mastr_metadata(&snapshot)?;
            Ok(snapshot)
        })
        .as_ref()
        .map_err(Clone::clone)
}

pub fn parse_bdew_identifiers(
    bytes: &[u8],
) -> Result<BdewIdentifiersSnapshot, ReferenceCatalogError> {
    let snapshot: BdewIdentifiersSnapshot = serde_json::from_slice(bytes)
        .map_err(|error| ReferenceCatalogError::new(format!("invalid BDEW JSON: {error}")))?;
    validate_bdew_identifiers(&snapshot)?;
    Ok(snapshot)
}

pub fn parse_mastr_prefixes(bytes: &[u8]) -> Result<MastrPrefixesSnapshot, ReferenceCatalogError> {
    let snapshot: MastrPrefixesSnapshot = serde_json::from_slice(bytes)
        .map_err(|error| ReferenceCatalogError::new(format!("invalid MaStR JSON: {error}")))?;
    validate_mastr_prefixes(&snapshot)?;
    Ok(snapshot)
}

pub fn validate_bdew_identifiers(
    snapshot: &BdewIdentifiersSnapshot,
) -> Result<(), ReferenceCatalogError> {
    validate_common_metadata(CommonMetadata {
        schema_version: snapshot.schema_version,
        name: &snapshot.name,
        expected_name: BDEW_IDENTIFIERS_NAME,
        authority: &snapshot.authority,
        expected_authority: "BDEW Bundesverband der Energie- und Wasserwirtschaft e. V.",
        version: &snapshot.version,
        published: &snapshot.published,
        checked_at: &snapshot.checked_at,
        source_url: &snapshot.source_url,
        source_sha256: &snapshot.source_sha256,
        contains_allocations: snapshot.contains_allocations,
    })?;

    const EXPECTED: &[(
        BdewIdentifierKind,
        &str,
        usize,
        BdewCharacterSet,
        BdewChecksumScheme,
    )] = &[
        (
            BdewIdentifierKind::ClusterResource,
            "A",
            11,
            BdewCharacterSet::UppercaseAlphanumeric,
            BdewChecksumScheme::BdewAscii,
        ),
        (
            BdewIdentifierKind::ControlGroup,
            "B",
            11,
            BdewCharacterSet::UppercaseAlphanumeric,
            BdewChecksumScheme::BdewAscii,
        ),
        (
            BdewIdentifierKind::ControllableResource,
            "C",
            11,
            BdewCharacterSet::UppercaseAlphanumeric,
            BdewChecksumScheme::BdewAscii,
        ),
        (
            BdewIdentifierKind::MarketPartnerBdew,
            "99",
            13,
            BdewCharacterSet::Numeric,
            BdewChecksumScheme::BdewLokWaggon,
        ),
        (
            BdewIdentifierKind::MarketPartnerDvgw,
            "98",
            13,
            BdewCharacterSet::Numeric,
            BdewChecksumScheme::BdewLokWaggon,
        ),
        (
            BdewIdentifierKind::NetworkArea,
            "F",
            11,
            BdewCharacterSet::UppercaseAlphanumeric,
            BdewChecksumScheme::BdewAscii,
        ),
        (
            BdewIdentifierKind::Package,
            "P9",
            11,
            BdewCharacterSet::UppercaseAlphanumeric,
            BdewChecksumScheme::BdewAscii,
        ),
        (
            BdewIdentifierKind::TechnicalResource,
            "D",
            11,
            BdewCharacterSet::UppercaseAlphanumeric,
            BdewChecksumScheme::BdewAscii,
        ),
    ];
    if snapshot.rules.len() != EXPECTED.len() {
        return Err(ReferenceCatalogError::new(format!(
            "BDEW catalog must contain {} implemented rules, got {}",
            EXPECTED.len(),
            snapshot.rules.len()
        )));
    }
    for (index, (rule, expected)) in snapshot.rules.iter().zip(EXPECTED).enumerate() {
        let (kind, prefix, length, character_set, checksum) = *expected;
        if rule.kind != kind
            || rule.prefix != prefix
            || rule.total_length != length
            || rule.body_character_set != character_set
            || rule.checksum_scheme != checksum
            || rule.checksum_position != length
        {
            return Err(ReferenceCatalogError::new(format!(
                "BDEW rule {} does not match the reviewed formation rule",
                index + 1
            )));
        }
        if rule.label_de.trim().is_empty() || rule.source_sections.is_empty() {
            return Err(ReferenceCatalogError::new(format!(
                "BDEW rule {:?} requires a label and source sections",
                rule.kind
            )));
        }
        if !strictly_sorted(rule.source_sections.iter().map(String::as_str))
            || !rule.source_sections.iter().all(|section| {
                section
                    .split('.')
                    .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
            })
        {
            return Err(ReferenceCatalogError::new(format!(
                "BDEW rule {:?} source sections must be unique, sorted numeric section paths",
                rule.kind
            )));
        }
        if !rule
            .prefix
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
        {
            return Err(ReferenceCatalogError::new(format!(
                "BDEW rule {:?} has an invalid prefix",
                rule.kind
            )));
        }
        validate_bdew_allocation_mode(rule)?;
    }
    Ok(())
}

fn validate_bdew_allocation_mode(rule: &BdewIdentifierRule) -> Result<(), ReferenceCatalogError> {
    let expected = match rule.kind {
        BdewIdentifierKind::MarketPartnerBdew => Some((3, 0, 8)),
        BdewIdentifierKind::MarketPartnerDvgw => Some((3, 0, 9)),
        _ => None,
    };
    let actual = rule
        .allocation_mode
        .map(|mode| (mode.position, mode.minimum_digit, mode.maximum_digit));
    if actual != expected {
        return Err(ReferenceCatalogError::new(format!(
            "BDEW rule {:?} has an invalid allocation-mode range",
            rule.kind
        )));
    }
    Ok(())
}

pub fn validate_mastr_prefixes(
    snapshot: &MastrPrefixesSnapshot,
) -> Result<(), ReferenceCatalogError> {
    validate_common_metadata(CommonMetadata {
        schema_version: snapshot.schema_version,
        name: &snapshot.name,
        expected_name: MASTR_PREFIXES_NAME,
        authority: &snapshot.authority,
        expected_authority: "Bundesnetzagentur",
        version: &snapshot.version,
        published: &snapshot.published,
        checked_at: &snapshot.checked_at,
        source_url: &snapshot.source_url,
        source_sha256: &snapshot.source_sha256,
        contains_allocations: snapshot.contains_allocations,
    })?;
    let structure = &snapshot.identifier_structure;
    if structure.prefix_length != 3
        || structure.version_digit != 9
        || structure.random_digit_count != 10
        || structure.checksum_scheme != MastrChecksumScheme::EanMod10
        || structure.role_suffix_length != 2
        || structure.allocation_model != MastrAllocationModel::Central
    {
        return Err(ReferenceCatalogError::new(
            "MaStR identifier_structure does not match the reviewed number concept",
        ));
    }
    if snapshot.prefixes.len() != 27 || snapshot.role_suffixes.len() != 19 {
        return Err(ReferenceCatalogError::new(format!(
            "MaStR catalog must contain 27 prefixes and 19 role suffixes, got {} and {}",
            snapshot.prefixes.len(),
            snapshot.role_suffixes.len()
        )));
    }

    let role_codes: BTreeSet<_> = snapshot
        .role_suffixes
        .iter()
        .map(|role| role.code.as_str())
        .collect();
    if role_codes.len() != snapshot.role_suffixes.len()
        || !strictly_sorted(snapshot.role_suffixes.iter().map(|role| role.code.as_str()))
    {
        return Err(ReferenceCatalogError::new(
            "MaStR role suffixes must be unique and sorted by code",
        ));
    }
    for role in &snapshot.role_suffixes {
        validate_upper_code(&role.code, 2, "MaStR role suffix")?;
        if role.label_de.trim().is_empty() {
            return Err(ReferenceCatalogError::new(format!(
                "MaStR role suffix {} has an empty label",
                role.code
            )));
        }
    }

    if !strictly_sorted(snapshot.prefixes.iter().map(|prefix| prefix.code.as_str())) {
        return Err(ReferenceCatalogError::new(
            "MaStR prefixes must be unique and sorted by code",
        ));
    }
    let prefix_codes: BTreeSet<_> = snapshot
        .prefixes
        .iter()
        .map(|prefix| prefix.code.as_str())
        .collect();
    if prefix_codes.len() != snapshot.prefixes.len() {
        return Err(ReferenceCatalogError::new("duplicate MaStR prefix"));
    }
    for prefix in &snapshot.prefixes {
        validate_upper_code(&prefix.code, 3, "MaStR prefix")?;
        if prefix.label_de.trim().is_empty() {
            return Err(ReferenceCatalogError::new(format!(
                "MaStR prefix {} has an empty label",
                prefix.code
            )));
        }
        if !strictly_sorted(prefix.role_suffixes.iter().map(String::as_str)) {
            return Err(ReferenceCatalogError::new(format!(
                "MaStR role suffixes for {} must be unique and sorted",
                prefix.code
            )));
        }
        for suffix in &prefix.role_suffixes {
            if !role_codes.contains(suffix.as_str()) {
                return Err(ReferenceCatalogError::new(format!(
                    "MaStR prefix {} refers to unknown role suffix {}",
                    prefix.code, suffix
                )));
            }
        }
    }
    validate_mastr_against_runtime(snapshot)?;
    Ok(())
}

fn validate_mastr_against_runtime(
    snapshot: &MastrPrefixesSnapshot,
) -> Result<(), ReferenceCatalogError> {
    use crate::identifiers::registers::mastr::{
        MastrLifecycle, MastrObjectGroup, MastrPrefix, MastrRoleSuffix, MastrSector,
        MASTR_PREFIXES as RUNTIME_PREFIXES,
    };

    if snapshot.prefixes.len() != RUNTIME_PREFIXES.len() {
        return Err(ReferenceCatalogError::new(
            "MaStR prefix vocabulary differs from the runtime validator",
        ));
    }
    for record in &snapshot.prefixes {
        let runtime = MastrPrefix::from_code(&record.code).ok_or_else(|| {
            ReferenceCatalogError::new(format!(
                "MaStR catalog prefix {} is not supported by the runtime validator",
                record.code
            ))
        })?;
        let expected_sector = match runtime.sector() {
            MastrSector::Electricity => MastrCatalogSector::Electricity,
            MastrSector::Gas => MastrCatalogSector::Gas,
            MastrSector::CrossSector => MastrCatalogSector::CrossSector,
        };
        let expected_group = match runtime.object_group() {
            MastrObjectGroup::UnitGroupingOrApproval => {
                MastrCatalogObjectGroup::UnitGroupingOrApproval
            }
            MastrObjectGroup::MarketParticipant => MastrCatalogObjectGroup::MarketParticipant,
        };
        let expected_lifecycle = match runtime.lifecycle() {
            MastrLifecycle::Current => MastrCatalogLifecycle::Current,
            MastrLifecycle::LegacyMigratedUnit => MastrCatalogLifecycle::LegacyMigratedUnit,
        };
        let expected_roles: BTreeSet<_> = runtime
            .allowed_role_suffixes()
            .iter()
            .map(|role| role.code())
            .collect();
        let actual_roles: BTreeSet<_> = record.role_suffixes.iter().map(String::as_str).collect();
        if record.label_de != runtime.label_de()
            || record.sector != expected_sector
            || record.object_group != expected_group
            || record.lifecycle != expected_lifecycle
            || actual_roles != expected_roles
        {
            return Err(ReferenceCatalogError::new(format!(
                "MaStR catalog metadata for {} differs from the runtime validator",
                record.code
            )));
        }
    }
    if snapshot
        .role_suffixes
        .iter()
        .any(|record| MastrRoleSuffix::from_code(&record.code).is_none())
    {
        return Err(ReferenceCatalogError::new(
            "MaStR role vocabulary differs from the runtime validator",
        ));
    }
    Ok(())
}

struct CommonMetadata<'a> {
    schema_version: u16,
    name: &'a str,
    expected_name: &'a str,
    authority: &'a str,
    expected_authority: &'a str,
    version: &'a str,
    published: &'a str,
    checked_at: &'a str,
    source_url: &'a str,
    source_sha256: &'a str,
    contains_allocations: bool,
}

fn validate_common_metadata(metadata: CommonMetadata<'_>) -> Result<(), ReferenceCatalogError> {
    if metadata.schema_version != 1 {
        return Err(ReferenceCatalogError::new(format!(
            "unsupported reference schema version {}",
            metadata.schema_version
        )));
    }
    if metadata.name != metadata.expected_name
        || metadata.authority != metadata.expected_authority
        || metadata.version.is_empty()
        || !metadata
            .version
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    {
        return Err(ReferenceCatalogError::new(
            "reference name, authority or version is invalid",
        ));
    }
    validate_date_or_month(metadata.published, "published")?;
    validate_date(metadata.checked_at, "checked_at")?;
    if metadata
        .checked_at
        .get(..7)
        .is_none_or(|checked_month| checked_month < &metadata.published[..7])
    {
        return Err(ReferenceCatalogError::new(
            "checked_at must not be earlier than published",
        ));
    }
    if !metadata.source_url.starts_with("https://") {
        return Err(ReferenceCatalogError::new(
            "reference source_url must use HTTPS",
        ));
    }
    validate_sha256(metadata.source_sha256, "source_sha256")?;
    if metadata.contains_allocations {
        return Err(ReferenceCatalogError::new(
            "formation-rule catalogs must not contain registry allocations",
        ));
    }
    Ok(())
}

fn validate_date_or_month(value: &str, field: &str) -> Result<(), ReferenceCatalogError> {
    match value.len() {
        7 => {
            let bytes = value.as_bytes();
            if bytes[4] != b'-'
                || !bytes[..4].iter().all(u8::is_ascii_digit)
                || !bytes[5..].iter().all(u8::is_ascii_digit)
                || !("01"..="12").contains(&&value[5..])
            {
                return Err(ReferenceCatalogError::new(format!(
                    "{field} must use YYYY-MM or YYYY-MM-DD"
                )));
            }
            Ok(())
        }
        10 => validate_date(value, field),
        _ => Err(ReferenceCatalogError::new(format!(
            "{field} must use YYYY-MM or YYYY-MM-DD"
        ))),
    }
}

fn validate_date(value: &str, field: &str) -> Result<(), ReferenceCatalogError> {
    let bytes = value.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
    {
        return Err(ReferenceCatalogError::new(format!(
            "{field} must use YYYY-MM-DD"
        )));
    }
    let year: u32 = value[..4]
        .parse()
        .map_err(|_| ReferenceCatalogError::new(format!("{field} has an invalid year")))?;
    let month: u32 = value[5..7]
        .parse()
        .map_err(|_| ReferenceCatalogError::new(format!("{field} has an invalid month")))?;
    let day: u32 = value[8..10]
        .parse()
        .map_err(|_| ReferenceCatalogError::new(format!("{field} has an invalid day")))?;
    let leap_year =
        year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let maximum_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap_year => 29,
        2 => 28,
        _ => 0,
    };
    if day == 0 || day > maximum_day {
        return Err(ReferenceCatalogError::new(format!(
            "{field} is not a calendar date"
        )));
    }
    Ok(())
}

fn validate_sha256(value: &str, field: &str) -> Result<(), ReferenceCatalogError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ReferenceCatalogError::new(format!(
            "{field} must contain 64 lowercase hexadecimal characters"
        )));
    }
    Ok(())
}

fn validate_upper_code(
    value: &str,
    expected_length: usize,
    field: &str,
) -> Result<(), ReferenceCatalogError> {
    if value.len() != expected_length || !value.bytes().all(|byte| byte.is_ascii_uppercase()) {
        return Err(ReferenceCatalogError::new(format!(
            "{field} must contain {expected_length} uppercase ASCII letters"
        )));
    }
    Ok(())
}

fn strictly_sorted<'a>(values: impl IntoIterator<Item = &'a str>) -> bool {
    let mut previous: Option<&str> = None;
    for value in values {
        if previous.is_some_and(|previous| previous >= value) {
            return false;
        }
        previous = Some(value);
    }
    true
}

fn verify_embedded_bdew_metadata(
    snapshot: &BdewIdentifiersSnapshot,
) -> Result<(), ReferenceCatalogError> {
    if snapshot.schema_version != BDEW_IDENTIFIERS_SCHEMA_VERSION
        || snapshot.version != BDEW_IDENTIFIERS_VERSION
        || snapshot.published != BDEW_IDENTIFIERS_PUBLISHED
        || snapshot.checked_at != BDEW_IDENTIFIERS_CHECKED_AT
        || snapshot.source_url != BDEW_IDENTIFIERS_SOURCE_URL
        || snapshot.source_sha256 != BDEW_IDENTIFIERS_SOURCE_SHA256
    {
        return Err(ReferenceCatalogError::new(
            "embedded BDEW metadata does not match its public constants",
        ));
    }
    Ok(())
}

fn verify_embedded_mastr_metadata(
    snapshot: &MastrPrefixesSnapshot,
) -> Result<(), ReferenceCatalogError> {
    if snapshot.schema_version != MASTR_PREFIXES_SCHEMA_VERSION
        || snapshot.version != MASTR_PREFIXES_VERSION
        || snapshot.published != MASTR_PREFIXES_PUBLISHED
        || snapshot.checked_at != MASTR_PREFIXES_CHECKED_AT
        || snapshot.source_url != MASTR_PREFIXES_SOURCE_URL
        || snapshot.source_sha256 != MASTR_PREFIXES_SOURCE_SHA256
    {
        return Err(ReferenceCatalogError::new(
            "embedded MaStR metadata does not match its public constants",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identifiers::{
        energy::{
            market_partner::MarketPartnerIdKind, network::NetworkIdentifierKind,
            resource::ResourceIdKind,
        },
        registers::mastr::{MastrPrefix, MASTR_PREFIXES as RUNTIME_MASTR_PREFIXES},
    };

    #[test]
    fn embedded_catalogs_are_strict_and_allocation_free() {
        let bdew = bdew_identifiers().unwrap();
        let mastr = mastr_prefixes().unwrap();
        assert_eq!(bdew.rules.len(), 8);
        assert_eq!(mastr.prefixes.len(), 27);
        assert_eq!(mastr.role_suffixes.len(), 19);
        assert!(!bdew.contains_allocations);
        assert!(!mastr.contains_allocations);
        assert!(RULE_REFERENCE_CATALOGS
            .iter()
            .all(|metadata| !metadata.contains_allocations));
    }

    #[test]
    fn bdew_catalog_matches_runtime_prefixes() {
        let snapshot = bdew_identifiers().unwrap();
        let prefix = |kind| {
            snapshot
                .rules
                .iter()
                .find(|rule| rule.kind == kind)
                .unwrap()
                .prefix
                .as_str()
        };
        assert_eq!(
            prefix(BdewIdentifierKind::MarketPartnerBdew),
            MarketPartnerIdKind::BdewElectricity.prefix()
        );
        assert_eq!(
            prefix(BdewIdentifierKind::MarketPartnerDvgw),
            MarketPartnerIdKind::DvgwGas.prefix()
        );
        assert_eq!(
            prefix(BdewIdentifierKind::NetworkArea),
            NetworkIdentifierKind::NetworkArea.prefix().to_string()
        );
        assert_eq!(prefix(BdewIdentifierKind::Package), "P9");
        assert_eq!(
            prefix(BdewIdentifierKind::ClusterResource),
            ResourceIdKind::ClusterResource.prefix().to_string()
        );
        assert_eq!(
            prefix(BdewIdentifierKind::ControlGroup),
            ResourceIdKind::ControlGroup.prefix().to_string()
        );
        assert_eq!(
            prefix(BdewIdentifierKind::ControllableResource),
            ResourceIdKind::ControllableResource.prefix().to_string()
        );
        assert_eq!(
            prefix(BdewIdentifierKind::TechnicalResource),
            ResourceIdKind::TechnicalResource.prefix().to_string()
        );
    }

    #[test]
    fn mastr_catalog_matches_runtime_prefixes_and_role_matrix() {
        let snapshot = mastr_prefixes().unwrap();
        assert_eq!(snapshot.prefixes.len(), RUNTIME_MASTR_PREFIXES.len());
        for runtime in RUNTIME_MASTR_PREFIXES {
            let record = snapshot
                .prefixes
                .iter()
                .find(|record| record.code == runtime.code())
                .unwrap();
            assert_eq!(record.label_de, runtime.label_de());
            let catalog_roles: BTreeSet<_> =
                record.role_suffixes.iter().map(String::as_str).collect();
            let runtime_roles: BTreeSet<_> = runtime
                .allowed_role_suffixes()
                .iter()
                .map(|role| role.code())
                .collect();
            assert_eq!(catalog_roles, runtime_roles, "{}", runtime.code());
            assert_eq!(MastrPrefix::from_code(&record.code), Some(*runtime));
        }
    }

    #[test]
    fn unknown_fields_unsorted_data_and_allocation_claims_are_rejected() {
        let invalid = br#"{
          "schema_version":1,
          "name":"bdew_identifiers",
          "authority":"BDEW Bundesverband der Energie- und Wasserwirtschaft e. V.",
          "version":"1.2",
          "published":"2025-02-07",
          "checked_at":"2026-08-14",
          "source_url":"https://example.invalid/source.pdf",
          "source_sha256":"0000000000000000000000000000000000000000000000000000000000000000",
          "contains_allocations":false,
          "rules":[],
          "unexpected":true
        }"#;
        assert!(parse_bdew_identifiers(invalid).is_err());

        let mut mastr = mastr_prefixes().unwrap().clone();
        mastr.prefixes.swap(0, 1);
        assert!(validate_mastr_prefixes(&mastr).is_err());

        let mut bdew = bdew_identifiers().unwrap().clone();
        bdew.contains_allocations = true;
        assert!(validate_bdew_identifiers(&bdew).is_err());
    }
}
