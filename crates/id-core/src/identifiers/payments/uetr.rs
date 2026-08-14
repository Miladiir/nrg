//! SWIFT Unique End-to-End Transaction References (UETR).
//!
//! A UETR is a canonical, lower-case UUID version 4 using the RFC 4122
//! variant.  The deterministic generator in this module exists exclusively
//! for reproducible test fixtures: it is not a cryptographically random UUID
//! generator and its values must not be used for production payments.

use std::fmt;

use crate::fixture::DeterministicRng;

pub const UETR_LENGTH: usize = 36;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UuidVariant {
    Rfc4122,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UetrError {
    Empty,
    InvalidLength { expected: usize, actual: usize },
    InvalidCharacter { position: usize, character: char },
    InvalidSeparator { position: usize },
    InvalidVersion { actual: char },
    InvalidVariant { actual: char },
}

impl fmt::Display for UetrError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("UETR must not be empty"),
            Self::InvalidLength { expected, actual } => {
                write!(formatter, "UETR must be {expected} characters, got {actual}")
            }
            Self::InvalidCharacter {
                position,
                character,
            } => write!(
                formatter,
                "invalid UETR character {character:?} at position {position}; lower-case hexadecimal is required",
            ),
            Self::InvalidSeparator { position } => write!(
                formatter,
                "UETR requires a hyphen at position {position} and nowhere else",
            ),
            Self::InvalidVersion { actual } => write!(
                formatter,
                "UETR UUID version nibble must be '4', got {actual:?}",
            ),
            Self::InvalidVariant { actual } => write!(
                formatter,
                "UETR UUID variant nibble must be one of '8', '9', 'a', or 'b', got {actual:?}",
            ),
        }
    }
}

impl std::error::Error for UetrError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UetrParts {
    pub canonical: String,
    pub bytes: [u8; 16],
    pub version: u8,
    pub variant: UuidVariant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedUetr {
    pub value: String,
    pub parts: UetrParts,
    pub synthetic: bool,
    pub production_usable: bool,
    pub generator_version: &'static str,
}

/// Parses and validates the exact UETR representation required by SWIFT.
///
/// Deliberately rejected presentation variants include braces, missing
/// hyphens, upper-case hexadecimal characters, non-ASCII characters, UUID
/// versions other than 4, and non-RFC-4122 variants.
pub fn parse_uetr(input: &str) -> Result<UetrParts, UetrError> {
    if input.is_empty() {
        return Err(UetrError::Empty);
    }

    for (position, character) in input.chars().enumerate() {
        if !character.is_ascii() {
            return Err(UetrError::InvalidCharacter {
                position: position + 1,
                character,
            });
        }
    }

    if input.len() != UETR_LENGTH {
        return Err(UetrError::InvalidLength {
            expected: UETR_LENGTH,
            actual: input.len(),
        });
    }

    let input_bytes = input.as_bytes();
    const HYPHEN_OFFSETS: [usize; 4] = [8, 13, 18, 23];
    for (offset, byte) in input_bytes.iter().copied().enumerate() {
        if HYPHEN_OFFSETS.contains(&offset) {
            if byte != b'-' {
                return Err(UetrError::InvalidSeparator {
                    position: offset + 1,
                });
            }
        } else if byte == b'-' {
            return Err(UetrError::InvalidSeparator {
                position: offset + 1,
            });
        } else if !(byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)) {
            return Err(UetrError::InvalidCharacter {
                position: offset + 1,
                character: char::from(byte),
            });
        }
    }

    if input_bytes[14] != b'4' {
        return Err(UetrError::InvalidVersion {
            actual: char::from(input_bytes[14]),
        });
    }
    if !matches!(input_bytes[19], b'8' | b'9' | b'a' | b'b') {
        return Err(UetrError::InvalidVariant {
            actual: char::from(input_bytes[19]),
        });
    }

