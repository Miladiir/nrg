//! Shared generate/validate dispatch.
//!
//! Both the HTTP API handlers and the browser WebAssembly exports call these
//! functions, so the frontend and backend run the exact same code — including
//! option handling, not just the checksum math.

use std::fmt;

use serde::Deserialize;

use crate::identifiers::business::{lei::validate_lei, vat_id::validate_german_vat_id};
use crate::identifiers::energy::{
    generate_bdew_market_partner_id, generate_cr_id, generate_dvgw_market_partner_id,
    generate_nebe_id, generate_package_id, generate_sg_id, generate_sr_id, generate_tr_id,
    validate_cr_id, validate_market_partner_id, validate_nebe_id, validate_package_id,
    validate_sg_id, validate_sr_id, validate_tr_id,
};
use crate::identifiers::metering::{validate_din_43849, validate_obis};
use crate::identifiers::payments::{
    bic::{generate_bic_test_training_pattern, validate_bic},
    creditor_id::{generate_creditor_id_official_test_fixture, validate_german_creditor_id},
    end_to_end_id::{generate_end_to_end_id, validate_end_to_end_id},
    iban::generate_iban_synthetic_non_routable,
    international_iban::{
        generate_international_iban_checksum_only, iban_country_spec, validate_international_iban,
    },
    mandate_reference::{generate_mandate_reference, validate_mandate_reference},
    rf_reference::{build_rf_reference, generate_rf_reference, validate_rf_reference},
    uetr::{generate_uetr, validate_uetr},
};
use crate::identifiers::registers::{
    generate_synthetic_mastr, validate_eic, validate_mastr, MastrPrefix, MastrRoleSuffix,
};
use crate::reference_data::BundesbankBlzDirectory;

pub const MIN_COUNT: u32 = 1;
pub const MAX_COUNT: u32 = 100;

/// Options accepted by [`generate`]. Every field is optional; fields that an
/// identifier does not use are ignored.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct GenerateOptions {
    /// Number of values to generate (1–100, default 1).
    pub count: Option<u32>,
    /// Reproducible seed: the same seed reproduces the same values. Omitted or
    /// empty means a random seed.
    pub seed: Option<String>,
    /// `electronic` (default) or `formatted`; honored by IBAN and RF
    /// reference.
    pub format: Option<String>,
    /// ISO 3166 alpha-2 IBAN country, default `DE`.
    pub country: Option<String>,
    /// `electricity` (default) or `gas`; honored by MP-ID and MaStR.
    pub sector: Option<String>,
    /// Three-letter MaStR object prefix.
    pub prefix: Option<String>,
    /// Two-letter MaStR role suffix.
    pub role_suffix: Option<String>,
    /// Generate an 11-character BIC with branch identifier instead of BIC8.
    pub include_branch: Option<bool>,
    /// Explicit RF reference body; requires `count` 1.
    pub invoice_reference: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpsError {
    /// The options are invalid; the caller should fix the request.
    InvalidOptions(String),
    /// A generator violated an internal invariant.
    Failed(String),
}

impl fmt::Display for OpsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidOptions(message) | Self::Failed(message) => formatter.write_str(message),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sector {
    Electricity,
    Gas,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Electronic,
    Formatted,
}

fn parse_sector(options: &GenerateOptions) -> Result<Sector, OpsError> {
    match options.sector.as_deref() {
        None | Some("electricity") => Ok(Sector::Electricity),
        Some("gas") => Ok(Sector::Gas),
        Some(other) => Err(OpsError::InvalidOptions(format!(
            "Unknown sector {other:?}; expected 'electricity' or 'gas'"
        ))),
    }
}

fn parse_format(options: &GenerateOptions) -> Result<Format, OpsError> {
    match options.format.as_deref() {
        None | Some("electronic") => Ok(Format::Electronic),
        Some("formatted") => Ok(Format::Formatted),
        Some(other) => Err(OpsError::InvalidOptions(format!(
            "Unknown format {other:?}; expected 'electronic' or 'formatted'"
        ))),
    }
}

fn rendered(electronic: String, formatted: String, format: Format) -> String {
    match format {
        Format::Electronic => electronic,
        Format::Formatted => formatted,
    }
}

