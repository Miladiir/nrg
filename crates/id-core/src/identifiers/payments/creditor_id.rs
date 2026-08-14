//! German SEPA Creditor Identifiers.
//!
//! German identifiers are 18 characters long. Their three-character Creditor
//! Business Code is deliberately excluded from the MOD 97-10 calculation.

use std::fmt;

use crate::checksum::mod97;
use crate::fixture::DeterministicRng;

pub const GERMAN_CREDITOR_ID_LENGTH: usize = 18;
pub const OFFICIAL_GERMAN_TEST_CREDITOR_ID: &str = "DE98ZZZ09999999999";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreditorIdError {
    Empty,
    InvalidCharacter { position: usize, character: char },
    InvalidLength { expected: usize, actual: usize },
    UnsupportedCountry { country: String },
    InvalidCheckDigits,
    InvalidBusinessCode,
    InvalidNationalIdentifier,
    ChecksumMismatch,
    Checksum(mod97::Mod97Error),
}

impl fmt::Display for CreditorIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("creditor ID must not be empty"),
            Self::InvalidCharacter {
                position,
                character,
            } => write!(
                formatter,
                "invalid creditor-ID character {character:?} at position {position}"
            ),
            Self::InvalidLength { expected, actual } => write!(
                formatter,
                "German creditor ID must be {expected} characters, got {actual}"
            ),
            Self::UnsupportedCountry { country } => write!(
                formatter,
                "only German creditor IDs are supported, got {country:?}"
            ),
            Self::InvalidCheckDigits => {
                formatter.write_str("creditor-ID positions 3 and 4 must be decimal check digits")
            }
            Self::InvalidBusinessCode => formatter.write_str(
                "creditor business code must be 3 uppercase ASCII alphanumeric characters",
            ),
            Self::InvalidNationalIdentifier => formatter.write_str(
                "German national creditor identifier must be 11 digits and currently start with 0",
            ),
            Self::ChecksumMismatch => formatter.write_str("creditor-ID MOD-97 checksum is invalid"),
            Self::Checksum(error) => write!(formatter, "creditor-ID checksum error: {error}"),
        }
    }
}

impl std::error::Error for CreditorIdError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Checksum(error) => Some(error),
            _ => None,
        }
    }
}

