//! International IBAN validation and checksum-only fixture generation.
//!
//! Country lengths, BBAN character structures and examples come from release
//! 102 (June 2026) of the SWIFT IBAN Registry.  A valid result proves only the
//! registered shape and ISO 7064 MOD 97-10 checksum.  It does not prove that a
//! bank or account exists.

use std::{collections::HashSet, fmt, sync::OnceLock};

use serde::Deserialize;

use crate::{checksum::mod97, fixture::DeterministicRng};

use super::iban::{normalize_iban, IbanError};

pub const IBAN_REGISTRY_NAME: &str = "swift_iban_registry";
pub const IBAN_REGISTRY_RELEASE: u16 = 102;
pub const IBAN_REGISTRY_PUBLISHED: &str = "2026-06";
pub const IBAN_REGISTRY_SOURCE_URL: &str = "https://www.swift.com/swift-resource/9606/download";
/// SHA-256 of the checked-in, canonical JSON projection (not of the source PDF).
pub const IBAN_REGISTRY_DATA_SHA256: &str =
    "f908dfa2c9f055d98eedc80edda944edb9c93f8b059cab3c30a86a5a4d2afb20";

const IBAN_REGISTRY_JSON: &str = include_str!("../../../../../data/iban_registry_release_102.json");

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct IbanCountrySpec {
    pub country_code: String,
    pub country_name: String,
    pub sepa: bool,
    pub iban_length: usize,
    pub bban_length: usize,
    pub bban_structure: String,
    pub bank_identifier_position: Option<String>,
    pub bank_identifier_length: Option<String>,
    pub branch_identifier_position: Option<String>,
    pub branch_identifier_length: Option<String>,
    pub example_electronic: String,
    pub example_print: Option<String>,
    pub effective_date: Option<String>,
    pub last_update_date: Option<String>,
}

#[derive(Debug, Deserialize)]
struct IbanRegistrySnapshot {
    registry_name: String,
    registry_authority: String,
    release: u16,
    published: String,
    source_url: String,
    extracted_from_official_registry: bool,
    countries: Vec<IbanCountrySpec>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InternationalIbanError {
    InvalidInput(IbanError),
    TooShort {
        actual: usize,
    },
    InvalidCountryCode,
    UnknownCountry {
        country: String,
    },
    InvalidCheckDigits,
    InvalidLength {
        country: String,
        expected: usize,
        actual: usize,
    },
    InvalidBbanCharacter {
        position: usize,
        expected: &'static str,
        character: char,
    },
    InvalidRegistryPattern {
        pattern: String,
    },
    ChecksumMismatch,
    Checksum(mod97::Mod97Error),
    ReferenceData(String),
}

impl fmt::Display for InternationalIbanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(error) => write!(formatter, "invalid IBAN input: {error}"),
            Self::TooShort { actual } => {
                write!(
                    formatter,
                    "IBAN needs at least four characters, got {actual}"
                )
            }
            Self::InvalidCountryCode => {
                formatter.write_str("IBAN country code must be two ASCII letters")
            }
            Self::UnknownCountry { country } => {
                write!(
                    formatter,
                    "country {country:?} is not in the embedded IBAN registry"
                )
            }
            Self::InvalidCheckDigits => {
                formatter.write_str("IBAN positions 3 and 4 must be decimal check digits")
            }
            Self::InvalidLength {
                country,
                expected,
                actual,
            } => write!(
                formatter,
                "{country} IBAN must be {expected} characters, got {actual}"
            ),
            Self::InvalidBbanCharacter {
                position,
                expected,
                character,
            } => write!(
                formatter,
                "invalid BBAN character {character:?} at position {position}; expected {expected}"
            ),
            Self::InvalidRegistryPattern { pattern } => {
                write!(formatter, "unsupported embedded BBAN pattern {pattern:?}")
            }
            Self::ChecksumMismatch => formatter.write_str("IBAN MOD-97 checksum is invalid"),
            Self::Checksum(error) => write!(formatter, "IBAN checksum input error: {error}"),
            Self::ReferenceData(error) => write!(formatter, "IBAN registry is invalid: {error}"),
        }
    }
}

impl std::error::Error for InternationalIbanError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidInput(error) => Some(error),
            Self::Checksum(error) => Some(error),
            _ => None,
        }
    }
}

impl From<IbanError> for InternationalIbanError {
    fn from(value: IbanError) -> Self {
        Self::InvalidInput(value)
    }
}

