//! BDEW network-area (NeBe) and network-change package identifiers.

use std::{error::Error, fmt};

use crate::{
    checksum::{bdew_ascii, ChecksumInputError},
    fixture::DeterministicRng,
};

use super::{GeneratedEnergyIdentifier, ValidatedEnergyIdentifier};

const BASE_LENGTH: usize = 10;
const FULL_LENGTH: usize = 11;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NetworkIdentifierKind {
    NetworkArea,
    Package,
}

impl NetworkIdentifierKind {
    pub const fn prefix(self) -> char {
        match self {
            Self::NetworkArea => 'F',
            Self::Package => 'P',
        }
    }

    pub const fn slug(self) -> &'static str {
        match self {
            Self::NetworkArea => "nebe",
            Self::Package => "package-id",
        }
    }

    const fn fixture_namespace(self) -> &'static str {
        match self {
            Self::NetworkArea => "energy.network-area.nebe",
            Self::Package => "energy.network-change.package",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkIdentifierError {
    NonAscii,
    InvalidLength {
        expected: usize,
        actual: usize,
    },
    InvalidPrefix {
        found: char,
    },
    InvalidPackageIssuer {
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
        expected: NetworkIdentifierKind,
        actual: NetworkIdentifierKind,
    },
}

impl fmt::Display for NetworkIdentifierError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonAscii => formatter.write_str("network identifier must contain only ASCII"),
            Self::InvalidLength { expected, actual } => write!(
                formatter,
                "network identifier must be {expected} characters, got {actual}"
            ),
            Self::InvalidPrefix { found } => write!(
                formatter,
                "network identifier prefix must be F (NeBe) or P (package), got {found}"
            ),
            Self::InvalidPackageIssuer { found } => write!(
                formatter,
                "package ID position 2 must be 9 (BDEW/electricity), got {found}"
            ),
            Self::InvalidCharacter { position, found } => write!(
                formatter,
                "network identifier contains invalid character '{found}' at position {position}"
            ),
            Self::NonNumericCheckDigit { found } => {
                write!(formatter, "check digit must be numeric, got {found}")
            }
            Self::ChecksumInput(error) => write!(formatter, "invalid checksum input: {error}"),
            Self::ChecksumMismatch { expected, actual } => write!(
                formatter,
                "invalid network identifier checksum: expected {expected}, got {actual}"
            ),
            Self::KindMismatch { expected, actual } => write!(
                formatter,
                "network identifier kind mismatch: expected {expected:?}, got {actual:?}"
            ),
        }
    }
}

