//! German International Bank Account Numbers (IBANs).
//!
//! This module validates the German 22-character shape and ISO 7064 MOD 97-10
//! checksum. It does not prove that an account exists. Directory-backed
//! generation is parameterised over [`GermanBankDirectory`], so the core never
//! embeds or invents a supposedly real bank code.

use std::fmt;

use crate::checksum::mod97;
use crate::fixture::DeterministicRng;

pub const GERMAN_IBAN_LENGTH: usize = 22;
pub const GERMAN_BANK_CODE_LENGTH: usize = 8;
pub const GERMAN_ACCOUNT_NUMBER_LENGTH: usize = 10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IbanError {
    Empty,
    InvalidCharacter { position: usize, character: char },
    InvalidLength { expected: usize, actual: usize },
    UnsupportedCountry { country: String },
    InvalidCheckDigits,
    InvalidBankCode,
    InvalidAccountNumber,
    ChecksumMismatch,
    DirectoryIsEmpty,
    InvalidDirectoryRecord { bank_code: String },
    NoUnassignedBankCodeFound,
    Checksum(mod97::Mod97Error),
}

impl fmt::Display for IbanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("IBAN must not be empty"),
            Self::InvalidCharacter {
                position,
                character,
            } => write!(
                formatter,
                "invalid IBAN character {character:?} at position {position}"
            ),
            Self::InvalidLength { expected, actual } => {
                write!(
                    formatter,
                    "German IBAN must be {expected} characters, got {actual}"
                )
            }
            Self::UnsupportedCountry { country } => {
                write!(
                    formatter,
                    "only German IBANs are supported, got {country:?}"
                )
            }
            Self::InvalidCheckDigits => {
                formatter.write_str("IBAN positions 3 and 4 must be decimal check digits")
            }
            Self::InvalidBankCode => {
                formatter.write_str("German bank code must contain exactly 8 ASCII digits")
            }
            Self::InvalidAccountNumber => {
                formatter.write_str("German account number must contain exactly 10 ASCII digits")
            }
            Self::ChecksumMismatch => formatter.write_str("IBAN MOD-97 checksum is invalid"),
            Self::DirectoryIsEmpty => {
                formatter.write_str("bank directory contains no selectable active records")
            }
            Self::InvalidDirectoryRecord { bank_code } => write!(
                formatter,
                "bank directory returned an invalid German bank code {bank_code:?}"
            ),
            Self::NoUnassignedBankCodeFound => formatter
                .write_str("could not derive a bank code absent from the supplied bank directory"),
            Self::Checksum(error) => write!(formatter, "IBAN checksum input error: {error}"),
        }
    }
}

impl std::error::Error for IbanError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Checksum(error) => Some(error),
            _ => None,
        }
    }
}

