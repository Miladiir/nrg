//! BDEW/DVGW Lok-und-Waggon check-digit procedure.

use super::ChecksumInputError;

/// Calculates the check digit for an ASCII digit sequence.
pub fn calculate(input: &str) -> Result<u8, ChecksumInputError> {
    if input.is_empty() {
        return Err(ChecksumInputError::Empty);
    }
    if !input.is_ascii() {
        return Err(ChecksumInputError::NonAscii);
    }
    for (index, byte) in input.bytes().enumerate() {
        if !byte.is_ascii_digit() {
            return Err(ChecksumInputError::InvalidCharacter {
                position: index + 1,
                found: char::from(byte),
            });
        }
    }
    Ok(from_valid_ascii_digits(input.as_bytes()))
}

pub(crate) fn from_valid_ascii_digits(input: &[u8]) -> u8 {
    let weighted_sum: u32 = input
        .iter()
        .enumerate()
        .map(|(index, byte)| {
            let digit = u32::from(*byte - b'0');
            if index % 2 == 0 {
                digit
            } else {
                digit * 2
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
        assert_eq!(calculate("4137355924"), Ok(1));
        assert_eq!(calculate(""), Err(ChecksumInputError::Empty));
        assert_eq!(
            calculate("123456789A"),
            Err(ChecksumInputError::InvalidCharacter {
                position: 10,
                found: 'A',
            })
        );
        assert_eq!(calculate("123456789١"), Err(ChecksumInputError::NonAscii));
    }
}
