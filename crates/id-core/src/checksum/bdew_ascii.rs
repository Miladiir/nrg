//! BDEW ASCII check-digit procedure for uppercase alphanumeric identifiers.

use super::ChecksumInputError;

/// Calculates the BDEW ASCII check digit for uppercase ASCII alphanumerics.
pub fn calculate(input: &str) -> Result<u8, ChecksumInputError> {
    if input.is_empty() {
        return Err(ChecksumInputError::Empty);
    }
    if !input.is_ascii() {
        return Err(ChecksumInputError::NonAscii);
    }
    for (index, byte) in input.bytes().enumerate() {
        if !(byte.is_ascii_digit() || byte.is_ascii_uppercase()) {
            return Err(ChecksumInputError::InvalidCharacter {
                position: index + 1,
                found: char::from(byte),
            });
        }
    }
    Ok(from_valid_upper_alphanumeric(input.as_bytes()))
}

pub(crate) fn from_valid_upper_alphanumeric(input: &[u8]) -> u8 {
    let weighted_sum: u32 = input
        .iter()
        .enumerate()
        .map(|(index, byte)| {
            let value = if byte.is_ascii_digit() {
                u32::from(*byte - b'0')
            } else {
                u32::from(*byte)
            };
            if index % 2 == 0 {
                value
            } else {
                value * 2
            }
        })
        .sum();
    ((10 - (weighted_sum % 10)) % 10) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn official_example_and_invalid_inputs() {
        assert_eq!(calculate("A113735592"), Ok(5));
        assert_eq!(calculate(""), Err(ChecksumInputError::Empty));
        assert_eq!(
            calculate("A12345678a"),
            Err(ChecksumInputError::InvalidCharacter {
                position: 10,
                found: 'a',
            })
        );
        assert_eq!(calculate("A12345678ß"), Err(ChecksumInputError::NonAscii));
    }
}