impl From<mod97::Mod97Error> for IbanError {
    fn from(value: mod97::Mod97Error) -> Self {
        Self::Checksum(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GermanIbanParts {
    pub electronic: String,
    pub country_code: String,
    pub check_digits: String,
    pub bank_code: String,
    pub account_number: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedGermanIban {
    pub value: String,
    pub formatted: String,
    pub parts: GermanIbanParts,
    /// Present only when a directory record supplied a BIC. This is reference
    /// data, not evidence that the randomly derived account exists.
    pub directory_bic: Option<String>,
    pub generator_version: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IbanDirectoryStatus {
    Found,
    NotFound,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryValidatedGermanIban {
    pub parts: GermanIbanParts,
    pub directory_status: IbanDirectoryStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GermanBankRecord<'a> {
    pub bank_code: &'a str,
    pub bic: Option<&'a str>,
}

/// Read-only access to a versioned German bank-code directory.
///
/// Implementations should expose active, selectable records through `record`.
/// `contains_bank_code` may additionally include non-selectable/deleted records
/// when a profile needs a conservative "not present anywhere" assertion.
pub trait GermanBankDirectory {
    fn record_count(&self) -> usize;
    fn record(&self, index: usize) -> Option<GermanBankRecord<'_>>;

    fn iter_records(&self) -> Box<dyn Iterator<Item = GermanBankRecord<'_>> + '_> {
        Box::new((0..self.record_count()).filter_map(|index| self.record(index)))
    }

    fn contains_bank_code(&self, bank_code: &str) -> bool {
        self.iter_records()
            .any(|record| record.bank_code == bank_code)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SliceGermanBankDirectory<'a> {
    records: &'a [GermanBankRecord<'a>],
}

impl<'a> SliceGermanBankDirectory<'a> {
    pub const fn new(records: &'a [GermanBankRecord<'a>]) -> Self {
        Self { records }
    }
}

impl GermanBankDirectory for SliceGermanBankDirectory<'_> {
    fn record_count(&self) -> usize {
        self.records.len()
    }

    fn record(&self, index: usize) -> Option<GermanBankRecord<'_>> {
        self.records.get(index).copied()
    }

    fn iter_records(&self) -> Box<dyn Iterator<Item = GermanBankRecord<'_>> + '_> {
        Box::new(self.records.iter().copied())
    }
}

impl GermanBankDirectory for crate::reference_data::BundesbankBlzDirectory {
    fn record_count(&self) -> usize {
        (*self).directory_record_count()
    }

    fn record(&self, index: usize) -> Option<GermanBankRecord<'_>> {
        (*self)
            .directory_record(index)
            .map(|record| GermanBankRecord {
                bank_code: record.bank_code,
                bic: record.bic,
            })
    }

    fn iter_records(&self) -> Box<dyn Iterator<Item = GermanBankRecord<'_>> + '_> {
        Box::new(
            (*self)
                .records()
                .filter(crate::reference_data::BundesbankBankRecord::is_directory_plausible)
                .map(|record| GermanBankRecord {
                    bank_code: record.bank_code,
                    bic: record.bic,
                }),
        )
    }

    fn contains_bank_code(&self, bank_code: &str) -> bool {
        (*self).contains_bank_code(bank_code)
    }
}

/// Removes ASCII spaces and converts lowercase ASCII letters to uppercase.
/// All other separators and every non-ASCII character are rejected.
pub fn normalize_iban(input: &str) -> Result<String, IbanError> {
    if input.is_empty() {
        return Err(IbanError::Empty);
    }

    let mut normalized = String::with_capacity(input.len());
    for (position, character) in input.chars().enumerate() {
        if character == ' ' {
            continue;
        }
        if character.is_ascii_alphanumeric() {
            normalized.push(character.to_ascii_uppercase());
        } else {
            return Err(IbanError::InvalidCharacter {
                position: position + 1,
                character,
            });
        }
    }

    if normalized.is_empty() {
        return Err(IbanError::Empty);
    }
    Ok(normalized)
}

/// Parses the German IBAN shape without claiming checksum validity.
pub fn parse_german_iban(input: &str) -> Result<GermanIbanParts, IbanError> {
    let electronic = normalize_iban(input)?;
    if electronic.len() != GERMAN_IBAN_LENGTH {
        return Err(IbanError::InvalidLength {
            expected: GERMAN_IBAN_LENGTH,
            actual: electronic.len(),
        });
    }

    // `normalize_iban` guarantees ASCII, therefore these byte ranges are UTF-8
    // boundaries and cannot panic.
    if &electronic[0..2] != "DE" {
        return Err(IbanError::UnsupportedCountry {
            country: electronic[0..2].to_string(),
        });
    }
    if !electronic[2..4].bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(IbanError::InvalidCheckDigits);
    }
    if !electronic[4..12].bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(IbanError::InvalidBankCode);
    }
    if !electronic[12..22].bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(IbanError::InvalidAccountNumber);
    }

    Ok(GermanIbanParts {
        country_code: electronic[0..2].to_string(),
        check_digits: electronic[2..4].to_string(),
        bank_code: electronic[4..12].to_string(),
        account_number: electronic[12..22].to_string(),
        electronic,
    })
}

/// Parses and validates a German IBAN's MOD-97 checksum.
pub fn validate_german_iban(input: &str) -> Result<GermanIbanParts, IbanError> {
    let parts = parse_german_iban(input)?;
    let rearranged = format!("{}{}", &parts.electronic[4..], &parts.electronic[..4]);
    if !mod97::is_valid(&rearranged)? {
        return Err(IbanError::ChecksumMismatch);
    }
    Ok(parts)
}

/// Alias for callers whose identifier kind already establishes Germany.
pub fn validate_iban(input: &str) -> Result<GermanIbanParts, IbanError> {
    validate_german_iban(input)
}

pub fn validate_german_iban_with_directory(
    input: &str,
    directory: &dyn GermanBankDirectory,
) -> Result<DirectoryValidatedGermanIban, IbanError> {
    let parts = validate_german_iban(input)?;
    let directory_status = if directory.contains_bank_code(&parts.bank_code) {
        IbanDirectoryStatus::Found
    } else {
        IbanDirectoryStatus::NotFound
    };
    Ok(DirectoryValidatedGermanIban {
        parts,
        directory_status,
    })
}

