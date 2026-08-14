//! BDEW resource IDs used for Redispatch 2.0.
//!
//! Resource IDs are centrally allocated, uppercase alphanumeric 11-character
//! identifiers. Their first character selects the type: `A` (CR-ID), `B`
//! (SG-ID), `C` (SR-ID), or `D` (TR-ID). Positions 2 through 10 are uppercase
//! ASCII alphanumeric characters and position 11 is a decimal check digit
//! calculated with the BDEW ASCII procedure.

use std::error::Error;
use std::fmt;

use super::checksum::{self, ChecksumInputError};
use super::{GeneratedEnergyIdentifier, ValidatedEnergyIdentifier};
use crate::fixture::DeterministicRng;

const BASE_LENGTH: usize = 10;
const FULL_LENGTH: usize = 11;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceIdKind {
    ClusterResource,
    ControlGroup,
    ControllableResource,
    TechnicalResource,
}

impl ResourceIdKind {
    pub const fn prefix(self) -> char {
        match self {
            Self::ClusterResource => 'A',
            Self::ControlGroup => 'B',
            Self::ControllableResource => 'C',
            Self::TechnicalResource => 'D',
        }
    }

    pub const fn slug(self) -> &'static str {
        match self {
            Self::ClusterResource => "cr-id",
            Self::ControlGroup => "sg-id",
            Self::ControllableResource => "sr-id",
            Self::TechnicalResource => "tr-id",
        }
    }

    const fn fixture_namespace(self) -> &'static str {
        match self {
            Self::ClusterResource => "energy.resource.cr",
            Self::ControlGroup => "energy.resource.sg",
            Self::ControllableResource => "energy.resource.sr",
            Self::TechnicalResource => "energy.resource.tr",
        }
    }

    fn from_prefix(prefix: u8) -> Option<Self> {
        match prefix {
            b'A' => Some(Self::ClusterResource),
            b'B' => Some(Self::ControlGroup),
            b'C' => Some(Self::ControllableResource),
            b'D' => Some(Self::TechnicalResource),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceIdError {
    NonAscii,
    InvalidLength {
        expected: usize,
        actual: usize,
    },
    InvalidPrefix {
        found: char,
    },
    InvalidCharacter {
        position: usize,
        found: char,
    },
    NonNumericCheckDigit {
        found: char,
    },
    ChecksumInput(ChecksumInputError),
    ChecksumMismatch {
        expected: u8,
        actual: u8,
    },
    KindMismatch {
        expected: ResourceIdKind,
        actual: ResourceIdKind,
    },
}

impl fmt::Display for ResourceIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonAscii => formatter.write_str("resource ID must contain only ASCII characters"),
            Self::InvalidLength { expected, actual } => {
                write!(
                    formatter,
                    "resource ID must be {expected} characters, got {actual}"
                )
            }
            Self::InvalidPrefix { found } => write!(
                formatter,
                "resource ID prefix must be A (CR), B (SG), C (SR), or D (TR), got {found}"
            ),
            Self::InvalidCharacter { position, found } => write!(
                formatter,
                "resource ID contains invalid character '{found}' at 1-based position {position}"
            ),
            Self::NonNumericCheckDigit { found } => write!(
                formatter,
                "resource ID check digit must be numeric, got {found}"
            ),
            Self::ChecksumInput(error) => write!(formatter, "invalid checksum input: {error}"),
            Self::ChecksumMismatch { expected, actual } => write!(
                formatter,
                "invalid resource ID checksum: expected {expected}, got {actual}"
            ),
            Self::KindMismatch { expected, actual } => write!(
                formatter,
                "resource ID kind mismatch: expected {expected:?}, got {actual:?}"
            ),
        }
    }
}

