//! Marktstammdatenregister (MaStR) identifiers.
//!
//! The Bundesnetzagentur number concept defines a three-letter object or
//! market-function prefix, version digit `9`, ten random digits, an EAN-style
//! check digit, and an optional two-letter role suffix for selected market
//! participants. This module validates those published rules only. It performs
//! no MaStR lookup and therefore cannot establish allocation.

use std::{error::Error, fmt};

use crate::fixture::DeterministicRng;

pub const MASTR_NUMBER_CONCEPT_VERSION: &str = crate::reference_data::MASTR_PREFIXES_VERSION;
pub const MASTR_WEB_SERVICE_VERSION_CHECKED: &str = "26.1.177 (2026-08-14)";

const PREFIX_LENGTH: usize = 3;
const NUMERIC_BASE_LENGTH: usize = 11;
const NUMERIC_FULL_LENGTH: usize = 12;
const WITHOUT_SUFFIX_LENGTH: usize = PREFIX_LENGTH + NUMERIC_FULL_LENGTH;
const WITH_SUFFIX_LENGTH: usize = WITHOUT_SUFFIX_LENGTH + 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MastrSector {
    Electricity,
    Gas,
    CrossSector,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MastrObjectGroup {
    UnitGroupingOrApproval,
    MarketParticipant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MastrLifecycle {
    Current,
    LegacyMigratedUnit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MastrPrefix {
    TechnicalElectricityGenerationLocation,
    TechnicalElectricityConsumptionLocation,
    ElectricityGenerationUnit,
    MigratedElectricityUnit,
    ElectricityConsumptionUnit,
    ElectricityStorageUnit,
    EegInstallation,
    KwkInstallation,
    ElectricityApproval,
    ElectricityNetwork,
    ElectricityNetworkConnectionPoint,
    TechnicalGasGenerationLocation,
    TechnicalGasConsumptionLocation,
    GasGenerationUnit,
    MigratedGasUnit,
    GasConsumptionUnit,
    GasStorageUnit,
    GasNetwork,
    GasNetworkConnectionPoint,
    ElectricityNetworkOperator,
    GasNetworkOperator,
    InstallationOperator,
    ElectricityMarketActor,
    GasMarketActor,
    OrganisedMarketplace,
    AuthorityAssociationInstitution,
    OtherMarketActor,
}

pub const MASTR_PREFIXES: &[MastrPrefix] = &[
    MastrPrefix::TechnicalElectricityGenerationLocation,
    MastrPrefix::TechnicalElectricityConsumptionLocation,
    MastrPrefix::ElectricityGenerationUnit,
    MastrPrefix::MigratedElectricityUnit,
    MastrPrefix::ElectricityConsumptionUnit,
    MastrPrefix::ElectricityStorageUnit,
    MastrPrefix::EegInstallation,
    MastrPrefix::KwkInstallation,
    MastrPrefix::ElectricityApproval,
    MastrPrefix::ElectricityNetwork,
    MastrPrefix::ElectricityNetworkConnectionPoint,
    MastrPrefix::TechnicalGasGenerationLocation,
    MastrPrefix::TechnicalGasConsumptionLocation,
    MastrPrefix::GasGenerationUnit,
    MastrPrefix::MigratedGasUnit,
    MastrPrefix::GasConsumptionUnit,
    MastrPrefix::GasStorageUnit,
    MastrPrefix::GasNetwork,
    MastrPrefix::GasNetworkConnectionPoint,
    MastrPrefix::ElectricityNetworkOperator,
    MastrPrefix::GasNetworkOperator,
    MastrPrefix::InstallationOperator,
    MastrPrefix::ElectricityMarketActor,
    MastrPrefix::GasMarketActor,
    MastrPrefix::OrganisedMarketplace,
    MastrPrefix::AuthorityAssociationInstitution,
    MastrPrefix::OtherMarketActor,
];

impl MastrPrefix {
    pub const fn code(self) -> &'static str {
        match self {
            Self::TechnicalElectricityGenerationLocation => "SEL",
            Self::TechnicalElectricityConsumptionLocation => "SVL",
            Self::ElectricityGenerationUnit => "SEE",
            Self::MigratedElectricityUnit => "SME",
            Self::ElectricityConsumptionUnit => "SVE",
            Self::ElectricityStorageUnit => "SSE",
            Self::EegInstallation => "EEG",
            Self::KwkInstallation => "KWK",
            Self::ElectricityApproval => "SGE",
            Self::ElectricityNetwork => "SNE",
            Self::ElectricityNetworkConnectionPoint => "SAN",
            Self::TechnicalGasGenerationLocation => "GEL",
            Self::TechnicalGasConsumptionLocation => "GVL",
            Self::GasGenerationUnit => "GEE",
            Self::MigratedGasUnit => "GME",
            Self::GasConsumptionUnit => "GVE",
            Self::GasStorageUnit => "GSE",
            Self::GasNetwork => "GNE",
            Self::GasNetworkConnectionPoint => "GAN",
            Self::ElectricityNetworkOperator => "SNB",
            Self::GasNetworkOperator => "GNB",
            Self::InstallationOperator => "ABR",
            Self::ElectricityMarketActor => "SEM",
            Self::GasMarketActor => "GEM",
            Self::OrganisedMarketplace => "OMP",
            Self::AuthorityAssociationInstitution => "BVI",
            Self::OtherMarketActor => "SOM",
        }
    }

    pub const fn label_de(self) -> &'static str {
        match self {
            Self::TechnicalElectricityGenerationLocation => "Technische Stromerzeugungslokation",
            Self::TechnicalElectricityConsumptionLocation => "Technische Stromverbrauchslokation",
            Self::ElectricityGenerationUnit => "Stromerzeugungseinheit",
            Self::MigratedElectricityUnit => "Migrierte Stromeinheit",
            Self::ElectricityConsumptionUnit => "Stromverbrauchseinheit",
            Self::ElectricityStorageUnit => "Stromspeicher",
            Self::EegInstallation => "EEG-Anlage",
            Self::KwkInstallation => "KWK-Anlage",
            Self::ElectricityApproval => "Genehmigung Strom",
            Self::ElectricityNetwork => "Stromnetz",
            Self::ElectricityNetworkConnectionPoint => "Strom-Netzanschlusspunkt",
            Self::TechnicalGasGenerationLocation => "Technische Gaserzeugungslokation",
            Self::TechnicalGasConsumptionLocation => "Technische Gasverbrauchslokation",
            Self::GasGenerationUnit => "Gaserzeugungseinheit",
            Self::MigratedGasUnit => "Migrierte Gaseinheit",
            Self::GasConsumptionUnit => "Gasverbrauchseinheit",
            Self::GasStorageUnit => "Gasspeicher",
            Self::GasNetwork => "Gasnetz",
            Self::GasNetworkConnectionPoint => "Gas-Netzanschlusspunkt",
            Self::ElectricityNetworkOperator => "Stromnetzbetreiber",
            Self::GasNetworkOperator => "Gasnetzbetreiber",
            Self::InstallationOperator => "Anlagenbetreiber",
            Self::ElectricityMarketActor => "Akteur im Strommarkt",
            Self::GasMarketActor => "Akteur im Gasmarkt",
            Self::OrganisedMarketplace => "Organisierter Marktplatz",
            Self::AuthorityAssociationInstitution => "Behörde, Verband oder Institution",
            Self::OtherMarketActor => "Sonstiger Marktakteur",
        }
    }

    pub const fn sector(self) -> MastrSector {
        match self {
            Self::TechnicalElectricityGenerationLocation
            | Self::TechnicalElectricityConsumptionLocation
            | Self::ElectricityGenerationUnit
            | Self::MigratedElectricityUnit
            | Self::ElectricityConsumptionUnit
            | Self::ElectricityStorageUnit
            | Self::EegInstallation
            | Self::KwkInstallation
            | Self::ElectricityApproval
            | Self::ElectricityNetwork
            | Self::ElectricityNetworkConnectionPoint
            | Self::ElectricityNetworkOperator
            | Self::ElectricityMarketActor => MastrSector::Electricity,
            Self::TechnicalGasGenerationLocation
            | Self::TechnicalGasConsumptionLocation
            | Self::GasGenerationUnit
            | Self::MigratedGasUnit
            | Self::GasConsumptionUnit
            | Self::GasStorageUnit
            | Self::GasNetwork
            | Self::GasNetworkConnectionPoint
            | Self::GasNetworkOperator
            | Self::GasMarketActor => MastrSector::Gas,
            Self::InstallationOperator
            | Self::OrganisedMarketplace
            | Self::AuthorityAssociationInstitution
            | Self::OtherMarketActor => MastrSector::CrossSector,
        }
    }

    pub const fn object_group(self) -> MastrObjectGroup {
        match self {
            Self::ElectricityNetworkOperator
            | Self::GasNetworkOperator
            | Self::InstallationOperator
            | Self::ElectricityMarketActor
            | Self::GasMarketActor
            | Self::OrganisedMarketplace
            | Self::AuthorityAssociationInstitution
            | Self::OtherMarketActor => MastrObjectGroup::MarketParticipant,
            _ => MastrObjectGroup::UnitGroupingOrApproval,
        }
    }

    pub const fn lifecycle(self) -> MastrLifecycle {
        match self {
            Self::MigratedElectricityUnit | Self::MigratedGasUnit => {
                MastrLifecycle::LegacyMigratedUnit
            }
            _ => MastrLifecycle::Current,
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        MASTR_PREFIXES
            .iter()
            .copied()
            .find(|prefix| prefix.code() == code)
    }

    pub const fn allowed_role_suffixes(self) -> &'static [MastrRoleSuffix] {
        use MastrRoleSuffix as Role;
        match self {
            Self::ElectricityNetworkOperator => &[
                Role::TransmissionSystemOperator,
                Role::ConnectionNetworkOperator,
                Role::BalanceResponsibleParty,
                Role::BalanceCoordinator,
                Role::MeteringPointOperator,
            ],
            Self::GasNetworkOperator => &[
                Role::TransmissionNetworkOperator,
                Role::MarketAreaManager,
                Role::ConnectionNetworkOperator,
                Role::MeteringPointOperator,
            ],
            Self::ElectricityMarketActor => &[
                Role::BalanceResponsibleParty,
                Role::MeteringPointOperator,
                Role::SupplierDirectMarketerWholesaler,
            ],
            Self::GasMarketActor => &[
                Role::BalanceResponsibleParty,
                Role::MeteringPointOperator,
                Role::Shipper,
            ],
            Self::OrganisedMarketplace => &[
                Role::CrossBorderCapacityBookingPlatform,
                Role::Exchange,
                Role::OtcPlatform,
                Role::GasCapacityBookingPlatform,
                Role::GasStorageBookingPlatform,
            ],
            Self::AuthorityAssociationInstitution => &[
                Role::Authority,
                Role::EnergyIndustryAssociation,
                Role::EnergyIndustryInstitution,
            ],
            Self::OtherMarketActor => &[Role::ServiceProvider, Role::OtherMarketRole],
            _ => &[],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MastrRoleSuffix {
    TransmissionSystemOperator,
    ConnectionNetworkOperator,
    BalanceResponsibleParty,
    BalanceCoordinator,
    MeteringPointOperator,
    TransmissionNetworkOperator,
    MarketAreaManager,
    SupplierDirectMarketerWholesaler,
    Shipper,
    CrossBorderCapacityBookingPlatform,
    Exchange,
    OtcPlatform,
    GasCapacityBookingPlatform,
    GasStorageBookingPlatform,
    Authority,
    EnergyIndustryAssociation,
    EnergyIndustryInstitution,
    ServiceProvider,
    OtherMarketRole,
}

impl MastrRoleSuffix {
    pub const fn code(self) -> &'static str {
        match self {
            Self::TransmissionSystemOperator => "UN",
            Self::ConnectionNetworkOperator => "AN",
            Self::BalanceResponsibleParty => "BV",
            Self::BalanceCoordinator => "BK",
            Self::MeteringPointOperator => "MB",
            Self::TransmissionNetworkOperator => "FN",
            Self::MarketAreaManager => "MV",
            Self::SupplierDirectMarketerWholesaler => "LT",
            Self::Shipper => "TK",
            Self::CrossBorderCapacityBookingPlatform => "SK",
            Self::Exchange => "BO",
            Self::OtcPlatform => "OP",
            Self::GasCapacityBookingPlatform => "GK",
            Self::GasStorageBookingPlatform => "GS",
            Self::Authority => "BE",
            Self::EnergyIndustryAssociation => "EV",
            Self::EnergyIndustryInstitution => "EI",
            Self::ServiceProvider => "DL",
            Self::OtherMarketRole => "SR",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        const ALL: &[MastrRoleSuffix] = &[
            MastrRoleSuffix::TransmissionSystemOperator,
            MastrRoleSuffix::ConnectionNetworkOperator,
            MastrRoleSuffix::BalanceResponsibleParty,
            MastrRoleSuffix::BalanceCoordinator,
            MastrRoleSuffix::MeteringPointOperator,
            MastrRoleSuffix::TransmissionNetworkOperator,
            MastrRoleSuffix::MarketAreaManager,
            MastrRoleSuffix::SupplierDirectMarketerWholesaler,
            MastrRoleSuffix::Shipper,
            MastrRoleSuffix::CrossBorderCapacityBookingPlatform,
            MastrRoleSuffix::Exchange,
            MastrRoleSuffix::OtcPlatform,
            MastrRoleSuffix::GasCapacityBookingPlatform,
            MastrRoleSuffix::GasStorageBookingPlatform,
            MastrRoleSuffix::Authority,
            MastrRoleSuffix::EnergyIndustryAssociation,
            MastrRoleSuffix::EnergyIndustryInstitution,
            MastrRoleSuffix::ServiceProvider,
            MastrRoleSuffix::OtherMarketRole,
        ];
        ALL.iter().copied().find(|role| role.code() == code)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MastrAllocationStatus {
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MastrCollisionGuarantee {
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MastrParts {
    pub value: String,
    pub prefix: MastrPrefix,
    pub version: u8,
    pub random_body: String,
    pub check_digit: u8,
    pub role_suffix: Option<MastrRoleSuffix>,
    pub allocation_status: MastrAllocationStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntheticMastrFixture {
    pub identifier: MastrParts,
    pub synthetic: bool,
    pub production_usable: bool,
    pub collision_guarantee: MastrCollisionGuarantee,
    pub generator_version: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MastrError {
    NonAscii,
    InvalidLength {
        actual: usize,
    },
    UnknownPrefix {
        prefix: String,
    },
    InvalidVersion {
        found: char,
    },
    NonDigit {
        position: usize,
        found: char,
    },
    UnknownRoleSuffix {
        suffix: String,
    },
    RoleSuffixNotAllowed {
        prefix: MastrPrefix,
        suffix: MastrRoleSuffix,
    },
    ChecksumMismatch {
        expected: u8,
        actual: u8,
    },
    InvalidChecksumBaseLength {
        actual: usize,
    },
}

impl fmt::Display for MastrError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonAscii => formatter.write_str("MaStR number must contain only ASCII"),
            Self::InvalidLength { actual } => write!(
                formatter,
                "MaStR number must be {WITHOUT_SUFFIX_LENGTH} characters without a role suffix or {WITH_SUFFIX_LENGTH} with one, got {actual}"
            ),
            Self::UnknownPrefix { prefix } => {
                write!(formatter, "unknown MaStR prefix {prefix:?}")
            }
            Self::InvalidVersion { found } => {
                write!(formatter, "MaStR version digit must be 9, got {found}")
            }
            Self::NonDigit { position, found } => write!(
                formatter,
                "MaStR numeric part contains non-digit {found:?} at position {position}"
            ),
            Self::UnknownRoleSuffix { suffix } => {
                write!(formatter, "unknown MaStR role suffix {suffix:?}")
            }
            Self::RoleSuffixNotAllowed { prefix, suffix } => write!(
                formatter,
                "MaStR role suffix {} is not allowed for prefix {}",
                suffix.code(),
                prefix.code()
            ),
            Self::ChecksumMismatch { expected, actual } => write!(
                formatter,
                "invalid MaStR checksum: expected {expected}, got {actual}"
            ),
            Self::InvalidChecksumBaseLength { actual } => write!(
                formatter,
                "MaStR checksum base must be {NUMERIC_BASE_LENGTH} digits, got {actual}"
            ),
        }
    }
}

impl Error for MastrError {}

pub fn calculate_mastr_check_digit(numeric_base: &str) -> Result<u8, MastrError> {
    if !numeric_base.is_ascii() {
        return Err(MastrError::NonAscii);
    }
    if numeric_base.len() != NUMERIC_BASE_LENGTH {
        return Err(MastrError::InvalidChecksumBaseLength {
            actual: numeric_base.len(),
        });
    }
    let mut sum = 0_u32;
    for (index, byte) in numeric_base.bytes().enumerate() {
        if !byte.is_ascii_digit() {
            return Err(MastrError::NonDigit {
                position: PREFIX_LENGTH + index + 1,
                found: char::from(byte),
            });
        }
        let weight = if index % 2 == 0 { 3 } else { 1 };
        sum += u32::from(byte - b'0') * weight;
    }
    Ok(((10 - (sum % 10)) % 10) as u8)
}

pub fn parse_mastr(input: &str) -> Result<MastrParts, MastrError> {
    if !input.is_ascii() {
        return Err(MastrError::NonAscii);
    }
    if !matches!(input.len(), WITHOUT_SUFFIX_LENGTH | WITH_SUFFIX_LENGTH) {
        return Err(MastrError::InvalidLength {
            actual: input.len(),
        });
    }

    let prefix_code = &input[..PREFIX_LENGTH];
    let prefix = MastrPrefix::from_code(prefix_code).ok_or_else(|| MastrError::UnknownPrefix {
        prefix: prefix_code.to_string(),
    })?;

    let numeric = &input[PREFIX_LENGTH..WITHOUT_SUFFIX_LENGTH];
    for (index, byte) in numeric.bytes().enumerate() {
        if !byte.is_ascii_digit() {
            return Err(MastrError::NonDigit {
                position: PREFIX_LENGTH + index + 1,
                found: char::from(byte),
            });
        }
    }
    if numeric.as_bytes()[0] != b'9' {
        return Err(MastrError::InvalidVersion {
            found: char::from(numeric.as_bytes()[0]),
        });
    }

    let expected = calculate_mastr_check_digit(&numeric[..NUMERIC_BASE_LENGTH])?;
    let actual = numeric.as_bytes()[NUMERIC_BASE_LENGTH] - b'0';
    if actual != expected {
        return Err(MastrError::ChecksumMismatch { expected, actual });
    }

    let role_suffix = if input.len() == WITH_SUFFIX_LENGTH {
        let suffix_code = &input[WITHOUT_SUFFIX_LENGTH..];
        let suffix = MastrRoleSuffix::from_code(suffix_code).ok_or_else(|| {
            MastrError::UnknownRoleSuffix {
                suffix: suffix_code.to_string(),
            }
        })?;
        if !prefix.allowed_role_suffixes().contains(&suffix) {
            return Err(MastrError::RoleSuffixNotAllowed { prefix, suffix });
        }
        Some(suffix)
    } else {
        None
    };

    Ok(MastrParts {
        value: input.to_string(),
        prefix,
        version: 9,
        random_body: numeric[1..NUMERIC_BASE_LENGTH].to_string(),
        check_digit: actual,
        role_suffix,
        allocation_status: MastrAllocationStatus::Unknown,
    })
}

pub fn validate_mastr(input: &str) -> Result<MastrParts, MastrError> {
    parse_mastr(input)
}

/// Creates a reproducible syntax- and checksum-valid fixture.
///
/// MaStR allocates these identifiers centrally. Consequently the fixture has
/// no collision guarantee and is never marked production-usable, even though
/// its checksum is valid.
pub fn generate_synthetic_mastr(
    prefix: MastrPrefix,
    role_suffix: Option<MastrRoleSuffix>,
    fixture_seed: &str,
    index: u32,
) -> Result<SyntheticMastrFixture, MastrError> {
    if let Some(suffix) = role_suffix {
        if !prefix.allowed_role_suffixes().contains(&suffix) {
            return Err(MastrError::RoleSuffixNotAllowed { prefix, suffix });
        }
    }

    let namespace = format!("registers.mastr.{}", prefix.code());
    let mut rng = DeterministicRng::new(fixture_seed, &namespace, index);
    let mut numeric_base = String::with_capacity(NUMERIC_BASE_LENGTH);
    numeric_base.push('9');
    for _ in 0..10 {
        numeric_base.push(char::from(b'0' + rng.digit()));
    }
    let check_digit = calculate_mastr_check_digit(&numeric_base)?;
    let mut value = String::with_capacity(if role_suffix.is_some() {
        WITH_SUFFIX_LENGTH
    } else {
        WITHOUT_SUFFIX_LENGTH
    });
    value.push_str(prefix.code());
    value.push_str(&numeric_base);
    value.push(char::from(b'0' + check_digit));
    if let Some(suffix) = role_suffix {
        value.push_str(suffix.code());
    }

    Ok(SyntheticMastrFixture {
        identifier: parse_mastr(&value)?,
        synthetic: true,
        production_usable: false,
        collision_guarantee: MastrCollisionGuarantee::None,
        generator_version: crate::GENERATOR_VERSION,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn official_number_concept_examples_validate() {
        let network_operator = validate_mastr("SNB901234567898AN").unwrap();
        assert_eq!(
            network_operator.prefix,
            MastrPrefix::ElectricityNetworkOperator
        );
        assert_eq!(
            network_operator.role_suffix,
            Some(MastrRoleSuffix::ConnectionNetworkOperator)
        );

        let installation_operator = validate_mastr("ABR919283764526").unwrap();
        assert_eq!(
            installation_operator.prefix,
            MastrPrefix::InstallationOperator
        );
        assert_eq!(installation_operator.role_suffix, None);
    }

    #[test]
    fn prefix_and_role_compatibility_is_strict() {
        assert!(matches!(
            validate_mastr("SEE901234567898AN"),
            Err(MastrError::RoleSuffixNotAllowed { .. })
        ));
        assert!(matches!(
            validate_mastr("GNB901234567898UN"),
            Err(MastrError::RoleSuffixNotAllowed { .. })
        ));
        assert!(matches!(
            validate_mastr("XYZ901234567898"),
            Err(MastrError::UnknownPrefix { .. })
        ));
    }

    #[test]
    fn checksum_mutation_and_unicode_are_rejected() {
        let mut value = b"ABR919283764526".to_vec();
        value[14] = b'7';
        assert!(matches!(
            validate_mastr(&String::from_utf8(value).unwrap()),
            Err(MastrError::ChecksumMismatch { .. })
        ));
        assert_eq!(
            validate_mastr("ABR91928376452６"),
            Err(MastrError::NonAscii)
        );
    }

    #[test]
    fn every_generated_fixture_roundtrips_and_is_explicitly_unassigned() {
        for (prefix_index, prefix) in MASTR_PREFIXES.iter().copied().enumerate() {
            let suffix = prefix.allowed_role_suffixes().first().copied();
            for index in 0..32 {
                let fixture = generate_synthetic_mastr(
                    prefix,
                    suffix,
                    "mastr-property",
                    index + prefix_index as u32,
                )
                .unwrap();
                assert_eq!(
                    validate_mastr(&fixture.identifier.value).unwrap(),
                    fixture.identifier
                );
                assert!(fixture.synthetic);
                assert!(!fixture.production_usable);
                assert_eq!(fixture.collision_guarantee, MastrCollisionGuarantee::None);
                assert_eq!(
                    fixture.identifier.allocation_status,
                    MastrAllocationStatus::Unknown
                );
                assert_eq!(
                    fixture,
                    generate_synthetic_mastr(
                        prefix,
                        suffix,
                        "mastr-property",
                        index + prefix_index as u32
                    )
                    .unwrap()
                );
            }
        }
    }

    #[test]
    fn prefix_catalog_is_unique_and_roundtrips() {
        for (index, prefix) in MASTR_PREFIXES.iter().enumerate() {
            assert_eq!(MastrPrefix::from_code(prefix.code()), Some(*prefix));
            assert!(MASTR_PREFIXES[index + 1..]
                .iter()
                .all(|other| other.code() != prefix.code()));
        }
    }
}
