//! Reusable checksum algorithms shared across identifier families.

use std::{error::Error, fmt};

pub mod bdew_ascii;
pub mod bdew_lok_waggon;
pub mod mod97;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChecksumInputError {
    Empty,
    NonAscii,
    InvalidCharacter { position: usize, found: char },
}

impl fmt::Display for ChecksumInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("checksum input must not be empty"),
            Self::NonAscii => {
                formatter.write_str("checksum input must contain only ASCII characters")
            }
            Self::InvalidCharacter { position, found } => write!(
                formatter,
                "invalid checksum character '{found}' at 1-based position {position}"
            ),
        }
    }
}

impl Error for ChecksumInputError {}
