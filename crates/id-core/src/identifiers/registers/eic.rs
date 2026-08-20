//! ENTSO-E Energy Identification Codes (EIC).
//!
//! EICs are allocated by authorised Local Issuing Offices (LIOs). The public
//! ENTSO-E algorithm may be used to check an allocated code, but the guide
//! expressly prohibits using it to allocate codes unless the caller is an
//! authorised LIO. This module therefore intentionally has no generator.

use std::{error::Error, fmt, sync::OnceLock};

use serde::{Deserialize, Serialize};

use crate::reference_data::ENTSO_E_EIC_DIRECTORY_TSV;

pub const EIC_REFERENCE_MANUAL_VERSION: &str = "5.4 (2021-09-15)";
pub const EIC_IMPLEMENTATION_GUIDE_VERSION: &str = "1.1";

const EIC_LENGTH: usize = 16;
const EIC_BODY_LENGTH: usize = 15;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EicObjectType {
    Party,
    Area,
    MeasurementPoint,
    ResourceObject,
    TieLine,
    Location,
    Substation,
}

impl EicObjectType {
    pub const fn code(self) -> char {
        match self {
            Self::Party => 'X',
            Self::Area => 'Y',
            Self::MeasurementPoint => 'Z',
            Self::ResourceObject => 'W',
            Self::TieLine => 'T',
            Self::Location => 'V',
            Self::Substation => 'A',
        }
    }

    fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            b'X' => Some(Self::Party),
            b'Y' => Some(Self::Area),
            b'Z' => Some(Self::MeasurementPoint),
            b'W' => Some(Self::ResourceObject),
            b'T' => Some(Self::TieLine),
            b'V' => Some(Self::Location),
            b'A' => Some(Self::Substation),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EicAllocationStatus {
    Unknown,
}

/// Lifecycle status recorded in the embedded ENTSO-E bulk snapshot.
///
/// This status is qualified by the snapshot timestamp. It is deliberately not
/// converted into an unqualified current allocation assertion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EicDirectoryStatus {
    Active,
    Inactive,
}

impl EicDirectoryStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
        }
    }
}

/// Privacy-minimized fields projected from one ENTSO-E EIC bulk record.
///
/// The projection deliberately contains only the identifier and its
/// timestamped lifecycle status. No source-provided free text, names,
/// descriptions, dates, contacts, addresses or responsible-party data can be
/// represented by this type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EicDirectoryRecord {
    pub value: String,
    pub status: EicDirectoryStatus,
}

/// Lookup facade for the versioned EIC bulk snapshot embedded at compile time.
#[derive(Debug, Clone, Copy, Default)]
pub struct EicDirectory;

impl EicDirectory {
    pub fn record_count(self) -> usize {
        eic_directory_rows().len()
    }

    /// Returns a record only when the exact EIC occurs in the embedded
    /// snapshot. `None` means "not found in this snapshot", not "unallocated".
    pub fn lookup(self, value: &str) -> Option<EicDirectoryRecord> {
        if value.len() != EIC_LENGTH || !value.is_ascii() {
            return None;
        }
        let rows = eic_directory_rows();
        let index = rows
            .binary_search_by(|row| row.get(..EIC_LENGTH).unwrap_or_default().cmp(value))
            .ok()?;
        parse_directory_row(rows[index])
    }
}

pub fn lookup_eic_directory(value: &str) -> Option<EicDirectoryRecord> {
    EicDirectory.lookup(value)
}

fn eic_directory_rows() -> &'static [&'static str] {
    static ROWS: OnceLock<Vec<&'static str>> = OnceLock::new();
    ROWS.get_or_init(|| {
        ENTSO_E_EIC_DIRECTORY_TSV
            .lines()
            .skip_while(|line| line.starts_with('#'))
            .skip(1)
            .filter(|line| !line.is_empty())
            .collect()
    })
}