impl From<mod97::Mod97Error> for InternationalIbanError {
    fn from(value: mod97::Mod97Error) -> Self {
        Self::Checksum(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InternationalIbanParts {
    pub electronic: String,
    pub formatted: String,
    pub country_code: String,
    pub check_digits: String,
    pub bban: String,
    pub country_name: String,
    pub sepa: bool,
    pub bank_identifier: Option<String>,
    pub branch_identifier: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedInternationalIban {
    pub value: String,
    pub formatted: String,
    pub parts: InternationalIbanParts,
    pub generator_version: &'static str,
}

static REGISTRY: OnceLock<Result<IbanRegistrySnapshot, String>> = OnceLock::new();

fn registry() -> Result<&'static IbanRegistrySnapshot, InternationalIbanError> {
    let parsed = REGISTRY.get_or_init(|| {
        let snapshot: IbanRegistrySnapshot =
            serde_json::from_str(IBAN_REGISTRY_JSON).map_err(|error| error.to_string())?;
        validate_registry_snapshot(&snapshot)?;
        Ok(snapshot)
    });
    parsed
        .as_ref()
        .map_err(|error| InternationalIbanError::ReferenceData(error.clone()))
}

fn validate_registry_snapshot(snapshot: &IbanRegistrySnapshot) -> Result<(), String> {
    if snapshot.registry_name != IBAN_REGISTRY_NAME
        || snapshot.release != IBAN_REGISTRY_RELEASE
        || snapshot.published != IBAN_REGISTRY_PUBLISHED
        || snapshot.source_url != IBAN_REGISTRY_SOURCE_URL
        || !snapshot.extracted_from_official_registry
    {
        return Err("registry metadata does not match compiled constants".to_string());
    }
    if snapshot.registry_authority.trim().is_empty() || snapshot.countries.is_empty() {
        return Err("registry authority or countries are empty".to_string());
    }

    let mut countries = HashSet::with_capacity(snapshot.countries.len());
    for country in &snapshot.countries {
        if country.country_code.len() != 2
            || !country
                .country_code
                .bytes()
                .all(|byte| byte.is_ascii_uppercase())
            || !countries.insert(country.country_code.as_str())
            || country.iban_length != country.bban_length + 4
            || country.example_electronic.len() != country.iban_length
            || !country
                .example_electronic
                .starts_with(&country.country_code)
        {
            return Err(format!(
                "invalid or duplicate country entry {}",
                country.country_code
            ));
        }
        let segments =
            parse_bban_pattern(&country.bban_structure).map_err(|error| error.to_string())?;
        if segments.iter().map(|segment| segment.length).sum::<usize>() != country.bban_length {
            return Err(format!(
                "BBAN structure length mismatch for {}",
                country.country_code
            ));
        }
    }
    Ok(())
}

pub fn iban_registry_countries() -> Result<&'static [IbanCountrySpec], InternationalIbanError> {
    Ok(&registry()?.countries)
}

pub fn iban_country_spec(
    country_code: &str,
) -> Result<&'static IbanCountrySpec, InternationalIbanError> {
    let code = country_code.trim().to_ascii_uppercase();
    if code.len() != 2 || !code.bytes().all(|byte| byte.is_ascii_uppercase()) {
        return Err(InternationalIbanError::InvalidCountryCode);
    }
    registry()?
        .countries
        .iter()
        .find(|country| country.country_code == code)
        .ok_or(InternationalIbanError::UnknownCountry { country: code })
}

/// Parses the registered country length and BBAN character structure without
/// claiming that the MOD-97 checksum is valid.
pub fn parse_international_iban(
    input: &str,
) -> Result<InternationalIbanParts, InternationalIbanError> {
    let electronic = normalize_iban(input)?;
    if electronic.len() < 4 {
        return Err(InternationalIbanError::TooShort {
            actual: electronic.len(),
        });
    }
    let bytes = electronic.as_bytes();
    if !bytes[0].is_ascii_uppercase() || !bytes[1].is_ascii_uppercase() {
        return Err(InternationalIbanError::InvalidCountryCode);
    }
    if !bytes[2].is_ascii_digit() || !bytes[3].is_ascii_digit() {
        return Err(InternationalIbanError::InvalidCheckDigits);
    }

    let country_code = &electronic[0..2];
    let spec = iban_country_spec(country_code)?;
    if electronic.len() != spec.iban_length {
        return Err(InternationalIbanError::InvalidLength {
            country: country_code.to_string(),
            expected: spec.iban_length,
            actual: electronic.len(),
        });
    }
    let bban = &electronic[4..];
    validate_bban(bban, &spec.bban_structure)?;

    Ok(InternationalIbanParts {
        electronic: electronic.clone(),
        formatted: group_in_fours(&electronic),
        country_code: country_code.to_string(),
        check_digits: electronic[2..4].to_string(),
        bban: bban.to_string(),
        country_name: spec.country_name.clone(),
        sepa: spec.sepa,
        bank_identifier: extract_registered_part(bban, spec.bank_identifier_position.as_deref()),
        branch_identifier: extract_registered_part(
            bban,
            spec.branch_identifier_position.as_deref(),
        ),
    })
}

pub fn validate_international_iban(
    input: &str,
) -> Result<InternationalIbanParts, InternationalIbanError> {
    let parts = parse_international_iban(input)?;
    let rearranged = format!("{}{}", &parts.electronic[4..], &parts.electronic[..4]);
    if !mod97::is_valid(&rearranged)? {
        return Err(InternationalIbanError::ChecksumMismatch);
    }
    Ok(parts)
}

pub fn format_international_iban(input: &str) -> Result<String, InternationalIbanError> {
    Ok(parse_international_iban(input)?.formatted)
}

pub fn generate_international_iban_checksum_only(
    country_code: &str,
    seed: &str,
    index: u32,
) -> Result<GeneratedInternationalIban, InternationalIbanError> {
    let spec = iban_country_spec(country_code)?;
    let namespace = format!(
        "payments.iban.international.{}.checksum-only",
        spec.country_code
    );
    let mut rng = DeterministicRng::new(seed, &namespace, index);
    let mut bban = String::with_capacity(spec.bban_length);
    for segment in parse_bban_pattern(&spec.bban_structure)? {
        for _ in 0..segment.length {
            let character = match segment.kind {
                BbanCharacterKind::Numeric => char::from(b'0' + rng.digit()),
                BbanCharacterKind::Alphabetic => char::from(b'A' + rng.index(26) as u8),
                BbanCharacterKind::Alphanumeric => rng.uppercase_alphanumeric(),
            };
            bban.push(character);
        }
    }
    let check_digits = mod97::calculate_check_digits(&format!("{bban}{}00", spec.country_code))?;
    build_generated(&format!("{}{check_digits}{bban}", spec.country_code))
}

/// Returns the example published by SWIFT for the selected country.  This is
/// not marked synthetic and no non-routability guarantee is made.
pub fn international_iban_official_example(
    country_code: &str,
) -> Result<GeneratedInternationalIban, InternationalIbanError> {
    let spec = iban_country_spec(country_code)?;
    build_generated(&spec.example_electronic)
}

fn build_generated(value: &str) -> Result<GeneratedInternationalIban, InternationalIbanError> {
    let parts = validate_international_iban(value)?;
    Ok(GeneratedInternationalIban {
        formatted: parts.formatted.clone(),
        value: parts.electronic.clone(),
        parts,
        generator_version: crate::GENERATOR_VERSION,
    })
}

#[derive(Debug, Clone, Copy)]
struct BbanSegment {
    length: usize,
    kind: BbanCharacterKind,
}

#[derive(Debug, Clone, Copy)]
enum BbanCharacterKind {
    Numeric,
    Alphabetic,
    Alphanumeric,
}

fn parse_bban_pattern(pattern: &str) -> Result<Vec<BbanSegment>, InternationalIbanError> {
    let bytes = pattern.as_bytes();
    let mut cursor = 0;
    let mut segments = Vec::new();
    while cursor < bytes.len() {
        let start = cursor;
        while cursor < bytes.len() && bytes[cursor].is_ascii_digit() {
            cursor += 1;
        }
        if start == cursor || cursor + 1 >= bytes.len() || bytes[cursor] != b'!' {
            return Err(InternationalIbanError::InvalidRegistryPattern {
                pattern: pattern.to_string(),
            });
        }
        let length = pattern[start..cursor].parse::<usize>().map_err(|_| {
            InternationalIbanError::InvalidRegistryPattern {
                pattern: pattern.to_string(),
            }
        })?;
        cursor += 1;
        let kind = match bytes[cursor] {
            b'n' => BbanCharacterKind::Numeric,
            b'a' => BbanCharacterKind::Alphabetic,
            b'c' => BbanCharacterKind::Alphanumeric,
            _ => {
                return Err(InternationalIbanError::InvalidRegistryPattern {
                    pattern: pattern.to_string(),
                })
            }
        };
        cursor += 1;
        if length == 0 {
            return Err(InternationalIbanError::InvalidRegistryPattern {
                pattern: pattern.to_string(),
            });
        }
        segments.push(BbanSegment { length, kind });
    }
    if segments.is_empty() {
        return Err(InternationalIbanError::InvalidRegistryPattern {
            pattern: pattern.to_string(),
        });
    }
    Ok(segments)
}

fn validate_bban(bban: &str, pattern: &str) -> Result<(), InternationalIbanError> {
    let mut offset = 0;
    for segment in parse_bban_pattern(pattern)? {
        for character in bban[offset..offset + segment.length].chars() {
            let valid = match segment.kind {
                BbanCharacterKind::Numeric => character.is_ascii_digit(),
                BbanCharacterKind::Alphabetic => character.is_ascii_uppercase(),
                BbanCharacterKind::Alphanumeric => character.is_ascii_alphanumeric(),
            };
            if !valid {
                let expected = match segment.kind {
                    BbanCharacterKind::Numeric => "an ASCII digit",
                    BbanCharacterKind::Alphabetic => "an uppercase ASCII letter",
                    BbanCharacterKind::Alphanumeric => "an uppercase ASCII letter or digit",
                };
                return Err(InternationalIbanError::InvalidBbanCharacter {
                    position: offset + 5,
                    expected,
                    character,
                });
            }
            offset += 1;
        }
    }
    Ok(())
}

fn extract_registered_part(bban: &str, position: Option<&str>) -> Option<String> {
    let (start, end) = position?.split_once('-')?;
    let start = start.parse::<usize>().ok()?.checked_sub(1)?;
    let end = end.parse::<usize>().ok()?;
    bban.get(start..end).map(str::to_string)
}

fn group_in_fours(value: &str) -> String {
    value
        .as_bytes()
        .chunks(4)
        .map(|chunk| std::str::from_utf8(chunk).expect("IBAN is ASCII"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_contains_all_release_102_countries_once() {
        let countries = iban_registry_countries().unwrap();
        assert_eq!(countries.len(), 89);
        assert!(countries
            .windows(2)
            .all(|pair| pair[0].country_code < pair[1].country_code));
        assert_eq!(iban_country_spec("de").unwrap().iban_length, 22);
        assert!(iban_country_spec("ZZ").is_err());
    }

    #[test]
    fn every_official_example_matches_structure_and_mod97() {
        for country in iban_registry_countries().unwrap() {
            let parts = validate_international_iban(&country.example_electronic)
                .unwrap_or_else(|error| panic!("{} example failed: {error}", country.country_code));
            assert_eq!(parts.country_code, country.country_code);
        }
    }

    #[test]
    fn checksum_only_generation_is_reproducible_for_every_country() {
        for country in iban_registry_countries().unwrap() {
            let first = generate_international_iban_checksum_only(
                &country.country_code,
                "international-fixture",
                7,
            )
            .unwrap();
            let second = generate_international_iban_checksum_only(
                &country.country_code,
                "international-fixture",
                7,
            )
            .unwrap();
            assert_eq!(first, second);
            assert_eq!(
                validate_international_iban(&first.value).unwrap(),
                first.parts
            );
        }
    }

    #[test]
    fn checksum_mutation_and_bban_shape_errors_are_distinct() {
        let mut checksum_mutation = "DE89370400440532013000".to_string();
        checksum_mutation.replace_range(2..4, "88");
        assert_eq!(
            validate_international_iban(&checksum_mutation),
            Err(InternationalIbanError::ChecksumMismatch)
        );

        let shape_mutation = "AD12A0012030200359100100";
        assert!(matches!(
            parse_international_iban(shape_mutation),
            Err(InternationalIbanError::InvalidBbanCharacter { .. })
        ));
    }

    #[test]
    fn arbitrary_unicode_never_panics_or_slips_through() {
        for input in ["", "💳", "ＤＥ89370400440532013000", "DE89\u{0000}3704"] {
            assert!(validate_international_iban(input).is_err());
        }
        assert!(validate_international_iban("de89 3704 0044 0532 0130 00").is_ok());
    }
}
