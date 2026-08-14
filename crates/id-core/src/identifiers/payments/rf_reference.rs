//! ISO 11649 RF Creditor References.

use std::fmt;

use super::reference::deterministic_token;
use crate::checksum::mod97;

pub const RF_REFERENCE_MIN_LENGTH: usize = 5;
pub const RF_REFERENCE_MAX_LENGTH: usize = 25;
pub const RF_REFERENCE_BODY_MAX_LENGTH: usize = 21;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RfReferenceError {
    Empty,
    InvalidCharacter { position: usize, character: char },
    InvalidLength { actual: usize },
    InvalidPrefix,
    InvalidCheckDigits,
    InvalidReferenceBody,
    ChecksumMismatch,
    Checksum(mod97::Mod97Error),
}

impl fmt::Display for RfReferenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("RF creditor reference must not be empty"),
            Self::InvalidCharacter {
                position,
                character,
            } => write!(
                formatter,
                "invalid RF-reference character {character:?} at position {position}"
            ),
            Self::InvalidLength { actual } => write!(
                formatter,
                "RF creditor reference must be 5 to 25 characters, got {actual}"
            ),
            Self::InvalidPrefix => {
                formatter.write_str("RF creditor reference must start with 'RF'")
            }
            Self::InvalidCheckDigits => {
                formatter.write_str("RF-reference positions 3 and 4 must be decimal check digits")
            }
            Self::InvalidReferenceBody => formatter.write_str(
                "RF-reference body must contain 1 to 21 uppercase ASCII alphanumeric characters",
            ),
            Self::ChecksumMismatch => {
                formatter.write_str("RF-reference MOD-97 checksum is invalid")
            }
            Self::Checksum(error) => write!(formatter, "RF-reference checksum error: {error}"),
        }
    }
}

impl std::error::Error for RfReferenceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Checksum(error) => Some(error),
            _ => None,
        }
    }
}

