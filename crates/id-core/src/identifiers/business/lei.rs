//! ISO 17442 Legal Entity Identifier (LEI) parsing and validation.
//!
//! LEIs are allocated by supervised LEI Issuers and published through GLEIF.
//! A valid MOD-97 checksum detects likely transcription errors; it does not
//! establish registration, status, entity identity, or production usability.
//! Consequently this module deliberately exposes no LEI generator.

use std::fmt;

use crate::checksum::mod97;

pub const LEI_LENGTH: usize = 20;
pub const GLEIF_API_LEI_RECORDS_URL: &str = "https://api.gleif.org/api/v1/lei-records";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeiLookupStatus {
    /// The transport-neutral core performs no live GLEIF request.
    NotPerformed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeiAssignmentStatus {
    /// Checksum validity alone is not evidence of an issued LEI.
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeiError {
    Empty,
    InvalidLength { expected: usize, actual: usize },
    InvalidCharacter { position: usize, character: char },
    InvalidCheckDigits,
    ChecksumMismatch,
    Checksum(mod97::Mod97Error),
}

impl fmt::Display for LeiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("LEI must not be empty"),
            Self::InvalidLength { expected, actual } => {
                write!(formatter, "LEI must be {expected} characters, got {actual}")
            }
            Self::InvalidCharacter {
                position,
                character,
            } => write!(
                formatter,
                "invalid LEI character {character:?} at position {position}; uppercase ASCII alphanumeric is required",
            ),
            Self::InvalidCheckDigits => {
                formatter.write_str("LEI positions 19 and 20 must be decimal check digits")
            }
            Self::ChecksumMismatch => formatter.write_str("LEI MOD-97 checksum is invalid"),
            Self::Checksum(error) => write!(formatter, "LEI checksum error: {error}"),
        }
    }
}

impl std::error::Error for LeiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Checksum(error) => Some(error),
            _ => None,
        }
    }
}

impl From<mod97::Mod97Error> for LeiError {
    fn from(value: mod97::Mod97Error) -> Self {
        Self::Checksum(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeiParts {
    pub value: String,
    pub issuer_prefix: String,
    pub entity_specific: String,
    pub check_digits: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeiValidation {
    pub parts: LeiParts,
    pub syntax_valid: bool,
    pub checksum_valid: bool,
    pub lookup_status: LeiLookupStatus,
    pub assignment_status: LeiAssignmentStatus,
}

/// Parses the GLEIF/ISO shape without claiming checksum or registry validity.
///
/// LEIs have no presentation form: lower-case letters, spaces, separators and
/// all non-ASCII characters are rejected instead of normalized.
pub fn parse_lei(input: &str) -> Result<LeiParts, LeiError> {
    if input.is_empty() {
        return Err(LeiError::Empty);
    }
    for (position, character) in input.chars().enumerate() {
        if !(character.is_ascii_uppercase() || character.is_ascii_digit()) {
            return Err(LeiError::InvalidCharacter {
                position: position + 1,
                character,
            });
        }
    }
    if input.len() != LEI_LENGTH {
        return Err(LeiError::InvalidLength {
            expected: LEI_LENGTH,
            actual: input.len(),
        });
    }
    // Character validation guarantees ASCII before byte slicing.
    if !input[18..20].bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(LeiError::InvalidCheckDigits);
    }

    Ok(LeiParts {
        issuer_prefix: input[..4].to_string(),
        entity_specific: input[4..18].to_string(),
        check_digits: input[18..20].to_string(),
        value: input.to_string(),
    })
}

pub fn validate_lei(input: &str) -> Result<LeiValidation, LeiError> {
    let parts = parse_lei(input)?;
    if !mod97::is_valid(&parts.value)? {
        return Err(LeiError::ChecksumMismatch);
    }
    Ok(LeiValidation {
        parts,
        syntax_valid: true,
        checksum_valid: true,
        lookup_status: LeiLookupStatus::NotPerformed,
        assignment_status: LeiAssignmentStatus::Unknown,
    })
}

/// Returns the canonical GLEIF record endpoint after complete offline
/// validation.  Constructing this URL performs no lookup and must not be
/// represented as registry evidence.
pub fn gleif_record_api_url(input: &str) -> Result<String, LeiError> {
    let report = validate_lei(input)?;
    Ok(format!(
        "{GLEIF_API_LEI_RECORDS_URL}/{}",
        report.parts.value
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const GLEIF_LEI: &str = "506700GE1G29325QX363";

    #[test]
    fn validates_the_lei_published_for_gleif() {
        let report = validate_lei(GLEIF_LEI).unwrap();
        assert_eq!(report.parts.issuer_prefix, "5067");
        assert_eq!(report.parts.entity_specific, "00GE1G29325QX3");
        assert_eq!(report.parts.check_digits, "63");
        assert!(report.syntax_valid);
        assert!(report.checksum_valid);
        assert_eq!(report.lookup_status, LeiLookupStatus::NotPerformed);
        assert_eq!(report.assignment_status, LeiAssignmentStatus::Unknown);
    }

    #[test]
    fn every_single_check_digit_mutation_is_rejected() {
        for position in [18_usize, 19_usize] {
            for replacement in b'0'..=b'9' {
                if replacement == GLEIF_LEI.as_bytes()[position] {
                    continue;
                }
                let mut mutated = GLEIF_LEI.as_bytes().to_vec();
                mutated[position] = replacement;
                let mutated = String::from_utf8(mutated).unwrap();
                assert_eq!(validate_lei(&mutated), Err(LeiError::ChecksumMismatch));
            }
        }
    }

    #[test]
    fn parser_does_not_confuse_shape_with_checksum_validity() {
        let mutated = "506700GE1G29325QX362";
        assert!(parse_lei(mutated).is_ok());
        assert_eq!(validate_lei(mutated), Err(LeiError::ChecksumMismatch));
    }

    #[test]
    fn checksum_valid_shapes_always_round_trip_without_assignment_claims() {
        for number in 0..1_000_u32 {
            // Test-only bodies exercise MOD-97 over digits and letters.  This
            // is intentionally not exposed as a public LEI generator.
            let body = format!("529900NRG{number:09}");
            assert_eq!(body.len(), 18);
            let check_digits = mod97::calculate_check_digits(&format!("{body}00")).unwrap();
            let candidate = format!("{body}{check_digits}");
            let report = validate_lei(&candidate).unwrap();
            assert_eq!(report.parts.value, candidate);
            assert_eq!(report.lookup_status, LeiLookupStatus::NotPerformed);
            assert_eq!(report.assignment_status, LeiAssignmentStatus::Unknown);
        }
    }

    #[test]
    fn rejects_lowercase_separators_bad_check_shape_and_unicode() {
        for input in [
            "506700ge1G29325QX363",
            "5067 00GE1G29325QX363",
            "506700GE1G29325QX3A3",
            "506700GE1G29325QX36😀",
            "５０６７00GE1G29325QX363",
        ] {
            assert!(parse_lei(input).is_err(), "accepted {input:?}");
        }
        assert_eq!(
            parse_lei("506700GE1G29325QX3AA"),
            Err(LeiError::InvalidCheckDigits)
        );
    }

    #[test]
    fn lookup_url_requires_full_offline_validity_but_does_not_lookup() {
        assert_eq!(
            gleif_record_api_url(GLEIF_LEI).unwrap(),
            "https://api.gleif.org/api/v1/lei-records/506700GE1G29325QX363"
        );
        assert!(gleif_record_api_url("506700GE1G29325QX362").is_err());
    }
}