fn parse_directory_row(row: &str) -> Option<EicDirectoryRecord> {
    let mut fields = row.split('\t');
    let value = fields.next()?;
    let status = match fields.next()? {
        "active" => EicDirectoryStatus::Active,
        "inactive" => EicDirectoryStatus::Inactive,
        _ => return None,
    };
    if fields.next().is_some() {
        return None;
    }
    Some(EicDirectoryRecord {
        value: value.to_owned(),
        status,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EicParts {
    pub value: String,
    pub lio_code: String,
    pub object_type: EicObjectType,
    pub local_identifier: String,
    pub check_character: char,
    pub checksum_valid: bool,
    pub allocation_status: EicAllocationStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EicError {
    NonAscii,
    InvalidLength { actual: usize },
    InvalidCharacter { position: usize, found: char },
    UnknownObjectType { found: char },
    InvalidCheckCharacter { found: char },
    ForbiddenCalculatedCheckCharacter,
    ChecksumMismatch { expected: char, actual: char },
}

impl fmt::Display for EicError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonAscii => formatter.write_str("EIC must contain only ASCII characters"),
            Self::InvalidLength { actual } => {
                write!(formatter, "EIC must be {EIC_LENGTH} characters, got {actual}")
            }
            Self::InvalidCharacter { position, found } => write!(
                formatter,
                "EIC contains invalid character {found:?} at position {position}"
            ),
            Self::UnknownObjectType { found } => write!(
                formatter,
                "EIC object type at position 3 must be X, Y, Z, W, T, V or A, got {found}"
            ),
            Self::InvalidCheckCharacter { found } => write!(
                formatter,
                "EIC check character must be an uppercase letter or digit, got {found:?}"
            ),
            Self::ForbiddenCalculatedCheckCharacter => formatter.write_str(
                "EIC body calculates to '-' as check character; ENTSO-E requires changing the proposed body",
            ),
            Self::ChecksumMismatch { expected, actual } => write!(
                formatter,
                "invalid EIC check character: expected {expected}, got {actual}"
            ),
        }
    }
}

impl Error for EicError {}

/// Parses the published 2+1+12+1 EIC structure without asserting allocation.
///
/// This function checks syntax only. Use [`validate_eic`] to also verify the
/// ENTSO-E check character.
pub fn parse_eic(input: &str) -> Result<EicParts, EicError> {
    if !input.is_ascii() {
        return Err(EicError::NonAscii);
    }
    if input.len() != EIC_LENGTH {
        return Err(EicError::InvalidLength {
            actual: input.len(),
        });
    }

    let bytes = input.as_bytes();
    for (index, byte) in bytes[..EIC_BODY_LENGTH].iter().copied().enumerate() {
        if !(byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'-') {
            return Err(EicError::InvalidCharacter {
                position: index + 1,
                found: char::from(byte),
            });
        }
    }
    let object_type =
        EicObjectType::from_byte(bytes[2]).ok_or_else(|| EicError::UnknownObjectType {
            found: char::from(bytes[2]),
        })?;
    let check_character = char::from(bytes[EIC_BODY_LENGTH]);
    if !(bytes[EIC_BODY_LENGTH].is_ascii_uppercase() || bytes[EIC_BODY_LENGTH].is_ascii_digit()) {
        return Err(EicError::InvalidCheckCharacter {
            found: check_character,
        });
    }

    let expected = expected_check_character(&bytes[..EIC_BODY_LENGTH]);
    Ok(EicParts {
        value: input.to_string(),
        lio_code: input[..2].to_string(),
        object_type,
        local_identifier: input[3..EIC_BODY_LENGTH].to_string(),
        check_character,
        checksum_valid: expected.is_some_and(|value| value == check_character),
        allocation_status: EicAllocationStatus::Unknown,
    })
}

/// Verifies syntax and the ENTSO-E check character.
///
/// A successful result does not prove that the LIO code or EIC is registered,
/// active, or assigned to the party/object presented by a caller.
pub fn validate_eic(input: &str) -> Result<EicParts, EicError> {
    let parsed = parse_eic(input)?;
    let expected = expected_check_character(&input.as_bytes()[..EIC_BODY_LENGTH])
        .ok_or(EicError::ForbiddenCalculatedCheckCharacter)?;
    if expected != parsed.check_character {
        return Err(EicError::ChecksumMismatch {
            expected,
            actual: parsed.check_character,
        });
    }
    Ok(EicParts {
        checksum_valid: true,
        ..parsed
    })
}

fn expected_check_character(body: &[u8]) -> Option<char> {
    debug_assert_eq!(body.len(), EIC_BODY_LENGTH);
    let weighted_sum: u32 = body
        .iter()
        .enumerate()
        .map(|(index, byte)| character_value(*byte) * (16 - index as u32))
        .sum();
    let value = 36 - ((weighted_sum - 1) % 37);
    (value != 36).then(|| value_to_character(value))
}

fn character_value(byte: u8) -> u32 {
    match byte {
        b'0'..=b'9' => u32::from(byte - b'0'),
        b'A'..=b'Z' => u32::from(byte - b'A') + 10,
        b'-' => 36,
        _ => unreachable!("syntax is checked before checksum calculation"),
    }
}

fn value_to_character(value: u32) -> char {
    match value {
        0..=9 => char::from(b'0' + value as u8),
        10..=35 => char::from(b'A' + (value as u8 - 10)),
        _ => unreachable!("EIC check value is in 0..=35"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference_data::ENTSO_E_EIC_DIRECTORY_RECORD_COUNT;

    #[test]
    fn official_entso_e_examples_validate() {
        for (value, kind) in [
            ("11Y123456789012T", EicObjectType::Area),
            ("11XRWENET12345-2", EicObjectType::Party),
            ("10X168Y4E6H0041Z", EicObjectType::Party),
            ("10X---ENTSOE---L", EicObjectType::Party),
        ] {
            let parts = validate_eic(value).unwrap();
            assert_eq!(parts.object_type, kind);
            assert!(parts.checksum_valid);
            assert_eq!(parts.allocation_status, EicAllocationStatus::Unknown);
        }
    }

    #[test]
    fn check_character_mutation_is_rejected_but_syntax_can_still_be_parsed() {
        let changed = "11Y123456789012U";
        let parsed = parse_eic(changed).unwrap();
        assert!(!parsed.checksum_valid);
        assert!(matches!(
            validate_eic(changed),
            Err(EicError::ChecksumMismatch {
                expected: 'T',
                actual: 'U'
            })
        ));
    }

    #[test]
    fn lowercase_unicode_and_unknown_types_are_rejected_without_panics() {
        assert!(matches!(
            parse_eic("10x168Y4E6H0041Z"),
            Err(EicError::InvalidCharacter { position: 3, .. })
        ));
        assert_eq!(parse_eic("10X168Y4E6H00４1Z"), Err(EicError::NonAscii));
        assert!(matches!(
            parse_eic("10Q168Y4E6H0041Z"),
            Err(EicError::UnknownObjectType { found: 'Q' })
        ));
    }

    #[test]
    fn all_public_object_types_roundtrip_through_syntax_parser() {
        // Bodies are deliberately not generated as EIC fixtures: ENTSO-E only
        // permits authorised LIOs to use its algorithm for allocation.
        for (object_type, value) in [
            (EicObjectType::Party, "10X168Y4E6H0041Z"),
            (EicObjectType::Area, "11Y123456789012T"),
            (EicObjectType::MeasurementPoint, "10Z------------0"),
            (EicObjectType::ResourceObject, "10W------------0"),
            (EicObjectType::TieLine, "10T------------0"),
            (EicObjectType::Location, "10V------------0"),
            (EicObjectType::Substation, "10A------------0"),
        ] {
            let parsed = parse_eic(value).unwrap();
            assert_eq!(parsed.object_type, object_type);
            assert_eq!(parsed.object_type.code(), value.chars().nth(2).unwrap());
        }
    }

    #[test]
    fn embedded_directory_finds_active_and_inactive_official_records() {
        let directory = EicDirectory;
        assert_eq!(directory.record_count(), ENTSO_E_EIC_DIRECTORY_RECORD_COUNT);

        let entso_e = directory.lookup("10X1001A1001A450").unwrap();
        assert_eq!(entso_e.status, EicDirectoryStatus::Active);
        assert_eq!(entso_e.value, "10X1001A1001A450");

        let inactive = lookup_eic_directory("10T1001C--00020B").unwrap();
        assert_eq!(inactive.status, EicDirectoryStatus::Inactive);
        assert_eq!(inactive.value, "10T1001C--00020B");
    }

    #[test]
    fn directory_absence_is_snapshot_scoped() {
        // This checksum-valid manual example is not an allocation claim and is
        // absent from this particular bulk snapshot.
        assert!(validate_eic("10X---ENTSOE---L").is_ok());
        assert!(lookup_eic_directory("10X---ENTSOE---L").is_none());
        assert!(lookup_eic_directory("10x1001A1001A450").is_none());
    }
}
