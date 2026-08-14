//! ISO 9362 Business Identifier Code (BIC) syntax and fixture patterns.
//!
//! Syntax validation is not a SWIFT-directory lookup. In particular, an eighth
//! character of `0` identifies the Test & Training address pattern, but does
//! not prove that SWIFT assigned the generated address.

use std::{collections::BTreeSet, fmt};

use crate::fixture::DeterministicRng;

use super::iban::GermanBankDirectory;

pub const BIC_PRIMARY_LENGTH: usize = 8;
pub const BIC_BRANCH_LENGTH: usize = 11;
pub const ISO_3166_ALPHA2_CHECKED_ON: &str = "2026-08-14";

const ALPHANUMERIC: &[u8; 36] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
const NON_TEST_LOCATION_CHARACTERS: &[u8; 35] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ123456789";

// Officially assigned ISO 3166-1 alpha-2 code elements, sorted and stored as
// fixed-width pairs for allocation-free lookup. User-assigned codes such as
// AA, QM..QZ, XA..XZ and ZZ are deliberately absent.
const ISO_3166_ALPHA2_CODES: &str = concat!(
    "ADAEAFAGAIALAMAOAQARASATAUAWAXAZ",
    "BABBBDBEBFBGBHBIBJBLBMBNBOBQBRBSBTBVBWBYBZ",
    "CACCCDCFCGCHCICKCLCMCNCOCRCUCVCWCXCYCZ",
    "DEDJDKDMDODZ",
    "ECEEEGEHERESET",
    "FIFJFKFMFOFR",
    "GAGBGDGEGFGGGHGIGLGMGNGPGQGRGSGTGUGWGY",
    "HKHMHNHRHTHU",
    "IDIEILIMINIOIQIRISIT",
    "JEJMJOJP",
    "KEKGKHKIKMKNKPKRKWKYKZ",
    "LALBLCLILKLRLSLTLULVLY",
    "MAMCMDMEMFMGMHMKMLMMMNMOMPMQMRMSMTMUMVMWMXMYMZ",
    "NANCNENFNGNINLNONPNRNUNZ",
    "OM",
    "PAPEPFPGPHPKPLPMPNPRPSPTPWPY",
    "QA",
    "RERORSRURW",
    "SASBSCSDSESGSHSISJSKSLSMSNSOSRSSSTSVSXSYSZ",
    "TCTDTFTGTHTJTKTLTMTNTOTRTTTVTWTZ",
    "UAUGUMUSUYUZ",
    "VAVCVEVGVIVNVU",
    "WFWS",
    "YEYT",
    "ZAZMZW",
);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BicError {
    Empty,
    InvalidCharacter { position: usize, character: char },
    InvalidLength { actual: usize },
    InvalidBusinessPartyPrefix,
    InvalidCountryCode,
    UnassignedCountryCode { country: String },
    InvalidLocationCode,
    InvalidBranchCode,
    DirectoryIsEmpty,
    DirectoryRecordHasNoBic,
}

impl fmt::Display for BicError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("BIC must not be empty"),
            Self::InvalidCharacter {
                position,
                character,
            } => write!(
                formatter,
                "invalid BIC character {character:?} at position {position}"
            ),
            Self::InvalidLength { actual } => {
                write!(formatter, "BIC must be 8 or 11 characters, got {actual}")
            }
            Self::InvalidBusinessPartyPrefix => formatter.write_str(
                "BIC business-party prefix must be 4 uppercase ASCII alphanumeric characters",
            ),
            Self::InvalidCountryCode => {
                formatter.write_str("BIC country code must be 2 uppercase ASCII letters")
            }
            Self::UnassignedCountryCode { country } => {
                write!(
                    formatter,
                    "BIC country code {country:?} is not assigned by ISO 3166-1"
                )
            }
            Self::InvalidLocationCode => formatter
                .write_str("BIC location code must be 2 uppercase ASCII alphanumeric characters"),
            Self::InvalidBranchCode => formatter
                .write_str("BIC branch code must be 3 uppercase ASCII alphanumeric characters"),
            Self::DirectoryIsEmpty => {
                formatter.write_str("BIC directory has no selectable records")
            }
            Self::DirectoryRecordHasNoBic => {
                formatter.write_str("selected directory record has no BIC")
            }
        }
    }
}