impl From<mod97::Mod97Error> for RfReferenceError {
    fn from(value: mod97::Mod97Error) -> Self {
        Self::Checksum(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RfReferenceParts {
    pub electronic: String,
    pub formatted: String,
    pub check_digits: String,
    pub reference_body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedRfReference {
    pub value: String,
    pub formatted: String,
    pub parts: RfReferenceParts,
    pub synthetic: bool,
    pub generator_version: &'static str,
}

pub fn normalize_rf_reference(input: &str) -> Result<String, RfReferenceError> {
    if input.is_empty() {
        return Err(RfReferenceError::Empty);
    }
    let mut normalized = String::with_capacity(input.len());
    for (position, character) in input.chars().enumerate() {
        if character == ' ' {
            continue;
        }
        if character.is_ascii_alphanumeric() {
            normalized.push(character.to_ascii_uppercase());
        } else {
            return Err(RfReferenceError::InvalidCharacter {
                position: position + 1,
                character,
            });
        }
    }
    if normalized.is_empty() {
        return Err(RfReferenceError::Empty);
    }
    Ok(normalized)
}

/// Parses the ISO 11649 shape without claiming checksum validity.
pub fn parse_rf_reference(input: &str) -> Result<RfReferenceParts, RfReferenceError> {
    let electronic = normalize_rf_reference(input)?;
    if !(RF_REFERENCE_MIN_LENGTH..=RF_REFERENCE_MAX_LENGTH).contains(&electronic.len()) {
        return Err(RfReferenceError::InvalidLength {
            actual: electronic.len(),
        });
    }

    // Normalisation guarantees ASCII and therefore safe byte slicing.
    if &electronic[0..2] != "RF" {
        return Err(RfReferenceError::InvalidPrefix);
    }
    if !electronic[2..4].bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(RfReferenceError::InvalidCheckDigits);
    }
    if electronic[4..].is_empty()
        || electronic[4..].len() > RF_REFERENCE_BODY_MAX_LENGTH
        || !electronic[4..]
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
    {
        return Err(RfReferenceError::InvalidReferenceBody);
    }

    Ok(RfReferenceParts {
        formatted: group_in_fours(&electronic),
        check_digits: electronic[2..4].to_string(),
        reference_body: electronic[4..].to_string(),
        electronic,
    })
}

pub fn validate_rf_reference(input: &str) -> Result<RfReferenceParts, RfReferenceError> {
    let parts = parse_rf_reference(input)?;
    let rearranged = format!("{}RF{}", parts.reference_body, parts.check_digits);
    if !mod97::is_valid(&rearranged)? {
        return Err(RfReferenceError::ChecksumMismatch);
    }
    Ok(parts)
}

pub fn build_rf_reference(reference_body: &str) -> Result<GeneratedRfReference, RfReferenceError> {
    let body = normalize_reference_body(reference_body)?;
    let check_digits = mod97::calculate_check_digits(&format!("{body}RF00"))?;
    let value = format!("RF{check_digits}{body}");
    let parts = validate_rf_reference(&value)?;
    Ok(GeneratedRfReference {
        formatted: parts.formatted.clone(),
        value,
        parts,
        synthetic: true,
        generator_version: crate::GENERATOR_VERSION,
    })
}

/// Generates a maximum-length, checksum-valid synthetic RF reference.
pub fn generate_rf_reference(
    seed: &str,
    index: u32,
) -> Result<GeneratedRfReference, RfReferenceError> {
    let token = deterministic_token("payments.rf-reference", seed, index);
    let body = format!("NRG{}", &token[8..]);
    build_rf_reference(&body)
}

fn normalize_reference_body(input: &str) -> Result<String, RfReferenceError> {
    if input.is_empty() {
        return Err(RfReferenceError::InvalidReferenceBody);
    }
    let mut normalized = String::with_capacity(input.len());
    for (position, character) in input.chars().enumerate() {
        if character == ' ' {
            continue;
        }
        if character.is_ascii_alphanumeric() {
            normalized.push(character.to_ascii_uppercase());
        } else {
            return Err(RfReferenceError::InvalidCharacter {
                position: position + 1,
                character,
            });
        }
    }
    if normalized.is_empty()
        || normalized.len() > RF_REFERENCE_BODY_MAX_LENGTH
        || !normalized.is_ascii()
    {
        return Err(RfReferenceError::InvalidReferenceBody);
    }
    Ok(normalized)
}

fn group_in_fours(electronic: &str) -> String {
    let extra_spaces = electronic.len().saturating_sub(1) / 4;
    let mut formatted = String::with_capacity(electronic.len() + extra_spaces);
    for (index, byte) in electronic.bytes().enumerate() {
        if index > 0 && index % 4 == 0 {
            formatted.push(' ');
        }
        formatted.push(char::from(byte));
    }
    formatted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_epc_published_example() {
        let parts = validate_rf_reference("RF18 5390 0754 7034").unwrap();
        assert_eq!(parts.electronic, "RF18539007547034");
        assert_eq!(parts.check_digits, "18");
        assert_eq!(parts.reference_body, "539007547034");
    }

    #[test]
    fn builds_reference_from_invoice_body() {
        let generated = build_rf_reference("NRG202600001234").unwrap();
        assert!(generated.value.starts_with("RF"));
        assert_eq!(generated.parts.reference_body, "NRG202600001234");
        assert_eq!(
            validate_rf_reference(&generated.value).unwrap(),
            generated.parts
        );
    }

    #[test]
    fn check_digit_and_body_mutations_are_rejected() {
        assert_eq!(
            validate_rf_reference("RF17539007547034"),
            Err(RfReferenceError::ChecksumMismatch)
        );
        assert_eq!(
            validate_rf_reference("RF18539007547035"),
            Err(RfReferenceError::ChecksumMismatch)
        );
    }

    #[test]
    fn generator_is_reproducible_bounded_and_self_validating() {
        let mut values = std::collections::HashSet::new();
        for index in 0..500 {
            let generated = generate_rf_reference("fixture", index).unwrap();
            assert_eq!(generated.value.len(), RF_REFERENCE_MAX_LENGTH);
            assert_eq!(generated, generate_rf_reference("fixture", index).unwrap());
            assert_eq!(
                validate_rf_reference(&generated.value).unwrap(),
                generated.parts
            );
            assert!(values.insert(generated.value));
        }
    }

    #[test]
    fn normalizes_case_and_spaces_but_rejects_other_separators_and_unicode() {
        assert_eq!(
            normalize_rf_reference("rf18 5390 0754 7034").unwrap(),
            "RF18539007547034"
        );
        for invalid in ["RF18-5390", "RF18Ä123", "😀"] {
            assert!(matches!(
                normalize_rf_reference(invalid),
                Err(RfReferenceError::InvalidCharacter { .. })
            ));
        }
        assert!(matches!(
            build_rf_reference(&"A".repeat(22)),
            Err(RfReferenceError::InvalidReferenceBody)
        ));
    }
}