    let mut decoded = [0_u8; 16];
    let mut decoded_index = 0_usize;
    let mut high_nibble = None;
    for byte in input_bytes.iter().copied().filter(|byte| *byte != b'-') {
        let nibble = decode_lower_hex(byte);
        if let Some(high) = high_nibble.take() {
            decoded[decoded_index] = (high << 4) | nibble;
            decoded_index += 1;
        } else {
            high_nibble = Some(nibble);
        }
    }
    debug_assert_eq!(decoded_index, decoded.len());
    debug_assert!(high_nibble.is_none());

    Ok(UetrParts {
        canonical: input.to_string(),
        bytes: decoded,
        version: 4,
        variant: UuidVariant::Rfc4122,
    })
}

pub fn validate_uetr(input: &str) -> Result<UetrParts, UetrError> {
    parse_uetr(input)
}

/// Generates a reproducible UUID-v4-shaped UETR test fixture.
///
/// `seed` and `index` determine all 122 variable bits.  Callers must still
/// scope indices uniquely within their fixture set.  This function makes no
/// global collision guarantee and does not provide cryptographic randomness.
pub fn generate_uetr(seed: &str, index: u32) -> GeneratedUetr {
    let mut rng = DeterministicRng::new(seed, "payments.uetr", index);
    let mut bytes = [0_u8; 16];
    bytes[..8].copy_from_slice(&rng.next_u64().to_be_bytes());
    bytes[8..].copy_from_slice(&rng.next_u64().to_be_bytes());

    // UUID version 4 and RFC 4122 variant (binary 10xx).
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;

    let value = encode_uuid(bytes);
    let parts = parse_uetr(&value).expect("generator always creates a valid UETR");
    GeneratedUetr {
        value,
        parts,
        synthetic: true,
        production_usable: false,
        generator_version: crate::GENERATOR_VERSION,
    }
}

fn decode_lower_hex(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => unreachable!("caller validates lower-case hexadecimal first"),
    }
}

fn encode_uuid(bytes: [u8; 16]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(UETR_LENGTH);
    for (index, byte) in bytes.into_iter().enumerate() {
        if matches!(index, 4 | 6 | 8 | 10) {
            output.push('-');
        }
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn accepts_the_swift_uuid_v4_shape_and_decodes_bytes() {
        let parts = validate_uetr("123e4567-e89b-42d3-a456-426614174000").unwrap();
        assert_eq!(parts.version, 4);
        assert_eq!(parts.variant, UuidVariant::Rfc4122);
        assert_eq!(parts.bytes[6] >> 4, 4);
        assert_eq!(parts.bytes[8] >> 6, 2);
    }

    #[test]
    fn generator_is_reproducible_unique_in_a_fixture_and_self_validating() {
        let mut values = HashSet::new();
        for index in 0..5_000 {
            let generated = generate_uetr("integration-test-4711", index);
            assert_eq!(generated, generate_uetr("integration-test-4711", index));
            assert_eq!(validate_uetr(&generated.value).unwrap(), generated.parts);
            assert!(!generated.production_usable);
            assert!(values.insert(generated.value));
        }
    }

    #[test]
    fn structural_mutations_are_rejected() {
        assert!(matches!(
            validate_uetr("123e4567-e89b-52d3-a456-426614174000"),
            Err(UetrError::InvalidVersion { .. })
        ));
        assert!(matches!(
            validate_uetr("123e4567-e89b-42d3-7456-426614174000"),
            Err(UetrError::InvalidVariant { .. })
        ));
        assert!(matches!(
            validate_uetr("123e4567_e89b-42d3-a456-426614174000"),
            Err(UetrError::InvalidSeparator { .. })
        ));
        assert!(matches!(
            validate_uetr("123E4567-e89b-42d3-a456-426614174000"),
            Err(UetrError::InvalidCharacter { .. })
        ));
    }

    #[test]
    fn arbitrary_unicode_never_panics_or_normalizes_into_a_uetr() {
        for input in [
            "",
            "😀",
            "123e4567-e89b-42d3-a456-42661417400😀",
            "１２３e4567-e89b-42d3-a456-426614174000",
            "{123e4567-e89b-42d3-a456-426614174000}",
        ] {
            assert!(validate_uetr(input).is_err(), "accepted {input:?}");
        }
    }
}