pub fn format_german_iban(input: &str) -> Result<String, IbanError> {
    let parts = parse_german_iban(input)?;
    Ok(group_in_fours(&parts.electronic))
}

pub fn build_german_iban(
    bank_code: &str,
    account_number: &str,
) -> Result<GeneratedGermanIban, IbanError> {
    build_german_iban_with_bic(bank_code, account_number, None)
}

fn build_german_iban_with_bic(
    bank_code: &str,
    account_number: &str,
    directory_bic: Option<&str>,
) -> Result<GeneratedGermanIban, IbanError> {
    if bank_code.len() != GERMAN_BANK_CODE_LENGTH
        || !bank_code.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(IbanError::InvalidBankCode);
    }
    if account_number.len() != GERMAN_ACCOUNT_NUMBER_LENGTH
        || !account_number.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(IbanError::InvalidAccountNumber);
    }

    let bban = format!("{bank_code}{account_number}");
    let check_digits = mod97::calculate_check_digits(&format!("{bban}DE00"))?;
    let value = format!("DE{check_digits}{bban}");
    let parts = validate_german_iban(&value)?;
    Ok(GeneratedGermanIban {
        formatted: group_in_fours(&value),
        value,
        parts,
        directory_bic: directory_bic.map(str::to_string),
        generator_version: crate::GENERATOR_VERSION,
    })
}

/// Generates a checksum-valid German IBAN without making a directory claim.
pub fn generate_iban_checksum_only(
    seed: &str,
    index: u32,
) -> Result<GeneratedGermanIban, IbanError> {
    let mut rng = DeterministicRng::new(seed, "payments.iban.checksum-only", index);
    let bank_code = random_digits(&mut rng, GERMAN_BANK_CODE_LENGTH);
    let account_number = nonzero_random_digits(&mut rng, GERMAN_ACCOUNT_NUMBER_LENGTH);
    build_german_iban(&bank_code, &account_number)
}

/// Generates a checksum-valid German IBAN whose bank code is absent from the
/// supplied reference snapshot.
///
/// The guarantee is relative to that exact directory implementation and its
/// version. It must be re-evaluated whenever the underlying snapshot changes.
pub fn generate_iban_synthetic_non_routable(
    seed: &str,
    index: u32,
    directory: &dyn GermanBankDirectory,
) -> Result<GeneratedGermanIban, IbanError> {
    const MAX_ATTEMPTS: usize = 10_000;

    let mut rng = DeterministicRng::new(seed, "payments.iban.synthetic-non-routable", index);
    for _ in 0..MAX_ATTEMPTS {
        let bank_code = random_digits(&mut rng, GERMAN_BANK_CODE_LENGTH);
        if directory.contains_bank_code(&bank_code) {
            continue;
        }
        let account_number = nonzero_random_digits(&mut rng, GERMAN_ACCOUNT_NUMBER_LENGTH);
        return build_german_iban(&bank_code, &account_number);
    }
    Err(IbanError::NoUnassignedBankCodeFound)
}

/// Generates a checksum-valid German IBAN using an actual record supplied by
/// the directory. The account number is still synthetic and its existence is
/// unknown; directory plausibility is not an account-existence assertion.
pub fn generate_iban_directory_plausible(
    seed: &str,
    index: u32,
    directory: &dyn GermanBankDirectory,
) -> Result<GeneratedGermanIban, IbanError> {
    let count = directory.record_count();
    if count == 0 {
        return Err(IbanError::DirectoryIsEmpty);
    }

    let mut rng = DeterministicRng::new(seed, "payments.iban.directory-plausible", index);
    let selected = rng.index(count);
    let record = directory
        .record(selected)
        .ok_or(IbanError::DirectoryIsEmpty)?;
    if record.bank_code.len() != GERMAN_BANK_CODE_LENGTH
        || !record.bank_code.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(IbanError::InvalidDirectoryRecord {
            bank_code: record.bank_code.to_string(),
        });
    }

    let account_number = nonzero_random_digits(&mut rng, GERMAN_ACCOUNT_NUMBER_LENGTH);
    build_german_iban_with_bic(record.bank_code, &account_number, record.bic)
}

fn random_digits(rng: &mut DeterministicRng, length: usize) -> String {
    (0..length)
        .map(|_| char::from(b'0' + rng.digit()))
        .collect()
}