impl std::error::Error for BicError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BicParts {
    pub electronic: String,
    pub business_party_prefix: String,
    pub country_code: String,
    pub location_code: String,
    pub branch_code: Option<String>,
    pub test_training_pattern: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedBic {
    pub value: String,
    pub parts: BicParts,
    /// Bank code belonging to an actual directory-backed value. This remains
    /// absent for synthetic syntax and Test & Training patterns.
    pub directory_bank_code: Option<String>,
    pub synthetic: bool,
    pub generator_version: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DirectoryBicValue {
    value: String,
    bank_code: String,
}

/// Converts lowercase ASCII to uppercase. BICs have no presentation
/// separators, so whitespace is rejected rather than silently removed.
pub fn normalize_bic(input: &str) -> Result<String, BicError> {
    if input.is_empty() {
        return Err(BicError::Empty);
    }
    let mut normalized = String::with_capacity(input.len());
    for (position, character) in input.chars().enumerate() {
        if !character.is_ascii_alphanumeric() {
            return Err(BicError::InvalidCharacter {
                position: position + 1,
                character,
            });
        }
        normalized.push(character.to_ascii_uppercase());
    }
    Ok(normalized)
}

/// Parses and validates the 8-character primary BIC and optional 3-character
/// branch identifier, including the embedded assigned ISO 3166 alpha-2 set.
pub fn parse_bic(input: &str) -> Result<BicParts, BicError> {
    let electronic = normalize_bic(input)?;
    if !matches!(electronic.len(), BIC_PRIMARY_LENGTH | BIC_BRANCH_LENGTH) {
        return Err(BicError::InvalidLength {
            actual: electronic.len(),
        });
    }

    // Normalisation guarantees ASCII, making these byte ranges safe.
    if !electronic[0..4]
        .bytes()
        .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
    {
        return Err(BicError::InvalidBusinessPartyPrefix);
    }
    if !electronic[4..6]
        .bytes()
        .all(|byte| byte.is_ascii_uppercase())
    {
        return Err(BicError::InvalidCountryCode);
    }
    if !is_assigned_country_code(&electronic[4..6]) {
        return Err(BicError::UnassignedCountryCode {
            country: electronic[4..6].to_string(),
        });
    }
    if !electronic[6..8]
        .bytes()
        .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
    {
        return Err(BicError::InvalidLocationCode);
    }
    if electronic.len() == BIC_BRANCH_LENGTH
        && !electronic[8..11]
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
    {
        return Err(BicError::InvalidBranchCode);
    }

    Ok(BicParts {
        business_party_prefix: electronic[0..4].to_string(),
        country_code: electronic[4..6].to_string(),
        location_code: electronic[6..8].to_string(),
        branch_code: (electronic.len() == BIC_BRANCH_LENGTH).then(|| electronic[8..11].to_string()),
        test_training_pattern: electronic.as_bytes()[7] == b'0',
        electronic,
    })
}

pub fn validate_bic(input: &str) -> Result<BicParts, BicError> {
    parse_bic(input)
}

pub fn is_test_training_bic_pattern(input: &str) -> Result<bool, BicError> {
    Ok(parse_bic(input)?.test_training_pattern)
}

pub fn is_assigned_country_code(country_code: &str) -> bool {
    country_code.len() == 2
        && ISO_3166_ALPHA2_CODES
            .as_bytes()
            .chunks_exact(2)
            .any(|code| code == country_code.as_bytes())
}

/// Generates a syntactically valid BIC with the SWIFT Test & Training pattern
/// (`0` in overall position 8). It is not claimed to be SWIFT-registered.
pub fn generate_bic_test_training_pattern(
    seed: &str,
    index: u32,
    include_branch: bool,
) -> Result<GeneratedBic, BicError> {
    // Two base-36 positions retain the recognisable NRG prefix while making
    // every API-sized batch (up to 100 values) collision-free. Deriving the
    // rotation from the seed, rather than from each index independently, makes
    // the mapping injective for the first 1,296 indices.
    let mut rng = DeterministicRng::new(seed, "payments.bic.test-training-pattern", 0);
    let ordinal = (rng.index(36 * 36) + index as usize) % (36 * 36);
    let mut value = String::from("NRG");
    value.push(char::from(ALPHANUMERIC[ordinal / 36]));
    value.push_str("DE");
    value.push(char::from(ALPHANUMERIC[ordinal % 36]));
    value.push('0');
    if include_branch {
        value.push_str("XXX");
    }
    generated(value)
}

/// Generates a syntax-only German BIC fixture. Position 8 is deliberately not
/// `0`, keeping it distinct from the Test & Training pattern profile.
pub fn generate_bic_syntax_only(
    seed: &str,
    index: u32,
    include_branch: bool,
) -> Result<GeneratedBic, BicError> {
    let mut rng = DeterministicRng::new(seed, "payments.bic.syntax-only", 0);
    let mut value = String::with_capacity(if include_branch { 11 } else { 8 });
    for _ in 0..4 {
        value.push(char::from(b'A' + rng.index(26) as u8));
    }
    value.push_str("DE");
    // Position 8 deliberately excludes `0`. The two-character location space
    // therefore contains 36 * 35 values and is traversed as a seeded rotation,
    // guaranteeing distinct primary BICs for every API-sized batch.
    let location_space = 36 * 35;
    let ordinal = (rng.index(location_space) + index as usize) % location_space;
    value.push(char::from(ALPHANUMERIC[ordinal / 35]));
    value.push(char::from(NON_TEST_LOCATION_CHARACTERS[ordinal % 35]));
    if include_branch {
        let mut branch_rng = DeterministicRng::new(seed, "payments.bic.syntax-only.branch", index);
        for _ in 0..3 {
            value.push(branch_rng.uppercase_alphanumeric());
        }
    }
    generated(value)
}

/// Selects an actual BIC from a versioned bank directory.
///
/// This profile intentionally returns a directory value rather than a
/// synthetic BIC. It is useful for BLZ/BIC relationship tests, but must not be
/// represented as collision-free or as a sandbox address.
pub fn generate_bic_directory_value(
    seed: &str,
    index: u32,
    directory: &dyn GermanBankDirectory,
) -> Result<GeneratedBic, BicError> {
    let values = directory_bic_values(directory, None)?;
    select_directory_value(seed, index, &values)
}

/// Selects a directory-backed BIC and renders it as either BIC8 or BIC11.
/// `XXX` denotes the primary office when a BIC8 directory entry is expanded.
pub fn generate_bic_directory_value_with_branch(
    seed: &str,
    index: u32,
    directory: &dyn GermanBankDirectory,
    include_branch: bool,
) -> Result<GeneratedBic, BicError> {
    let values = directory_bic_values(directory, Some(include_branch))?;
    select_directory_value(seed, index, &values)
}

fn directory_bic_values(
    directory: &dyn GermanBankDirectory,
    include_branch: Option<bool>,
) -> Result<Vec<DirectoryBicValue>, BicError> {
    let count = directory.record_count();
    if count == 0 {
        return Err(BicError::DirectoryIsEmpty);
    }

    let mut seen = BTreeSet::new();
    let mut values = Vec::new();
    for record in directory.iter_records() {
        let Some(bic) = record.bic else {
            continue;
        };
        let parsed = parse_bic(bic)?;
        let value = match include_branch {
            None => parsed.electronic,
            Some(false) => parsed.electronic[..BIC_PRIMARY_LENGTH].to_string(),
            Some(true) if parsed.electronic.len() == BIC_PRIMARY_LENGTH => {
                format!("{}XXX", parsed.electronic)
            }
            Some(true) => parsed.electronic,
        };
        if seen.insert(value.clone()) {
            values.push(DirectoryBicValue {
                value,
                bank_code: record.bank_code.to_string(),
            });
        }
    }
    if values.is_empty() {
        return Err(BicError::DirectoryRecordHasNoBic);
    }
    Ok(values)
}

fn select_directory_value(
    seed: &str,
    index: u32,
    values: &[DirectoryBicValue],
) -> Result<GeneratedBic, BicError> {
    let mut rng = DeterministicRng::new(seed, "payments.bic.directory-value", 0);
    let selected = (rng.index(values.len()) + index as usize) % values.len();
    let selected = &values[selected];
    generated_with_metadata(
        selected.value.clone(),
        false,
        Some(selected.bank_code.clone()),
    )
}

fn generated(value: String) -> Result<GeneratedBic, BicError> {
    generated_with_metadata(value, true, None)
}

fn generated_with_metadata(
    value: String,
    synthetic: bool,
    directory_bank_code: Option<String>,
) -> Result<GeneratedBic, BicError> {
    let parts = parse_bic(&value)?;
    Ok(GeneratedBic {
        value,
        parts,
        directory_bank_code,
        synthetic,
        generator_version: crate::GENERATOR_VERSION,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_country_codes_are_complete_sorted_and_unique() {
        let codes: Vec<_> = ISO_3166_ALPHA2_CODES.as_bytes().chunks_exact(2).collect();
        assert_eq!(ISO_3166_ALPHA2_CODES.len(), 249 * 2);
        assert!(codes.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(is_assigned_country_code("DE"));
        assert!(!is_assigned_country_code("ZZ"));
    }

    #[test]
    fn parses_primary_and_branch_bics() {
        let primary = parse_bic("DEUTDEFF").unwrap();
        assert_eq!(primary.business_party_prefix, "DEUT");
        assert_eq!(primary.country_code, "DE");
        assert_eq!(primary.location_code, "FF");
        assert_eq!(primary.branch_code, None);
        assert!(!primary.test_training_pattern);

        let branch = parse_bic("deutdeff500").unwrap();
        assert_eq!(branch.electronic, "DEUTDEFF500");
        assert_eq!(branch.branch_code.as_deref(), Some("500"));
    }

    #[test]
    fn recognizes_but_does_not_overclaim_test_training_pattern() {
        let parts = parse_bic("NRGXDE10XXX").unwrap();
        assert!(parts.test_training_pattern);
        assert_eq!(is_test_training_bic_pattern("NRGXDE10"), Ok(true));
    }

    #[test]
    fn accepts_iso_alphanumeric_business_party_prefix() {
        assert!(parse_bic("A1C3DEFF").is_ok());
    }

    #[test]
    fn rejects_bad_lengths_country_shape_separators_and_unicode() {
        assert!(matches!(
            parse_bic("DEUTDEF"),
            Err(BicError::InvalidLength { .. })
        ));
        assert_eq!(parse_bic("DEUT1EFF"), Err(BicError::InvalidCountryCode));
        assert_eq!(
            parse_bic("NRGXZZ10"),
            Err(BicError::UnassignedCountryCode {
                country: "ZZ".to_string()
            })
        );
        for input in ["DEUT DEFF", "DEUTDE-F", "DÉUTDEFF", "😀"] {
            assert!(matches!(
                parse_bic(input),
                Err(BicError::InvalidCharacter { .. })
            ));
        }
    }

    #[test]
    fn test_training_generators_are_reproducible_and_position_eight_is_zero() {
        let mut primary_values = std::collections::HashSet::new();
        for index in 0..250 {
            for include_branch in [false, true] {
                let generated =
                    generate_bic_test_training_pattern("fixture", index, include_branch).unwrap();
                assert_eq!(generated.value.as_bytes()[7], b'0');
                assert!(generated.parts.test_training_pattern);
                assert_eq!(
                    generated,
                    generate_bic_test_training_pattern("fixture", index, include_branch).unwrap()
                );
                assert_eq!(generated.value.len(), if include_branch { 11 } else { 8 });
                if !include_branch {
                    assert!(primary_values.insert(generated.value));
                }
            }
        }
    }

    #[test]
    fn syntax_only_generator_avoids_test_training_marker() {
        let mut values = std::collections::HashSet::new();
        for index in 0..250 {
            let generated = generate_bic_syntax_only("fixture", index, true).unwrap();
            assert_ne!(generated.value.as_bytes()[7], b'0');
            assert!(!generated.parts.test_training_pattern);
            assert_eq!(parse_bic(&generated.value).unwrap(), generated.parts);
            assert!(values.insert(generated.value));
        }
    }

    #[test]
    fn directory_value_is_real_directory_data_not_a_synthetic_claim() {
        use super::super::iban::{GermanBankRecord, SliceGermanBankDirectory};

        let records = [GermanBankRecord {
            bank_code: "10000000",
            bic: Some("MARKDEF1100"),
        }];
        let directory = SliceGermanBankDirectory::new(&records);
        let generated = generate_bic_directory_value("fixture", 0, &directory).unwrap();
        assert_eq!(generated.value, "MARKDEF1100");
        assert_eq!(generated.directory_bank_code.as_deref(), Some("10000000"));
        assert!(!generated.synthetic);
        assert!(!generated.parts.test_training_pattern);

        let primary =
            generate_bic_directory_value_with_branch("fixture", 0, &directory, false).unwrap();
        assert_eq!(primary.value, "MARKDEF1");
        assert_eq!(primary.directory_bank_code.as_deref(), Some("10000000"));
        assert!(!primary.synthetic);
    }

    #[test]
    fn directory_profile_walks_unique_bics_without_replacement() {
        use super::super::iban::{GermanBankRecord, SliceGermanBankDirectory};

        let records = [
            GermanBankRecord {
                bank_code: "10000000",
                bic: Some("MARKDEF1100"),
            },
            GermanBankRecord {
                bank_code: "20000000",
                bic: Some("MARKDEF1100"),
            },
            GermanBankRecord {
                bank_code: "30000000",
                bic: Some("DEUTDEFF500"),
            },
        ];
        let directory = SliceGermanBankDirectory::new(&records);
        let first =
            generate_bic_directory_value_with_branch("fixture", 0, &directory, false).unwrap();
        let second =
            generate_bic_directory_value_with_branch("fixture", 1, &directory, false).unwrap();
        assert_ne!(first.value, second.value);
    }
}