impl Error for ResourceIdError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ChecksumInput(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ChecksumInputError> for ResourceIdError {
    fn from(value: ChecksumInputError) -> Self {
        Self::ChecksumInput(value)
    }
}

/// Calculates the check digit for a complete 10-character resource-ID base.
///
/// This validates both the A/B/C/D resource prefix and all following uppercase
/// ASCII alphanumeric characters before applying the BDEW ASCII procedure.
pub fn calculate_resource_check_digit(base: &str) -> Result<u8, ResourceIdError> {
    validate_base(base)?;
    checksum::calculate_bdew_ascii_checksum(base).map_err(Into::into)
}

/// Validates syntax and checksum and infers CR-ID, SG-ID, SR-ID, or TR-ID.
///
/// No BDEW directory is queried, so a successful result retains
/// `allocation_status = Unknown`.
pub fn validate_resource_id(
    input: &str,
) -> Result<ValidatedEnergyIdentifier<ResourceIdKind>, ResourceIdError> {
    if !input.is_ascii() {
        return Err(ResourceIdError::NonAscii);
    }
    if input.len() != FULL_LENGTH {
        return Err(ResourceIdError::InvalidLength {
            expected: FULL_LENGTH,
            actual: input.len(),
        });
    }

    let kind = validate_base(&input[..BASE_LENGTH])?;
    let check_byte = input.as_bytes()[BASE_LENGTH];
    if !check_byte.is_ascii_digit() {
        return Err(ResourceIdError::NonNumericCheckDigit {
            found: char::from(check_byte),
        });
    }

    let expected =
        checksum::bdew_ascii_from_valid_upper_alphanumeric(&input.as_bytes()[..BASE_LENGTH]);
    let actual = check_byte - b'0';
    if actual != expected {
        return Err(ResourceIdError::ChecksumMismatch { expected, actual });
    }

    Ok(ValidatedEnergyIdentifier::new(
        input.to_owned(),
        kind,
        actual,
    ))
}

pub fn validate_resource_id_for_kind(
    input: &str,
    expected_kind: ResourceIdKind,
) -> Result<ValidatedEnergyIdentifier<ResourceIdKind>, ResourceIdError> {
    let validated = validate_resource_id(input)?;
    if validated.kind != expected_kind {
        return Err(ResourceIdError::KindMismatch {
            expected: expected_kind,
            actual: validated.kind,
        });
    }
    Ok(validated)
}

/// Creates a reproducible, format- and checksum-valid resource-ID fixture.
///
/// The generated value is not reserved with BDEW and may collide with a
/// centrally allocated value. Its allocation status therefore remains unknown.
pub fn generate_resource_id(
    kind: ResourceIdKind,
    fixture_seed: &str,
    index: u32,
) -> GeneratedEnergyIdentifier<ResourceIdKind> {
    let mut stream = DeterministicRng::new(fixture_seed, kind.fixture_namespace(), index);
    let mut base = String::with_capacity(BASE_LENGTH);
    base.push(kind.prefix());
    for _ in 0..9 {
        base.push(stream.uppercase_alphanumeric());
    }

    let check_digit = checksum::bdew_ascii_from_valid_upper_alphanumeric(base.as_bytes());
    let mut value = base;
    value.push(char::from(b'0' + check_digit));
    GeneratedEnergyIdentifier::new(value, kind, check_digit)
}

pub fn generate_cr_id(fixture_seed: &str, index: u32) -> GeneratedEnergyIdentifier<ResourceIdKind> {
    generate_resource_id(ResourceIdKind::ClusterResource, fixture_seed, index)
}

pub fn generate_sg_id(fixture_seed: &str, index: u32) -> GeneratedEnergyIdentifier<ResourceIdKind> {
    generate_resource_id(ResourceIdKind::ControlGroup, fixture_seed, index)
}

pub fn generate_sr_id(fixture_seed: &str, index: u32) -> GeneratedEnergyIdentifier<ResourceIdKind> {
    generate_resource_id(ResourceIdKind::ControllableResource, fixture_seed, index)
}

pub fn generate_tr_id(fixture_seed: &str, index: u32) -> GeneratedEnergyIdentifier<ResourceIdKind> {
    generate_resource_id(ResourceIdKind::TechnicalResource, fixture_seed, index)
}

pub fn validate_cr_id(
    input: &str,
) -> Result<ValidatedEnergyIdentifier<ResourceIdKind>, ResourceIdError> {
    validate_resource_id_for_kind(input, ResourceIdKind::ClusterResource)
}

pub fn validate_sg_id(
    input: &str,
) -> Result<ValidatedEnergyIdentifier<ResourceIdKind>, ResourceIdError> {
    validate_resource_id_for_kind(input, ResourceIdKind::ControlGroup)
}

pub fn validate_sr_id(
    input: &str,
) -> Result<ValidatedEnergyIdentifier<ResourceIdKind>, ResourceIdError> {
    validate_resource_id_for_kind(input, ResourceIdKind::ControllableResource)
}

pub fn validate_tr_id(
    input: &str,
) -> Result<ValidatedEnergyIdentifier<ResourceIdKind>, ResourceIdError> {
    validate_resource_id_for_kind(input, ResourceIdKind::TechnicalResource)
}

fn validate_base(base: &str) -> Result<ResourceIdKind, ResourceIdError> {
    if !base.is_ascii() {
        return Err(ResourceIdError::NonAscii);
    }
    if base.len() != BASE_LENGTH {
        return Err(ResourceIdError::InvalidLength {
            expected: BASE_LENGTH,
            actual: base.len(),
        });
    }

    let bytes = base.as_bytes();
    let kind = ResourceIdKind::from_prefix(bytes[0]).ok_or(ResourceIdError::InvalidPrefix {
        found: char::from(bytes[0]),
    })?;
    for (index, byte) in bytes.iter().enumerate().skip(1) {
        if !(byte.is_ascii_digit() || byte.is_ascii_uppercase()) {
            return Err(ResourceIdError::InvalidCharacter {
                position: index + 1,
                found: char::from(*byte),
            });
        }
    }
    Ok(kind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identifiers::energy::CentralAllocationStatus;

    fn mutate_check_digit(value: &str) -> String {
        let mut bytes = value.as_bytes().to_vec();
        let last = bytes.len() - 1;
        bytes[last] = b'0' + ((bytes[last] - b'0' + 1) % 10);
        String::from_utf8(bytes).unwrap()
    }

    #[test]
    fn accepts_official_bdew_ascii_example_as_cluster_resource() {
        // BDEW v1.2, section 8.2 publishes A1137355925 as its worked example.
        let result = validate_cr_id("A1137355925").unwrap();
        assert_eq!(result.kind, ResourceIdKind::ClusterResource);
        assert_eq!(result.check_digit, 5);
        assert_eq!(result.allocation_status, CentralAllocationStatus::Unknown);
    }

    #[test]
    fn each_normative_prefix_maps_to_exactly_one_kind() {
        for (kind, base) in [
            (ResourceIdKind::ClusterResource, "A123456789"),
            (ResourceIdKind::ControlGroup, "B123456789"),
            (ResourceIdKind::ControllableResource, "C123456789"),
            (ResourceIdKind::TechnicalResource, "D123456789"),
        ] {
            let check = calculate_resource_check_digit(base).unwrap();
            let value = format!("{base}{check}");
            assert_eq!(validate_resource_id(&value).unwrap().kind, kind);
            assert!(validate_resource_id_for_kind(&value, kind).is_ok());
        }
    }

    #[test]
    fn prefix_kind_mismatch_is_rejected() {
        assert!(matches!(
            validate_tr_id("A1137355925"),
            Err(ResourceIdError::KindMismatch {
                expected: ResourceIdKind::TechnicalResource,
                actual: ResourceIdKind::ClusterResource,
            })
        ));
    }

    #[test]
    fn checksum_mutation_is_rejected() {
        let mutated = mutate_check_digit("A1137355925");
        assert!(matches!(
            validate_resource_id(&mutated),
            Err(ResourceIdError::ChecksumMismatch { .. })
        ));
    }

    #[test]
    fn validators_are_strict_and_unicode_safe() {
        assert_eq!(
            validate_resource_id("A12345678ß5"),
            Err(ResourceIdError::NonAscii)
        );
        assert!(matches!(
            validate_resource_id("a1234567890"),
            Err(ResourceIdError::InvalidPrefix { found: 'a' })
        ));
        assert!(matches!(
            validate_resource_id("A12345-7890"),
            Err(ResourceIdError::InvalidCharacter { position: 7, .. })
        ));
        assert!(matches!(
            validate_resource_id("E1234567890"),
            Err(ResourceIdError::InvalidPrefix { found: 'E' })
        ));
        assert!(matches!(
            validate_resource_id("A123456789X"),
            Err(ResourceIdError::NonNumericCheckDigit { found: 'X' })
        ));
    }

    #[test]
    fn deterministic_generators_are_reproducible_and_self_validating() {
        let kinds = [
            ResourceIdKind::ClusterResource,
            ResourceIdKind::ControlGroup,
            ResourceIdKind::ControllableResource,
            ResourceIdKind::TechnicalResource,
        ];

        for index in 0..100 {
            for kind in kinds {
                let first = generate_resource_id(kind, "nrg-demo-1", index);
                let second = generate_resource_id(kind, "nrg-demo-1", index);
                assert_eq!(first, second);
                assert_eq!(first.allocation_status, CentralAllocationStatus::Unknown);
                assert_eq!(
                    validate_resource_id_for_kind(&first.value, kind)
                        .unwrap()
                        .check_digit,
                    first.check_digit
                );
            }
        }

        assert_ne!(
            generate_cr_id("nrg-demo-1", 0).value,
            generate_cr_id("nrg-demo-1", 1).value
        );
        assert_ne!(
            generate_cr_id("nrg-demo-1", 0).value,
            generate_cr_id("other", 0).value
        );
    }

    #[test]
    fn generator_version_one_snapshots_are_stable() {
        assert_eq!(crate::GENERATOR_VERSION, "1");
        assert_eq!(generate_cr_id("nrg-demo-1", 0).value, "AZOU04A7H65");
        assert_eq!(generate_sg_id("nrg-demo-1", 0).value, "B4Y57JUBBZ9");
        assert_eq!(generate_sr_id("nrg-demo-1", 0).value, "CR14JIJ7D62");
        assert_eq!(generate_tr_id("nrg-demo-1", 0).value, "DENLEZF1WM2");
    }
}
