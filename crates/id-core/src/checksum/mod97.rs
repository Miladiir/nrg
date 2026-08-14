//! Streaming ISO 7064 MOD 97-10 primitives.
//!
//! The implementation never materialises the expanded decimal number. Every
//! input character is folded into a remainder in the range `0..=96`, so input
//! length is not constrained by a machine integer or a big-integer library.

use std::fmt;

/// The remainder required for a complete MOD 97-10 identifier.
pub const VALID_REMAINDER: u8 = 1;

/// An error returned while expanding an ASCII alphanumeric MOD-97 input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mod97Error {
    Empty,
    InvalidCharacter { position: usize, character: char },
}

impl fmt::Display for Mod97Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("MOD-97 input must not be empty"),
            Self::InvalidCharacter {
                position,
                character,
            } => write!(
                formatter,
                "invalid MOD-97 character {character:?} at position {position}"
            ),
        }
    }
}

impl std::error::Error for Mod97Error {}

#[inline]
fn push_decimal_digit(remainder: u8, digit: u8) -> u8 {
    ((u16::from(remainder) * 10 + u16::from(digit)) % 97) as u8
}

/// Calculates the MOD-97 remainder of an ASCII alphanumeric string.
///
/// Digits retain their value. Uppercase ASCII letters are expanded according
/// to the ISO convention (`A = 10`, ..., `Z = 35`). Separators, lowercase
/// letters and non-ASCII characters are rejected deliberately; callers should
/// normalise presentation formats before invoking this low-level primitive.
pub fn remainder(input: &str) -> Result<u8, Mod97Error> {
    if input.is_empty() {
        return Err(Mod97Error::Empty);
    }

    let mut current = 0_u8;
    for (position, character) in input.chars().enumerate() {
        match character {
            '0'..='9' => {
                current = push_decimal_digit(current, character as u8 - b'0');
            }
            'A'..='Z' => {
                let value = character as u8 - b'A' + 10;
                current = push_decimal_digit(current, value / 10);
                current = push_decimal_digit(current, value % 10);
            }
            _ => {
                return Err(Mod97Error::InvalidCharacter {
                    position: position + 1,
                    character,
                });
            }
        }
    }
    Ok(current)
}

/// Checks whether a complete, already rearranged identifier has remainder 1.
pub fn is_valid(input: &str) -> Result<bool, Mod97Error> {
    Ok(remainder(input)? == VALID_REMAINDER)
}

/// Calculates two MOD-97 check digits from an already rearranged value.
///
/// `prepared_with_zero_check_digits` must contain the identifier in the order
/// prescribed by its standard and contain `00` at the check-digit position.
/// For an IBAN this is `BBAN + country + "00"`; for an RF reference it is
/// `reference body + "RF00"`.
pub fn calculate_check_digits(prepared_with_zero_check_digits: &str) -> Result<String, Mod97Error> {
    let check_digits = 98_u8 - remainder(prepared_with_zero_check_digits)?;
    Ok(format!("{check_digits:02}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_known_german_iban_rearrangement() {
        // DE89 3704 0044 0532 0130 00, rearranged as required for validation.
        assert_eq!(remainder("370400440532013000DE89"), Ok(1));
        assert_eq!(is_valid("370400440532013000DE89"), Ok(true));
    }

    #[test]
    fn calculates_known_german_iban_check_digits() {
        assert_eq!(
            calculate_check_digits("370400440532013000DE00"),
            Ok("89".to_string())
        );
    }

    #[test]
    fn calculates_known_rf_reference_check_digits() {
        assert_eq!(
            calculate_check_digits("539007547034RF00"),
            Ok("18".to_string())
        );
    }

    #[test]
    fn streams_inputs_larger_than_machine_integers() {
        let input = "1234567890".repeat(10_000);
        let streamed = remainder(&input).unwrap();

        let mut expected = 0_u8;
        for digit in input.bytes() {
            expected = push_decimal_digit(expected, digit - b'0');
        }
        assert_eq!(streamed, expected);
    }

    #[test]
    fn rejects_empty_lowercase_separators_and_unicode_without_panicking() {
        assert_eq!(remainder(""), Err(Mod97Error::Empty));
        for value in ["DE 00", "de00", "DE-00", "DÉ00", "😀"] {
            assert!(remainder(value).is_err(), "accepted {value:?}");
        }
    }
}
