//! SEPA End-to-End Identifications (`Max35Text`).

use super::reference::{deterministic_token, validate_sepa_reference, SepaReferenceError};

pub const NOT_PROVIDED_END_TO_END_ID: &str = "NOTPROVIDED";
pub const SYNTHETIC_END_TO_END_PREFIX: &str = "NRG-E2E-";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedEndToEndId {
    pub value: String,
    pub not_provided: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedEndToEndId {
    pub value: String,
    pub synthetic: bool,
    pub generator_version: &'static str,
}

pub fn validate_end_to_end_id(input: &str) -> Result<ValidatedEndToEndId, SepaReferenceError> {
    validate_sepa_reference(input)?;
    Ok(ValidatedEndToEndId {
        value: input.to_string(),
        not_provided: input == NOT_PROVIDED_END_TO_END_ID,
    })
}

/// Generates a concrete synthetic payment reference. `NOTPROVIDED` is valid
/// input but deliberately never the generator default.
pub fn generate_end_to_end_id(seed: &str, index: u32) -> GeneratedEndToEndId {
    GeneratedEndToEndId {
        value: format!(
            "{SYNTHETIC_END_TO_END_PREFIX}{}",
            deterministic_token("payments.end-to-end-id", seed, index)
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
    fn recognizes_explicit_not_provided_value() {
        let sentinel = validate_end_to_end_id(NOT_PROVIDED_END_TO_END_ID).unwrap();
        assert!(sentinel.not_provided);
        assert!(!validate_end_to_end_id("notprovided").unwrap().not_provided);
    }

    #[test]
    fn generator_never_defaults_to_not_provided_and_is_reproducible() {
        let values: Vec<_> = (0..1_000)
            .map(|index| generate_end_to_end_id("fixture", index))
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
            assert!(generated.value.starts_with(SYNTHETIC_END_TO_END_PREFIX));
            assert_ne!(generated.value, NOT_PROVIDED_END_TO_END_ID);
            assert!(
                !validate_end_to_end_id(&generated.value)
                    .unwrap()
                    .not_provided
            );
            assert_eq!(generated, &generate_end_to_end_id("fixture", index as u32));
        }
    }

    #[test]
    fn validator_enforces_length_character_and_slash_rules() {
        assert!(validate_end_to_end_id(&"A".repeat(35)).is_ok());
        for invalid in ["", &"A".repeat(36), "E2E-😀", "/E2E", "E2E//1"] {
            assert!(validate_end_to_end_id(invalid).is_err());
        }
    }
}
