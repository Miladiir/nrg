//! German VAT identification numbers (Umsatzsteuer-Identifikationsnummer).
//!
//! The official public format is `DE` followed by nine decimal digits.  The
//! German national validation algorithm is not published by the European
//! Commission, so this module makes no offline checksum claim.  Assignment and
//! current validity require a BZSt/VIES query outside this transport-neutral
//! core module.

use std::fmt;

pub const GERMAN_VAT_ID_LENGTH: usize = 11;
pub const VIES_VALIDATION_URL: &str = "https://ec.europa.eu/taxation_customs/vies/";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VatIdChecksumStatus {
    /// No official public national algorithm is available for an offline check.
    NotAvailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VatIdLookupStatus {
    /// This core validator performs no network access.
    NotPerformed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VatIdAssignmentStatus {
    /// Format alone cannot establish whether BZSt assigned the identifier.
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GermanVatIdError {
    Empty,
    InvalidCharacter { position: usize, character: char },
    InvalidLength { expected: usize, actual: usize },
    InvalidCountryPrefix,
    InvalidNationalIdentifier,
}

impl fmt::Display for GermanVatIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("German VAT ID must not be empty"),
            Self::InvalidCharacter {
                position,
                character,
            } => write!(
                formatter,
                "invalid German VAT-ID character {character:?} at position {position}",
            ),
            Self::InvalidLength { expected, actual } => write!(
                formatter,
                "German VAT ID must be {expected} characters, got {actual}",
            ),
            Self::InvalidCountryPrefix => formatter.write_str("German VAT ID must start with 'DE'"),
            Self::InvalidNationalIdentifier => formatter
                .write_str("German VAT-ID national identifier must contain exactly nine digits"),
        }
    }
}

impl std::error::Error for GermanVatIdError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GermanVatIdParts {
    pub electronic: String,
    pub country_code: String,
    pub national_identifier: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GermanVatIdValidation {
    pub parts: GermanVatIdParts,
    pub syntax_valid: bool,
    pub checksum_status: VatIdChecksumStatus,
    pub lookup_status: VatIdLookupStatus,
    pub assignment_status: VatIdAssignmentStatus,
}

/// Converts the common spaced/lower-case presentation to electronic form.
///
/// Only ASCII letters, ASCII digits, and literal spaces are accepted.  This
/// keeps visually confusable Unicode characters out of business identifiers.
pub fn normalize_german_vat_id(input: &str) -> Result<String, GermanVatIdError> {
    if input.is_empty() {
        return Err(GermanVatIdError::Empty);
    }

    let mut normalized = String::with_capacity(input.len());
    for (position, character) in input.chars().enumerate() {
        if character == ' ' {
            continue;
        }
        if character.is_ascii_alphanumeric() {
            normalized.push(character.to_ascii_uppercase());
        } else {
            return Err(GermanVatIdError::InvalidCharacter {
                position: position + 1,
                character,
            });
        }
    }
    if normalized.is_empty() {
        return Err(GermanVatIdError::Empty);
    }
    Ok(normalized)
}

/// Parses the official German shape without claiming assignment or validity in
/// the BZSt/VIES register.
pub fn parse_german_vat_id(input: &str) -> Result<GermanVatIdParts, GermanVatIdError> {
    let electronic = normalize_german_vat_id(input)?;
    if electronic.len() != GERMAN_VAT_ID_LENGTH {
        return Err(GermanVatIdError::InvalidLength {
            expected: GERMAN_VAT_ID_LENGTH,
            actual: electronic.len(),
        });
    }
    // Normalisation guarantees ASCII before byte slicing.
    if &electronic[..2] != "DE" {
        return Err(GermanVatIdError::InvalidCountryPrefix);
    }
    if !electronic[2..].bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(GermanVatIdError::InvalidNationalIdentifier);
    }

    Ok(GermanVatIdParts {
        country_code: electronic[..2].to_string(),
        national_identifier: electronic[2..].to_string(),
        electronic,
    })
}

/// Validates the publicly specified syntax and returns explicit evidence gaps.
///
/// A successful result means only that the format is valid.  It does not mean
/// that a checksum was verified or that the identifier exists, is active, or
/// belongs to a particular legal entity.
pub fn validate_german_vat_id(input: &str) -> Result<GermanVatIdValidation, GermanVatIdError> {
    let parts = parse_german_vat_id(input)?;
    Ok(GermanVatIdValidation {
        parts,
        syntax_valid: true,
        checksum_status: VatIdChecksumStatus::NotAvailable,
        lookup_status: VatIdLookupStatus::NotPerformed,
        assignment_status: VatIdAssignmentStatus::Unknown,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_exactly_de_and_nine_digits_without_overclaiming() {
        let report = validate_german_vat_id("DE 123 456 789").unwrap();
        assert_eq!(report.parts.electronic, "DE123456789");
        assert!(report.syntax_valid);
        assert_eq!(report.checksum_status, VatIdChecksumStatus::NotAvailable);
        assert_eq!(report.lookup_status, VatIdLookupStatus::NotPerformed);
        assert_eq!(report.assignment_status, VatIdAssignmentStatus::Unknown);
    }

    #[test]
    fn normalization_is_deterministic_for_ascii_presentation_only() {
        assert_eq!(
            normalize_german_vat_id("de 123 456 789").unwrap(),
            "DE123456789"
        );
        assert_eq!(
            normalize_german_vat_id("DE123456789").unwrap(),
            "DE123456789"
        );
    }

    #[test]
    fn length_prefix_and_character_mutations_are_rejected() {
        assert!(matches!(
            parse_german_vat_id("DE12345678"),
            Err(GermanVatIdError::InvalidLength { .. })
        ));
        assert!(matches!(
            parse_german_vat_id("FR123456789"),
            Err(GermanVatIdError::InvalidCountryPrefix)
        ));
        assert!(matches!(
            parse_german_vat_id("DE12345678A"),
            Err(GermanVatIdError::InvalidNationalIdentifier)
        ));
    }

    #[test]
    fn all_generated_digit_shapes_parse_but_never_gain_assignment_evidence() {
        for number in 0..10_000_u32 {
            let candidate = format!("DE{number:09}");
            let report = validate_german_vat_id(&candidate).unwrap();
            assert_eq!(report.parts.electronic, candidate);
            assert_eq!(report.checksum_status, VatIdChecksumStatus::NotAvailable);
            assert_eq!(report.assignment_status, VatIdAssignmentStatus::Unknown);
        }
    }

    #[test]
    fn arbitrary_unicode_is_rejected_without_panicking() {
        for input in [
            "DÉ123456789",
            "DE１２３４５６７８９",
            "DE12345😀789",
            "DE123\t456789",
            "\u{200b}DE123456789",
        ] {
            assert!(parse_german_vat_id(input).is_err(), "accepted {input:?}");
        }
    }
}
