use std::fmt;

use crate::fixture::DeterministicRng;

/// Validation errors shared by SEPA `Max35Text` references and identifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SepaReferenceError {
    Empty,
    TooLong { actual: usize, maximum: usize },
    InvalidCharacter { position: usize, character: char },
    StartsOrEndsWithSlash,
    ConsecutiveSlashes,
}

impl fmt::Display for SepaReferenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("SEPA reference must not be empty"),
            Self::TooLong { actual, maximum } => write!(
                formatter,
                "SEPA reference is {actual} characters long; maximum is {maximum}"
            ),
            Self::InvalidCharacter {
                position,
                character,
            } => write!(
                formatter,
                "invalid SEPA reference character {character:?} at position {position}"
            ),
            Self::StartsOrEndsWithSlash => {
                formatter.write_str("SEPA reference must not start or end with '/'")
            }
            Self::ConsecutiveSlashes => formatter.write_str("SEPA reference must not contain '//'"),
        }
    }
}

impl std::error::Error for SepaReferenceError {}

/// Validates a conservative ASCII subset of a SEPA `Max35Text` value.
///
/// The accepted characters are the commonly interoperable SEPA Latin set:
/// letters, digits, space and `/ - ? : ( ) . , ' +`. EPC slash rules are
/// enforced as well. Generators use the narrower `A-Z`, `0-9`, `-` subset.
pub fn validate_sepa_reference(value: &str) -> Result<(), SepaReferenceError> {
    if value.is_empty() {
        return Err(SepaReferenceError::Empty);
    }

    let length = value.chars().count();
    if length > 35 {
        return Err(SepaReferenceError::TooLong {
            actual: length,
            maximum: 35,
        });
    }

    for (position, character) in value.chars().enumerate() {
        let valid = character.is_ascii_alphanumeric()
            || matches!(
                character,
                ' ' | '/' | '-' | '?' | ':' | '(' | ')' | '.' | ',' | '\'' | '+'
            );
        if !valid {
            return Err(SepaReferenceError::InvalidCharacter {
                position: position + 1,
                character,
            });
        }
    }

    if value.starts_with('/') || value.ends_with('/') {
        return Err(SepaReferenceError::StartsOrEndsWithSlash);
    }
    if value.contains("//") {
        return Err(SepaReferenceError::ConsecutiveSlashes);
    }

    Ok(())
}

pub(crate) fn deterministic_token(namespace: &str, seed: &str, index: u32) -> String {
    const CROCKFORD_BASE32: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    let mut rng = DeterministicRng::new(seed, namespace, index);
    let mut token: String = (0..19)
        .map(|_| CROCKFORD_BASE32[rng.index(CROCKFORD_BASE32.len())] as char)
        .collect();

    // Seven base-32 characters encode all 32 bits of `index`. This makes the
    // token injective within one seeded fixture batch instead of merely relying
    // on a negligible pseudo-random collision probability.
    for shift in [30_u32, 25, 20, 15, 10, 5, 0] {
        let digit = ((index >> shift) & 0x1f) as usize;
        token.push(CROCKFORD_BASE32[digit] as char);
    }
    token
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_interoperable_sepa_characters() {
        assert_eq!(
            validate_sepa_reference("Invoice 17-2/A? (final), it's+ok."),
            Ok(())
        );
    }

    #[test]
    fn rejects_length_slash_and_unicode_violations() {
        assert_eq!(validate_sepa_reference(""), Err(SepaReferenceError::Empty));
        assert!(matches!(
            validate_sepa_reference(&"A".repeat(36)),
            Err(SepaReferenceError::TooLong { .. })
        ));
        assert_eq!(
            validate_sepa_reference("/ABC"),
            Err(SepaReferenceError::StartsOrEndsWithSlash)
        );
        assert_eq!(
            validate_sepa_reference("ABC//DEF"),
            Err(SepaReferenceError::ConsecutiveSlashes)
        );
        for value in ["MÄNDAT", "😀", "ABC\nDEF"] {
            assert!(matches!(
                validate_sepa_reference(value),
                Err(SepaReferenceError::InvalidCharacter { .. })
            ));
        }
    }

    #[test]
    fn deterministic_tokens_are_stable_and_domain_separated() {
        let first = deterministic_token("mandate", "fixture", 7);
        assert_eq!(first, deterministic_token("mandate", "fixture", 7));
        assert_ne!(first, deterministic_token("mandate", "fixture", 8));
        assert_ne!(first, deterministic_token("end-to-end", "fixture", 7));
        assert_eq!(first.len(), 26);

        let tokens: std::collections::HashSet<_> = (0..10_000)
            .map(|index| deterministic_token("mandate", "fixture", index))
            .collect();
        assert_eq!(tokens.len(), 10_000);
    }
}