fn checked_count(options: &GenerateOptions) -> Result<u32, OpsError> {
    let count = options.count.unwrap_or(MIN_COUNT);
    if (MIN_COUNT..=MAX_COUNT).contains(&count) {
        Ok(count)
    } else {
        Err(OpsError::InvalidOptions(format!(
            "count must be between {MIN_COUNT} and {MAX_COUNT}, got {count}"
        )))
    }
}

fn effective_seed(options: &GenerateOptions) -> String {
    options
        .seed
        .as_deref()
        .filter(|seed| !seed.is_empty())
        .map(str::to_string)
        .unwrap_or_else(crate::generate_melo)
}

fn batch<F>(options: &GenerateOptions, mut value: F) -> Result<Vec<String>, OpsError>
where
    F: FnMut(&str, u32) -> Result<String, OpsError>,
{
    let count = checked_count(options)?;
    let seed = effective_seed(options);
    let mut values = Vec::with_capacity(count as usize);
    for index in 0..count {
        values.push(value(&seed, index)?);
    }
    Ok(values)
}

fn failed(error: impl fmt::Display) -> OpsError {
    OpsError::Failed(error.to_string())
}

fn mastr_prefix(options: &GenerateOptions) -> Result<MastrPrefix, OpsError> {
    match options.prefix.as_deref() {
        None => Ok(match parse_sector(options)? {
            Sector::Gas => MastrPrefix::GasGenerationUnit,
            Sector::Electricity => MastrPrefix::ElectricityGenerationUnit,
        }),
        Some(value) => {
            let value = value.trim().to_ascii_uppercase();
            MastrPrefix::from_code(&value)
                .ok_or_else(|| OpsError::InvalidOptions(format!("Unknown MaStR prefix {value:?}")))
        }
    }
}

fn mastr_role_suffix(options: &GenerateOptions) -> Result<Option<MastrRoleSuffix>, OpsError> {
    options
        .role_suffix
        .as_deref()
        .map(|value| {
            let value = value.trim().to_ascii_uppercase();
            MastrRoleSuffix::from_code(&value).ok_or_else(|| {
                OpsError::InvalidOptions(format!("Unknown MaStR role suffix {value:?}"))
            })
        })
        .transpose()
}

/// Generates a batch of test values for the identifier named by `slug`.
pub fn generate(slug: &str, options: &GenerateOptions) -> Result<Vec<String>, OpsError> {
    match slug {
        "malo" => batch(options, |seed, index| {
            Ok(crate::generate_malo_seeded(seed, index))
        }),
        "melo" => batch(options, |seed, index| {
            Ok(crate::generate_melo_seeded(seed, index))
        }),
        "nelo" => batch(options, |seed, index| {
            Ok(crate::generate_nelo_seeded(seed, index))
        }),
        "nebe" => batch(options, |seed, index| {
            Ok(generate_nebe_id(seed, index).value)
        }),
        "mp-id" => {
            let sector = parse_sector(options)?;
            batch(options, |seed, index| {
                Ok(match sector {
                    Sector::Electricity => generate_bdew_market_partner_id(seed, index).value,
                    Sector::Gas => generate_dvgw_market_partner_id(seed, index).value,
                })
            })
        }
        "cr-id" => batch(options, |seed, index| Ok(generate_cr_id(seed, index).value)),
        "sg-id" => batch(options, |seed, index| Ok(generate_sg_id(seed, index).value)),
        "sr-id" => batch(options, |seed, index| Ok(generate_sr_id(seed, index).value)),
        "tr-id" => batch(options, |seed, index| Ok(generate_tr_id(seed, index).value)),
        "package-id" => batch(options, |seed, index| {
            Ok(generate_package_id(seed, index).value)
        }),
        "mastr" => {
            let prefix = mastr_prefix(options)?;
            let role_suffix = mastr_role_suffix(options)?;
            if let Some(role_suffix) = role_suffix {
                if !prefix.allowed_role_suffixes().contains(&role_suffix) {
                    return Err(OpsError::InvalidOptions(format!(
                        "MaStR role suffix {} is not allowed for prefix {}",
                        role_suffix.code(),
                        prefix.code()
                    )));
                }
            }
            batch(options, |seed, index| {
                generate_synthetic_mastr(prefix, role_suffix, seed, index)
                    .map(|fixture| fixture.identifier.value)
                    .map_err(failed)
            })
        }
        "iban" => {
            let format = parse_format(options)?;
            let country = options
                .country
                .as_deref()
                .unwrap_or("DE")
                .trim()
                .to_ascii_uppercase();
            iban_country_spec(&country)
                .map_err(|error| OpsError::InvalidOptions(error.to_string()))?;
            batch(options, |seed, index| {
                if country == "DE" {
                    generate_iban_synthetic_non_routable(seed, index, &BundesbankBlzDirectory)
                        .map(|generated| rendered(generated.value, generated.formatted, format))
                        .map_err(failed)
                } else {
                    generate_international_iban_checksum_only(&country, seed, index)
                        .map(|generated| rendered(generated.value, generated.formatted, format))
                        .map_err(failed)
                }
            })
        }
        "bic" => {
            let include_branch = options.include_branch.unwrap_or(false);
            batch(options, |seed, index| {
                generate_bic_test_training_pattern(seed, index, include_branch)
                    .map(|generated| generated.value)
                    .map_err(failed)
            })
        }
        "creditor-id" => batch(options, |seed, index| {
            generate_creditor_id_official_test_fixture(seed, index)
                .map(|generated| generated.value)
                .map_err(failed)
        }),
        "mandate-reference" => batch(options, |seed, index| {
            Ok(generate_mandate_reference(seed, index).value)
        }),
        "end-to-end-id" => batch(options, |seed, index| {
            Ok(generate_end_to_end_id(seed, index).value)
        }),
        "rf-reference" => {
            let format = parse_format(options)?;
            if let Some(reference_body) = options
                .invoice_reference
                .as_deref()
                .filter(|body| !body.is_empty())
            {
                if checked_count(options)? != 1 {
                    return Err(OpsError::InvalidOptions(
                        "invoice_reference requires count = 1".to_string(),
                    ));
                }
                let generated = build_rf_reference(reference_body)
                    .map_err(|error| OpsError::InvalidOptions(error.to_string()))?;
                return Ok(vec![rendered(generated.value, generated.formatted, format)]);
            }
            batch(options, |seed, index| {
                generate_rf_reference(seed, index)
                    .map(|generated| rendered(generated.value, generated.formatted, format))
                    .map_err(failed)
            })
        }
        "uetr" => batch(options, |seed, index| Ok(generate_uetr(seed, index).value)),
        _ => Err(OpsError::InvalidOptions(format!(
            "Unknown identifier {slug:?}"
        ))),
    }
}