fn nonzero_random_digits(rng: &mut DeterministicRng, length: usize) -> String {
    let mut value = random_digits(rng, length);
    if value.bytes().all(|byte| byte == b'0') {
        value.replace_range(length - 1..length, "1");
    }
    value
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

    const DIRECTORY_RECORDS: [GermanBankRecord<'static>; 1] = [GermanBankRecord {
        bank_code: "37040044",
        bic: Some("COBADEFFXXX"),
    }];

    #[test]
    fn validates_and_parses_official_registry_example() {
        let parts = validate_german_iban("DE89 3704 0044 0532 0130 00").unwrap();
        assert_eq!(parts.electronic, "DE89370400440532013000");
        assert_eq!(parts.bank_code, "37040044");
        assert_eq!(parts.account_number, "0532013000");
        assert_eq!(
            format_german_iban(&parts.electronic).unwrap(),
            "DE89 3704 0044 0532 0130 00"
        );
    }

    #[test]
    fn validates_proposed_unassigned_bank_code_vector_mathematically() {
        let parts = validate_german_iban("DE79000000001234567890").unwrap();
        assert_eq!(parts.bank_code, "00000000");
    }

    #[test]
    fn checksum_mutation_is_rejected() {
        assert_eq!(
            validate_german_iban("DE88 3704 0044 0532 0130 00"),
            Err(IbanError::ChecksumMismatch)
        );
    }

    #[test]
    fn normalizes_lowercase_but_rejects_non_ascii_and_bad_segments() {
        assert_eq!(
            normalize_iban("de89 3704 0044 0532 0130 00").unwrap(),
            "DE89370400440532013000"
        );
        for input in [
            "DÉ89370400440532013000",
            "DE89-3704-0044-0532-0130-00",
            "😀",
            "DE89\t370400440532013000",
        ] {
            assert!(normalize_iban(input).is_err(), "accepted {input:?}");
        }
        assert!(matches!(
            parse_german_iban("FR89370400440532013000"),
            Err(IbanError::UnsupportedCountry { .. })
        ));
        assert_eq!(
            parse_german_iban("DEAA370400440532013000"),
            Err(IbanError::InvalidCheckDigits)
        );
    }

    #[test]
    fn checksum_only_generation_is_reproducible_and_self_validating() {
        for index in 0..250 {
            let generated = generate_iban_checksum_only("fixture-4711", index).unwrap();
            assert_eq!(
                generated,
                generate_iban_checksum_only("fixture-4711", index).unwrap()
            );
            assert_eq!(
                validate_german_iban(&generated.value).unwrap(),
                generated.parts
            );
            assert_eq!(generated.formatted.len(), 27);
        }
        assert_ne!(
            generate_iban_checksum_only("fixture-4711", 0)
                .unwrap()
                .value,
            generate_iban_checksum_only("fixture-4711", 1)
                .unwrap()
                .value
        );
    }

    #[test]
    fn directory_plausible_uses_only_supplied_records() {
        let directory = SliceGermanBankDirectory::new(&DIRECTORY_RECORDS);
        let generated =
            generate_iban_directory_plausible("directory-fixture", 3, &directory).unwrap();
        assert_eq!(generated.parts.bank_code, "37040044");
        assert_eq!(generated.directory_bic.as_deref(), Some("COBADEFFXXX"));
        assert_eq!(
            validate_german_iban_with_directory(&generated.value, &directory)
                .unwrap()
                .directory_status,
            IbanDirectoryStatus::Found
        );
    }

    struct AllButLeadingZeroDirectory;

    impl GermanBankDirectory for AllButLeadingZeroDirectory {
        fn record_count(&self) -> usize {
            0
        }

        fn record(&self, _index: usize) -> Option<GermanBankRecord<'_>> {
            None
        }

        fn contains_bank_code(&self, bank_code: &str) -> bool {
            !bank_code.starts_with('0')
        }
    }

    #[test]
    fn synthetic_non_routable_is_absent_from_supplied_directory() {
        let directory = AllButLeadingZeroDirectory;
        for index in 0..100 {
            let generated =
                generate_iban_synthetic_non_routable("non-routable", index, &directory).unwrap();
            assert!(!directory.contains_bank_code(&generated.parts.bank_code));
            assert_eq!(
                validate_german_iban(&generated.value).unwrap(),
                generated.parts
            );
        }
    }

    #[test]
    fn invalid_directory_data_is_not_silently_used() {
        let records = [GermanBankRecord {
            bank_code: "NOT-BLZ",
            bic: None,
        }];
        let directory = SliceGermanBankDirectory::new(&records);
        assert!(matches!(
            generate_iban_directory_plausible("seed", 0, &directory),
            Err(IbanError::InvalidDirectoryRecord { .. })
        ));
    }
}
