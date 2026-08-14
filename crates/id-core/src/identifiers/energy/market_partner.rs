//! BDEW (electricity) and DVGW (gas) market-partner IDs.
//!
//! Both forms are centrally allocated, numeric 13-character identifiers. The
//! first two digits are `99` for BDEW/electricity and `98` for DVGW/gas. The
//! third digit is an allocation-mode digit (`0..=8` for BDEW, `0..=9` for
//! DVGW), positions 4 through 12 are digits, and position 13 is a Lok-und-
//! Waggon check digit.

use std::error::Error;
use std::fmt;

use super::checksum::{self, ChecksumInputError};
use super::{GeneratedEnergyIdentifier, ValidatedEnergyIdentifier};
use crate::fixture::DeterministicRng;

const BASE_LENGTH: usize = 12;
const FULL_LENGTH: usize = 13;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MarketPartnerIdKind {
    BdewElectricity,
    DvgwGas,
}

impl MarketPartnerIdKind {
    pub const fn prefix(self) -> &'static str {
        match self {
            Self::BdewElectricity => "99",
            Self::DvgwGas => "98",
        }
    }

    pub const fn slug(self) -> &'static str {
        match self {
            Self::BdewElectricity => "bdew-mp-id",
            Self::DvgwGas => "dvgw-mp-id",
        }
    }

    const fn fixture_namespace(self) -> &'static str {
        match self {
            Self::BdewElectricity => "energy.market-partner.bdew",
            Self::DvgwGas => "energy.market-partner.dvgw",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarketPartnerIdError {
    NonAscii,
    InvalidLength {
        expected: usize,
        actual: usize,
    },
    NonDigit {
        position: usize,
        found: char,
    },
    InvalidPrefix {
        first: char,
        second: char,
    },
    InvalidBdewAllocationMode {
        found: char,
    },
    ChecksumInput(ChecksumInputError),
    ChecksumMismatch {
        expected: u8,
        actual: u8,
    },
    KindMismatch {
        expected: MarketPartnerIdKind,
        actual: MarketPartnerIdKind,
    },
}

impl fmt::Display for MarketPartnerIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonAscii => {
                formatter.write_str("market-partner ID must contain only ASCII digits")
            }
            Self::InvalidLength { expected, actual } => write!(
                formatter,
                "market-partner ID must be {expected} digits, got {actual}"
            ),
            Self::NonDigit { position, found } => write!(
                formatter,
                "market-partner ID contains non-digit '{found}' at 1-based position {position}"
            ),
            Self::InvalidPrefix { first, second } => write!(
                formatter,
                "market-partner ID prefix must be 99 (BDEW) or 98 (DVGW), got {first}{second}"
            ),
            Self::InvalidBdewAllocationMode { found } => write!(
                formatter,
                "BDEW market-partner ID position 3 must be a digit from 0 through 8, got {found}"
            ),
            Self::ChecksumInput(error) => write!(formatter, "invalid checksum input: {error}"),
            Self::ChecksumMismatch { expected, actual } => write!(
                formatter,
                "invalid market-partner ID checksum: expected {expected}, got {actual}"
            ),
            Self::KindMismatch { expected, actual } => write!(
                formatter,
                "market-partner ID kind mismatch: expected {expected:?}, got {actual:?}"
            ),
        }
    }
}