impl Error for NetworkIdentifierError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ChecksumInput(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ChecksumInputError> for NetworkIdentifierError {
    fn from(error: ChecksumInputError) -> Self {
        Self::ChecksumInput(error)
    }
}

pub fn calculate_network_identifier_check_digit(base: &str) -> Result<u8, NetworkIdentifierError> {
    validate_base(base)?;
    bdew_ascii::calculate(base).map_err(Into::into)
}

pub fn validate_network_identifier(
    input: &str,
) -> Result<ValidatedEnergyIdentifier<NetworkIdentifierKind>, NetworkIdentifierError> {
    if !input.is_ascii() {
        return Err(NetworkIdentifierError::NonAscii);
    }
    if input.len() != FULL_LENGTH {
        return Err(NetworkIdentifierError::InvalidLength {
            expected: FULL_LENGTH,
            actual: input.len(),
        });
    }
    let kind = validate_base(&input[..BASE_LENGTH])?;
    let check_byte = input.as_bytes()[BASE_LENGTH];
    if !check_byte.is_ascii_digit() {
        return Err(NetworkIdentifierError::NonNumericCheckDigit {
            found: char::from(check_byte),
        });
    }
    let expected = bdew_ascii::from_valid_upper_alphanumeric(&input.as_bytes()[..BASE_LENGTH]);
    let actual = check_byte - b'0';
    if expected != actual {
        return Err(NetworkIdentifierError::ChecksumMismatch { expected, actual });
    }
    Ok(ValidatedEnergyIdentifier::new(
        input.to_string(),
        kind,
        actual,
    ))
}

pub fn validate_network_identifier_for_kind(
    input: &str,
    expected_kind: NetworkIdentifierKind,
) -> Result<ValidatedEnergyIdentifier<NetworkIdentifierKind>, NetworkIdentifierError> {
    let validated = validate_network_identifier(input)?;
    if validated.kind != expected_kind {
        return Err(NetworkIdentifierError::KindMismatch {
            expected: expected_kind,
            actual: validated.kind,
        });
    }
    Ok(validated)
}

pub fn validate_nebe_id(
    input: &str,
) -> Result<ValidatedEnergyIdentifier<NetworkIdentifierKind>, NetworkIdentifierError> {
    validate_network_identifier_for_kind(input, NetworkIdentifierKind::NetworkArea)
}

pub fn validate_package_id(
    input: &str,
) -> Result<ValidatedEnergyIdentifier<NetworkIdentifierKind>, NetworkIdentifierError> {
    validate_network_identifier_for_kind(input, NetworkIdentifierKind::Package)
}

pub fn generate_network_identifier(
    kind: NetworkIdentifierKind,
    fixture_seed: &str,
    index: u32,
) -> GeneratedEnergyIdentifier<NetworkIdentifierKind> {
    let mut rng = DeterministicRng::new(fixture_seed, kind.fixture_namespace(), index);
    let mut base = String::with_capacity(BASE_LENGTH);
    base.push(kind.prefix());
    if kind == NetworkIdentifierKind::Package {
        base.push('9');
    }
    while base.len() < BASE_LENGTH {
        base.push(rng.uppercase_alphanumeric());
    }
    let check_digit = bdew_ascii::from_valid_upper_alphanumeric(base.as_bytes());
    let mut value = base;
    value.push(char::from(b'0' + check_digit));
    GeneratedEnergyIdentifier::new(value, kind, check_digit)
}

pub fn generate_nebe_id(
    fixture_seed: &str,
    index: u32,
) -> GeneratedEnergyIdentifier<NetworkIdentifierKind> {
    generate_network_identifier(NetworkIdentifierKind::NetworkArea, fixture_seed, index)
}

pub fn generate_package_id(
    fixture_seed: &str,
    index: u32,
) -> GeneratedEnergyIdentifier<NetworkIdentifierKind> {
    generate_network_identifier(NetworkIdentifierKind::Package, fixture_seed, index)
}

fn validate_base(base: &str) -> Result<NetworkIdentifierKind, NetworkIdentifierError> {
    if !base.is_ascii() {
        return Err(NetworkIdentifierError::NonAscii);
    }
    if base.len() != BASE_LENGTH {
        return Err(NetworkIdentifierError::InvalidLength {
            expected: BASE_LENGTH,
            actual: base.len(),
        });
    }
    let bytes = base.as_bytes();
    let kind = match bytes[0] {
        b'F' => NetworkIdentifierKind::NetworkArea,
        b'P' => NetworkIdentifierKind::Package,
        other => {
            return Err(NetworkIdentifierError::InvalidPrefix {
                found: char::from(other),
            })
        }
    };
    if kind == NetworkIdentifierKind::Package && bytes[1] != b'9' {
        return Err(NetworkIdentifierError::InvalidPackageIssuer {
            found: char::from(bytes[1]),
        });
    }
    for (index, byte) in bytes.iter().enumerate().skip(1) {
        if !(byte.is_ascii_digit() || byte.is_ascii_uppercase()) {
            return Err(NetworkIdentifierError::InvalidCharacter {
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

    fn complete(base: &str) -> String {
        format!(
            "{base}{}",
            calculate_network_identifier_check_digit(base).unwrap()
        )
    }

    #[test]
    fn normative_prefixes_and_package_issuer_are_strict() {
        let nebe = complete("F123456789");
        let package = complete("P9ABCDEFGH");
        assert_eq!(
            validate_nebe_id(&nebe).unwrap().kind,
            NetworkIdentifierKind::NetworkArea
        );
        assert_eq!(
            validate_package_id(&package).unwrap().kind,
            NetworkIdentifierKind::Package
        );
        let invalid_package_base = "P8ABCDEFGH";
        let invalid_package = format!(
            "{invalid_package_base}{}",
            bdew_ascii::calculate(invalid_package_base).unwrap()
        );
        assert!(matches!(
            validate_network_identifier(&invalid_package),
            Err(NetworkIdentifierError::InvalidPackageIssuer { found: '8' })
        ));
        assert!(matches!(
            validate_package_id(&nebe),
            Err(NetworkIdentifierError::KindMismatch { .. })
        ));
    }

    #[test]
    fn checksum_mutations_and_unicode_are_rejected() {
        let mut value = complete("F123456789").into_bytes();
        value[10] = b'0' + ((value[10] - b'0' + 1) % 10);
        assert!(matches!(
            validate_nebe_id(&String::from_utf8(value).unwrap()),
            Err(NetworkIdentifierError::ChecksumMismatch { .. })
        ));
        assert_eq!(
            validate_network_identifier("F12345678ß5"),
            Err(NetworkIdentifierError::NonAscii)
        );
    }

    #[test]
    fn deterministic_generators_self_validate() {
        for index in 0..250 {
            let nebe = generate_nebe_id("fixture", index);
            let package = generate_package_id("fixture", index);
            assert!(validate_nebe_id(&nebe.value).is_ok());
            assert!(validate_package_id(&package.value).is_ok());
            assert_eq!(nebe, generate_nebe_id("fixture", index));
            assert_eq!(package, generate_package_id("fixture", index));
        }
    }
}
