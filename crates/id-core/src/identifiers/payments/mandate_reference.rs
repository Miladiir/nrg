//! SEPA Unique Mandate References (`Max35Text`).

use super::reference::{deterministic_token, validate_sepa_reference, SepaReferenceError};

pub const SYNTHETIC_MANDATE_PREFIX: &str = "NRG-MND-";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedMandateReference {
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedMandateReference {
    pub value: String,
    pub synthetic: bool,
    pub generator_version: &'static str,
}

pub fn validate_mandate_reference(
    input: &str,
) -> Result<ValidatedMandateReference, SepaReferenceError> {
    validate_sepa_reference(input)?;
    Ok(ValidatedMandateReference {
        value: input.to_string(),
    })
}

/// Generates a 34-character value in the conservative `A-Z`, `0-9`, `-`
/// subset. Uniqueness is deterministic within a `(seed, index)` fixture set;
/// there is no central allocation service for mandate references.
pub fn generate_mandate_reference(seed: &str, index: u32) -> GeneratedMandateReference {
    GeneratedMandateReference {
        value: format!(
            "{SYNTHETIC_MANDATE_PREFIX}{}",
            deterministic_token("payments.mandate-reference", seed, index)
        ),
        synthetic: true,
        generator_version: crate::GENERATOR_VERSION,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn generated_values_are_reproducible_valid_and_unique_within_fixture() {
        let values: Vec<_> = (0..1_000)
            .map(|index| generate_mandate_reference("fixture", index))
            .collect();
        assert_eq!(
            values.len(),
            values
                .iter()
                .map(|item| &item.value)
                .collect::<HashSet<_>>()
                .len()
        );

        for (index, generated) in values.iter().enumerate() {
            assert_eq!(generated.value.len(), 34);
            assert!(generated.value.starts_with(SYNTHETIC_MANDATE_PREFIX));
            assert!(generated
                .value
                .bytes()
                .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'-'));
            assert!(validate_mandate_reference(&generated.value).is_ok());
            assert_eq!(
                generated,
                &generate_mandate_reference("fixture", index as u32)
            );
        }
    }

    #[test]
    fn validator_enforces_max35_and_handles_unicode() {
        assert!(validate_mandate_reference("CREDITOR-ASSIGNED-REFERENCE").is_ok());
        assert!(validate_mandate_reference(&"A".repeat(35)).is_ok());
        assert!(validate_mandate_reference(&"A".repeat(36)).is_err());
        for invalid in ["", "MND-😀", "/MND", "MND//1"] {
            assert!(validate_mandate_reference(invalid).is_err());
        }
    }
}