impl Error for MarketPartnerIdError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ChecksumInput(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ChecksumInputError> for MarketPartnerIdError {
    fn from(value: ChecksumInputError) -> Self {
        Self::ChecksumInput(value)
    }
}

/// Calculates the check digit for a complete 12-digit BDEW/DVGW MP-ID base.
///
/// This validates the issuer prefix and the BDEW-specific third-position rule
/// before applying the Lok-und-Waggon procedure.
pub fn calculate_market_partner_check_digit(base: &str) -> Result<u8, MarketPartnerIdError> {
    validate_base(base)?;
    checksum::calculate_lok_waggon_checksum(base).map_err(Into::into)
}

/// Validates syntax and checksum and infers the BDEW/DVGW form.
///
/// No central directory is queried. A successful result therefore retains
/// `allocation_status = Unknown`, including for known registry values.
pub fn validate_market_partner_id(
    input: &str,
) -> Result<ValidatedEnergyIdentifier<MarketPartnerIdKind>, MarketPartnerIdError> {
    if !input.is_ascii() {
        return Err(MarketPartnerIdError::NonAscii);
    }
    if input.len() != FULL_LENGTH {
        return Err(MarketPartnerIdError::InvalidLength {
            expected: FULL_LENGTH,
            actual: input.len(),
        });
    }

    let kind = validate_base(&input[..BASE_LENGTH])?;
    let check_byte = input.as_bytes()[BASE_LENGTH];
    if !check_byte.is_ascii_digit() {
        return Err(MarketPartnerIdError::NonDigit {
            position: FULL_LENGTH,
            found: char::from(check_byte),
        });
    }
    let expected = checksum::lok_waggon_from_valid_ascii_digits(&input.as_bytes()[..BASE_LENGTH]);
    let actual = check_byte - b'0';
    if actual != expected {
        return Err(MarketPartnerIdError::ChecksumMismatch { expected, actual });
    }

    Ok(ValidatedEnergyIdentifier::new(
        input.to_owned(),
        kind,
        actual,
    ))
}

pub fn validate_market_partner_id_for_kind(
    input: &str,
    expected_kind: MarketPartnerIdKind,
) -> Result<ValidatedEnergyIdentifier<MarketPartnerIdKind>, MarketPartnerIdError> {
    let validated = validate_market_partner_id(input)?;
    if validated.kind != expected_kind {
        return Err(MarketPartnerIdError::KindMismatch {
            expected: expected_kind,
            actual: validated.kind,
        });
    }
    Ok(validated)
}

pub fn validate_bdew_market_partner_id(
    input: &str,
) -> Result<ValidatedEnergyIdentifier<MarketPartnerIdKind>, MarketPartnerIdError> {
    validate_market_partner_id_for_kind(input, MarketPartnerIdKind::BdewElectricity)
}

pub fn validate_dvgw_market_partner_id(
    input: &str,
) -> Result<ValidatedEnergyIdentifier<MarketPartnerIdKind>, MarketPartnerIdError> {
    validate_market_partner_id_for_kind(input, MarketPartnerIdKind::DvgwGas)
}

/// Creates a reproducible, format- and checksum-valid fixture.
///
/// The generated number is not reserved with BDEW or DVGW and may collide with
/// a centrally allocated value. Callers must preserve the returned
/// `allocation_status = Unknown` and must not treat the value as production
/// usable solely because it validates.
pub fn generate_market_partner_id(
    kind: MarketPartnerIdKind,
    fixture_seed: &str,
    index: u32,
) -> GeneratedEnergyIdentifier<MarketPartnerIdKind> {
    let mut stream = DeterministicRng::new(fixture_seed, kind.fixture_namespace(), index);
    let mut base = String::with_capacity(BASE_LENGTH);
    base.push_str(kind.prefix());

    let allocation_mode_upper_bound = match kind {
        MarketPartnerIdKind::BdewElectricity => 9,
        MarketPartnerIdKind::DvgwGas => 10,
    };
    base.push(ascii_digit(stream.index(allocation_mode_upper_bound) as u8));
    for _ in 0..9 {
        base.push(ascii_digit(stream.digit()));
    }

    let check_digit = checksum::lok_waggon_from_valid_ascii_digits(base.as_bytes());
    let mut value = base;
    value.push(ascii_digit(check_digit));
    GeneratedEnergyIdentifier::new(value, kind, check_digit)
}

pub fn generate_bdew_market_partner_id(
    fixture_seed: &str,
    index: u32,
) -> GeneratedEnergyIdentifier<MarketPartnerIdKind> {
    generate_market_partner_id(MarketPartnerIdKind::BdewElectricity, fixture_seed, index)
}

pub fn generate_dvgw_market_partner_id(
    fixture_seed: &str,
    index: u32,
) -> GeneratedEnergyIdentifier<MarketPartnerIdKind> {
    generate_market_partner_id(MarketPartnerIdKind::DvgwGas, fixture_seed, index)
}

fn validate_base(base: &str) -> Result<MarketPartnerIdKind, MarketPartnerIdError> {
    if !base.is_ascii() {
        return Err(MarketPartnerIdError::NonAscii);
    }
    if base.len() != BASE_LENGTH {
        return Err(MarketPartnerIdError::InvalidLength {
            expected: BASE_LENGTH,
            actual: base.len(),
        });
    }

    for (index, byte) in base.bytes().enumerate() {
        if !byte.is_ascii_digit() {
            return Err(MarketPartnerIdError::NonDigit {
                position: index + 1,
                found: char::from(byte),
            });
        }
    }

    let bytes = base.as_bytes();
    let kind = match &bytes[..2] {
        b"99" => MarketPartnerIdKind::BdewElectricity,
        b"98" => MarketPartnerIdKind::DvgwGas,
        _ => {
            return Err(MarketPartnerIdError::InvalidPrefix {
                first: char::from(bytes[0]),
                second: char::from(bytes[1]),
            })
        }
    };

    if kind == MarketPartnerIdKind::BdewElectricity && bytes[2] == b'9' {
        return Err(MarketPartnerIdError::InvalidBdewAllocationMode { found: '9' });
    }
    Ok(kind)
}

fn ascii_digit(digit: u8) -> char {
    char::from(b'0' + digit)
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
    fn accepts_officially_published_bdew_registry_value_without_claiming_allocation() {
        // Published by BDEW in "Redispatch 2.0: Information zu Marktrollen,
        // Verantwortlichkeiten und MP-ID" (2021-05-03), page 3.
        let result = validate_bdew_market_partner_id("9979425000005").unwrap();
        assert_eq!(result.kind, MarketPartnerIdKind::BdewElectricity);
        assert_eq!(result.check_digit, 5);
        assert_eq!(result.allocation_status, CentralAllocationStatus::Unknown);
    }

    #[test]
    fn accepts_derived_dvgw_format_vector() {
        // Derived directly from the v1.2 format and checksum rules. The source
        // does not designate this value as allocated or as an official fixture.
        let result = validate_dvgw_market_partner_id("9801234567895").unwrap();
        assert_eq!(result.kind, MarketPartnerIdKind::DvgwGas);
        assert_eq!(result.check_digit, 5);
    }

    #[test]
    fn checksum_mutation_is_rejected() {
        let mutated = mutate_check_digit("9979425000005");
        assert!(matches!(
            validate_market_partner_id(&mutated),
            Err(MarketPartnerIdError::ChecksumMismatch { .. })
        ));
    }

    #[test]
    fn bdew_rejects_allocation_mode_nine_but_dvgw_allows_it() {
        let bdew_base = "999123456789";
        let bdew_check = checksum::calculate_lok_waggon_checksum(bdew_base).unwrap();
        assert_eq!(
            validate_market_partner_id(&format!("{bdew_base}{bdew_check}")),
            Err(MarketPartnerIdError::InvalidBdewAllocationMode { found: '9' })
        );

        let dvgw_base = "989123456789";
        let dvgw_check = checksum::calculate_lok_waggon_checksum(dvgw_base).unwrap();
        assert!(validate_dvgw_market_partner_id(&format!("{dvgw_base}{dvgw_check}")).is_ok());
    }

    #[test]
    fn validators_are_strict_and_unicode_safe() {
        assert_eq!(
            validate_market_partner_id("997942500000٥"),
            Err(MarketPartnerIdError::NonAscii)
        );
        assert!(matches!(
            validate_market_partner_id("99794250000A5"),
            Err(MarketPartnerIdError::NonDigit { position: 12, .. })
        ));
        assert!(matches!(
            validate_market_partner_id("997942500000A"),
            Err(MarketPartnerIdError::NonDigit { position: 13, .. })
        ));
        assert!(matches!(
            validate_market_partner_id("9701234567890"),
            Err(MarketPartnerIdError::InvalidPrefix { .. })
        ));
    }

    #[test]
    fn deterministic_generators_are_reproducible_and_self_validating() {
        for index in 0..100 {
            for kind in [
                MarketPartnerIdKind::BdewElectricity,
                MarketPartnerIdKind::DvgwGas,
            ] {
                let first = generate_market_partner_id(kind, "integration-test-4711", index);
                let second = generate_market_partner_id(kind, "integration-test-4711", index);
                assert_eq!(first, second);
                assert_eq!(first.allocation_status, CentralAllocationStatus::Unknown);
                assert_eq!(
                    validate_market_partner_id_for_kind(&first.value, kind)
                        .unwrap()
                        .check_digit,
                    first.check_digit
                );
            }
        }

        assert_ne!(
            generate_bdew_market_partner_id("integration-test-4711", 0).value,
            generate_bdew_market_partner_id("integration-test-4711", 1).value
        );
        assert_ne!(
            generate_bdew_market_partner_id("integration-test-4711", 0).value,
            generate_bdew_market_partner_id("another-seed", 0).value
        );
    }

    #[test]
    fn generator_version_one_snapshots_are_stable() {
        assert_eq!(crate::GENERATOR_VERSION, "1");
        assert_eq!(
            generate_bdew_market_partner_id("integration-test-4711", 0).value,
            "9936030512106"
        );
        assert_eq!(
            generate_dvgw_market_partner_id("integration-test-4711", 0).value,
            "9834065834331"
        );
    }
}