/// Validates one value for the identifier named by `slug`. `Ok(())` means the
/// value is formally valid; `Err` carries the reason it is not.
pub fn validate(slug: &str, value: &str) -> Result<(), String> {
    fn outcome<T, E: fmt::Display>(result: Result<T, E>) -> Result<(), String> {
        result.map(|_| ()).map_err(|error| error.to_string())
    }

    match slug {
        "malo" => outcome(crate::validate_malo(value)),
        "melo" => outcome(crate::validate_melo(value)),
        "nelo" => outcome(crate::validate_nelo(value)),
        "nebe" => outcome(validate_nebe_id(value)),
        "mp-id" => outcome(validate_market_partner_id(value)),
        "cr-id" => outcome(validate_cr_id(value)),
        "sg-id" => outcome(validate_sg_id(value)),
        "sr-id" => outcome(validate_sr_id(value)),
        "tr-id" => outcome(validate_tr_id(value)),
        "package-id" => outcome(validate_package_id(value)),
        "mastr" => outcome(validate_mastr(value)),
        "eic" => outcome(validate_eic(value)),
        "iban" => outcome(validate_international_iban(value)),
        "bic" => outcome(validate_bic(value)),
        "creditor-id" => outcome(validate_german_creditor_id(value)),
        "mandate-reference" => outcome(validate_mandate_reference(value)),
        "end-to-end-id" => outcome(validate_end_to_end_id(value)),
        "rf-reference" => outcome(validate_rf_reference(value)),
        "uetr" => outcome(validate_uetr(value)),
        "vat-id" => outcome(validate_german_vat_id(value)),
        "lei" => outcome(validate_lei(value)),
        "obis" => outcome(validate_obis(value)),
        "din-43849" => outcome(validate_din_43849(value)),
        _ => Err(format!("Unknown identifier {slug:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GENERATING_SLUGS: &[&str] = &[
        "malo",
        "melo",
        "nelo",
        "nebe",
        "mp-id",
        "cr-id",
        "sg-id",
        "sr-id",
        "tr-id",
        "package-id",
        "mastr",
        "iban",
        "bic",
        "creditor-id",
        "mandate-reference",
        "end-to-end-id",
        "rf-reference",
        "uetr",
    ];

    fn seeded(seed: &str) -> GenerateOptions {
        GenerateOptions {
            count: Some(3),
            seed: Some(seed.to_string()),
            ..GenerateOptions::default()
        }
    }

    #[test]
    fn every_generated_value_passes_its_own_validator() {
        for slug in GENERATING_SLUGS {
            let values = generate(slug, &seeded("ops-roundtrip")).unwrap();
            assert_eq!(values.len(), 3, "slug: {slug}");
            for value in &values {
                assert!(
                    validate(slug, value).is_ok(),
                    "{slug} generated invalid value {value}"
                );
            }
        }
    }

    #[test]
    fn generation_is_deterministic_and_random_without_a_seed() {
        let first = generate("malo", &seeded("x")).unwrap();
        let second = generate("malo", &seeded("x")).unwrap();
        assert_eq!(first, second);

        // Without a seed, and with an empty one, each batch is independent.
        let random = generate("malo", &GenerateOptions::default()).unwrap();
        assert_eq!(random.len(), 1);
        let empty_seed = generate(
            "malo",
            &GenerateOptions {
                seed: Some(String::new()),
                ..GenerateOptions::default()
            },
        )
        .unwrap();
        assert_ne!(random, empty_seed);
    }

    #[test]
    fn count_bounds_unknown_slugs_and_unknown_options_are_invalid() {
        for count in [0, 101] {
            let error = generate(
                "malo",
                &GenerateOptions {
                    count: Some(count),
                    ..GenerateOptions::default()
                },
            )
            .unwrap_err();
            assert!(matches!(error, OpsError::InvalidOptions(_)));
        }
        assert!(matches!(
            generate("unknown", &GenerateOptions::default()),
            Err(OpsError::InvalidOptions(_))
        ));
        assert!(validate("unknown", "x").is_err());
        for (field, value) in [("sector", "water"), ("format", "fancy"), ("country", "ZZ")] {
            let mut options = GenerateOptions::default();
            match field {
                "sector" => options.sector = Some(value.to_string()),
                "format" => options.format = Some(value.to_string()),
                _ => options.country = Some(value.to_string()),
            }
            let slug = if field == "sector" { "mp-id" } else { "iban" };
            assert!(
                matches!(generate(slug, &options), Err(OpsError::InvalidOptions(_))),
                "{field}={value} must be rejected"
            );
        }
    }

    #[test]
    fn formatted_output_groups_iban_and_rf_values() {
        let electronic = generate("iban", &seeded("format")).unwrap();
        assert!(!electronic[0].contains(' '));

        let mut options = seeded("format");
        options.format = Some("formatted".to_string());
        let formatted = generate("iban", &options).unwrap();
        assert!(formatted[0].contains(' '));
        assert_eq!(
            formatted[0].replace(' ', ""),
            electronic[0],
            "formatted output must render the same value"
        );

        let rf = generate("rf-reference", &options).unwrap();
        assert!(rf[0].contains(' '));
    }

    #[test]
    fn rf_invoice_reference_requires_a_single_value() {
        let single = generate(
            "rf-reference",
            &GenerateOptions {
                invoice_reference: Some("NRG202600001234".to_string()),
                ..GenerateOptions::default()
            },
        )
        .unwrap();
        assert!(single[0].starts_with("RF"));
        assert!(single[0].ends_with("NRG202600001234"));

        let error = generate(
            "rf-reference",
            &GenerateOptions {
                count: Some(2),
                invoice_reference: Some("NRG202600001234".to_string()),
                ..GenerateOptions::default()
            },
        )
        .unwrap_err();
        assert!(matches!(error, OpsError::InvalidOptions(_)));
    }

    #[test]
    fn mastr_options_are_checked_before_generation() {
        let unknown_prefix = GenerateOptions {
            prefix: Some("XXX".to_string()),
            ..GenerateOptions::default()
        };
        assert!(matches!(
            generate("mastr", &unknown_prefix),
            Err(OpsError::InvalidOptions(_))
        ));

        let disallowed_suffix = GenerateOptions {
            prefix: Some("SEE".to_string()),
            role_suffix: Some("AN".to_string()),
            ..GenerateOptions::default()
        };
        assert!(matches!(
            generate("mastr", &disallowed_suffix),
            Err(OpsError::InvalidOptions(_))
        ));
    }
}