impl From<mod97::Mod97Error> for CreditorIdError {
    fn from(value: mod97::Mod97Error) -> Self {
        Self::Checksum(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GermanCreditorIdParts {
    pub electronic: String,
    pub country_code: String,
    pub check_digits: String,
    pub business_code: String,
    pub national_identifier: String,
    pub official_test_fixture: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedGermanCreditorId {
    pub value: String,
    pub parts: GermanCreditorIdParts,
    pub synthetic: bool,
    pub generator_version: &'static str,
}

pub fn normalize_creditor_id(input: &str) -> Result<String, CreditorIdError> {
    if input.is_empty() {
        return Err(CreditorIdError::Empty);
    }
    let mut normalized = String::with_capacity(input.len());
    for (position, character) in input.chars().enumerate() {
        if character == ' ' {
            continue;
        }
        if character.is_ascii_alphanumeric() {
            normalized.push(character.to_ascii_uppercase());
        } else {
            return Err(CreditorIdError::InvalidCharacter {
                position: position + 1,
                character,
            });
        }
    }
    if normalized.is_empty() {
        return Err(CreditorIdError::Empty);
    }
    Ok(normalized)
}

/// Parses the German shape without claiming checksum validity.
pub fn parse_german_creditor_id(input: &str) -> Result<GermanCreditorIdParts, CreditorIdError> {
    let electronic = normalize_creditor_id(input)?;
    if electronic.len() != GERMAN_CREDITOR_ID_LENGTH {
        return Err(CreditorIdError::InvalidLength {
            expected: GERMAN_CREDITOR_ID_LENGTH,
            actual: electronic.len(),
        });
    }

    // Normalisation guarantees ASCII and therefore safe byte slicing.
    if &electronic[0..2] != "DE" {
        return Err(CreditorIdError::UnsupportedCountry {
            country: electronic[0..2].to_string(),
        });
    }
    if !electronic[2..4].bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(CreditorIdError::InvalidCheckDigits);
    }
    if !electronic[4..7]
        .bytes()
        .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
    {
        return Err(CreditorIdError::InvalidBusinessCode);
    }
    if electronic.as_bytes()[7] != b'0'
        || !electronic[7..18].bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(CreditorIdError::InvalidNationalIdentifier);
    }

    Ok(GermanCreditorIdParts {
        country_code: electronic[0..2].to_string(),
        check_digits: electronic[2..4].to_string(),
        business_code: electronic[4..7].to_string(),
        national_identifier: electronic[7..18].to_string(),
        official_test_fixture: electronic == OFFICIAL_GERMAN_TEST_CREDITOR_ID,
        electronic,
    })
}

pub fn validate_german_creditor_id(input: &str) -> Result<GermanCreditorIdParts, CreditorIdError> {
    let parts = parse_german_creditor_id(input)?;

    // Per EPC/Bundesbank rules, positions 5..=7 (business code) do not
    // participate in the checksum.
    let checksum_input = format!(
        "{}{}{}",
        parts.national_identifier, parts.country_code, parts.check_digits
    );
    if !mod97::is_valid(&checksum_input)? {
        return Err(CreditorIdError::ChecksumMismatch);
    }
    Ok(parts)
}

pub fn validate_creditor_id(input: &str) -> Result<GermanCreditorIdParts, CreditorIdError> {
    validate_german_creditor_id(input)
}

pub fn build_german_creditor_id(
    business_code: &str,
    national_identifier: &str,
) -> Result<GeneratedGermanCreditorId, CreditorIdError> {
    let business_code = normalize_business_code(business_code)?;
    if national_identifier.len() != 11
        || !national_identifier
            .bytes()
            .all(|byte| byte.is_ascii_digit())
        || !national_identifier.starts_with('0')
    {
        return Err(CreditorIdError::InvalidNationalIdentifier);
    }

    let check_digits = mod97::calculate_check_digits(&format!("{national_identifier}DE00"))?;
    let value = format!("DE{check_digits}{business_code}{national_identifier}");
    let parts = validate_german_creditor_id(&value)?;
    Ok(GeneratedGermanCreditorId {
        value,
        parts,
        synthetic: true,
        generator_version: crate::GENERATOR_VERSION,
    })
}

/// Returns the test value explicitly published by the Deutsche Bundesbank.
/// Seed and index are accepted for a uniform generator call shape; an official
/// fixture is intentionally invariant under both.
pub fn generate_creditor_id_official_test_fixture(
    _seed: &str,
    _index: u32,
) -> Result<GeneratedGermanCreditorId, CreditorIdError> {
    let parts = validate_german_creditor_id(OFFICIAL_GERMAN_TEST_CREDITOR_ID)?;
    Ok(GeneratedGermanCreditorId {
        value: OFFICIAL_GERMAN_TEST_CREDITOR_ID.to_string(),
        parts,
        synthetic: true,
        generator_version: crate::GENERATOR_VERSION,
    })
}

/// Generates a checksum-valid value with unknown allocation status. Prefer the
/// official fixture unless a varying deterministic value is explicitly needed.
pub fn generate_creditor_id_checksum_only(
    seed: &str,
    index: u32,
) -> Result<GeneratedGermanCreditorId, CreditorIdError> {
    let mut rng = DeterministicRng::new(seed, "payments.creditor-id.checksum-only", index);
    let mut national_identifier = String::from("0");
    for _ in 0..10 {
        national_identifier.push(char::from(b'0' + rng.digit()));
    }
    build_german_creditor_id("ZZZ", &national_identifier)
}

fn normalize_business_code(input: &str) -> Result<String, CreditorIdError> {
    if input.len() != 3 || !input.is_ascii() {
        return Err(CreditorIdError::InvalidBusinessCode);
    }
    let normalized = input.to_ascii_uppercase();
    if !normalized
        .bytes()
        .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
    {
        return Err(CreditorIdError::InvalidBusinessCode);
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_bundesbank_official_test_fixture() {
        let parts = validate_german_creditor_id("DE98 ZZZ 0 9999999999").unwrap();
        assert_eq!(parts.electronic, OFFICIAL_GERMAN_TEST_CREDITOR_ID);
        assert!(parts.official_test_fixture);
        assert_eq!(parts.business_code, "ZZZ");
        assert_eq!(parts.national_identifier, "09999999999");
    }

    #[test]
    fn business_code_is_excluded_from_checksum() {
        let parts = validate_german_creditor_id("DE98ABC09999999999").unwrap();
        assert_eq!(parts.business_code, "ABC");
        assert!(!parts.official_test_fixture);
    }

    #[test]
    fn checksum_mutations_are_rejected() {
        for invalid in ["DE97ZZZ09999999999", "DE98ZZZ09999999998"] {
            assert_eq!(
                validate_german_creditor_id(invalid),
                Err(CreditorIdError::ChecksumMismatch),
                "accepted {invalid}"
            );
        }
    }

    #[test]
    fn rejects_bad_national_shape_and_unicode_without_panicking() {
        assert_eq!(
            parse_german_creditor_id("DE98ZZZ19999999999"),
            Err(CreditorIdError::InvalidNationalIdentifier)
        );
        for input in ["DÉ98ZZZ09999999999", "DE98ZZZ0😀999999999"] {
            assert!(matches!(
                parse_german_creditor_id(input),
                Err(CreditorIdError::InvalidCharacter { .. })
            ));
        }
    }

    #[test]
    fn official_generator_is_invariant_and_marks_fixture() {
        let first = generate_creditor_id_official_test_fixture("a", 0).unwrap();
        let second = generate_creditor_id_official_test_fixture("😀", u32::MAX).unwrap();
        assert_eq!(first, second);
        assert!(first.parts.official_test_fixture);
        assert!(first.synthetic);
    }

    #[test]
    fn checksum_only_generation_is_reproducible_and_self_validating() {
        for index in 0..250 {
            let generated = generate_creditor_id_checksum_only("fixture", index).unwrap();
            assert_eq!(
                generated,
                generate_creditor_id_checksum_only("fixture", index).unwrap()
            );
            assert_eq!(
                validate_german_creditor_id(&generated.value).unwrap(),
                generated.parts
            );
        }
    }
}
