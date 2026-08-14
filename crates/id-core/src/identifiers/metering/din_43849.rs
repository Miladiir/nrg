//! Cross-manufacturer device identifiers from DIN 43849:2024-05.
//!
//! The DIN text is not freely available. This validator therefore implements
//! only the field structure corroborated by the official DIN/DKE overview,
//! DLMS FLAG-ID rules and the public OMS specification: one OBIS category,
//! three uppercase manufacturer letters, a two-digit fabrication block and an
//! eight-digit fabrication number. It does not invent a checksum or claim that
//! a manufacturer/device is registered.

use std::{error::Error, fmt};

pub const DIN_43849_EDITION: &str = "DIN 43849:2024-05";
pub const DIN_43849_PUBLIC_STRUCTURE_SOURCE: &str = "OMS Vol. 2 Issue 5.0.1 (2023-12)";

const ELECTRONIC_LENGTH: usize = 14;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Din43849Category {
    AcElectricityMeter,
    DcElectricityMeter,
    HeatCostAllocator,
    CoolingMeter,
    HeatMeter,
    GasMeter,
    ColdWaterMeter,
    WarmWaterMeter,
    BusOrSystemDevice,
    OtherMedia,
    ControlOrSwitchingDevice,
    Unclassified(char),
}

impl Din43849Category {
    pub const fn from_character(character: char) -> Self {
        match character {
            '1' => Self::AcElectricityMeter,
            '2' => Self::DcElectricityMeter,
            '4' => Self::HeatCostAllocator,
            '5' => Self::CoolingMeter,
            '6' => Self::HeatMeter,
            '7' => Self::GasMeter,
            '8' => Self::ColdWaterMeter,
            '9' => Self::WarmWaterMeter,
            'E' => Self::BusOrSystemDevice,
            'F' => Self::OtherMedia,
            'G' => Self::ControlOrSwitchingDevice,
            other => Self::Unclassified(other),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Din43849ManufacturerStatus {
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Din43849Identifier {
    pub electronic: String,
    pub formatted: String,
    pub category_character: char,
    pub category: Din43849Category,
    pub manufacturer_id: String,
    pub fabrication_block: String,
    pub fabrication_number: String,
    pub manufacturer_status: Din43849ManufacturerStatus,
    pub checksum_applicable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Din43849Error {
    Empty,
    NonAscii,
    InvalidPresentation,
    InvalidLength { actual: usize },
    InvalidCategory { found: char },
    InvalidManufacturerCharacter { position: usize, found: char },
    InvalidFabricationBlockCharacter { position: usize, found: char },
    InvalidFabricationNumberCharacter { position: usize, found: char },
}

impl fmt::Display for Din43849Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("DIN 43849 identifier must not be empty"),
            Self::NonAscii => {
                formatter.write_str("DIN 43849 identifier must contain only ASCII")
            }
            Self::InvalidPresentation => formatter.write_str(
                "formatted DIN 43849 identifier must use groups 1-3-2-4-4 separated by spaces",
            ),
            Self::InvalidLength { actual } => write!(
                formatter,
                "DIN 43849 identifier must be {ELECTRONIC_LENGTH} electronic characters, got {actual}"
            ),
            Self::InvalidCategory { found } => write!(
                formatter,
                "DIN 43849 category must be one uppercase ASCII letter or digit, got {found:?}"
            ),
            Self::InvalidManufacturerCharacter { position, found } => write!(
                formatter,
                "DIN 43849 manufacturer ID must contain uppercase ASCII letters; got {found:?} at position {position}"
            ),
            Self::InvalidFabricationBlockCharacter { position, found } => write!(
                formatter,
                "DIN 43849 fabrication block must be numeric; got {found:?} at position {position}"
            ),
            Self::InvalidFabricationNumberCharacter { position, found } => write!(
                formatter,
                "DIN 43849 fabrication number must be numeric; got {found:?} at position {position}"
            ),
        }
    }
}

impl Error for Din43849Error {}

pub fn parse_din_43849(input: &str) -> Result<Din43849Identifier, Din43849Error> {
    let electronic = normalize_presentation(input)?;
    let bytes = electronic.as_bytes();
    let category_character = char::from(bytes[0]);
    if !(bytes[0].is_ascii_uppercase() || bytes[0].is_ascii_digit()) {
        return Err(Din43849Error::InvalidCategory {
            found: category_character,
        });
    }
    for (index, byte) in bytes[1..4].iter().copied().enumerate() {
        if !byte.is_ascii_uppercase() {
            return Err(Din43849Error::InvalidManufacturerCharacter {
                position: index + 2,
                found: char::from(byte),
            });
        }
    }
    for (index, byte) in bytes[4..6].iter().copied().enumerate() {
        if !byte.is_ascii_digit() {
            return Err(Din43849Error::InvalidFabricationBlockCharacter {
                position: index + 5,
                found: char::from(byte),
            });
        }
    }
    for (index, byte) in bytes[6..14].iter().copied().enumerate() {
        if !byte.is_ascii_digit() {
            return Err(Din43849Error::InvalidFabricationNumberCharacter {
                position: index + 7,
                found: char::from(byte),
            });
        }
    }

    Ok(Din43849Identifier {
        formatted: format!(
            "{} {} {} {} {}",
            &electronic[..1],
            &electronic[1..4],
            &electronic[4..6],
            &electronic[6..10],
            &electronic[10..14]
        ),
        category_character,
        category: Din43849Category::from_character(category_character),
        manufacturer_id: electronic[1..4].to_string(),
        fabrication_block: electronic[4..6].to_string(),
        fabrication_number: electronic[6..14].to_string(),
        electronic,
        manufacturer_status: Din43849ManufacturerStatus::Unknown,
        checksum_applicable: false,
    })
}

pub fn validate_din_43849(input: &str) -> Result<Din43849Identifier, Din43849Error> {
    parse_din_43849(input)
}

fn normalize_presentation(input: &str) -> Result<String, Din43849Error> {
    if input.is_empty() {
        return Err(Din43849Error::Empty);
    }
    if !input.is_ascii() {
        return Err(Din43849Error::NonAscii);
    }
    if !input.contains(char::is_whitespace) {
        if input.len() != ELECTRONIC_LENGTH {
            return Err(Din43849Error::InvalidLength {
                actual: input.len(),
            });
        }
        return Ok(input.to_string());
    }

    let groups: Vec<_> = input.split_ascii_whitespace().collect();
    if groups.len() != 5
        || groups[0].len() != 1
        || groups[1].len() != 3
        || groups[2].len() != 2
        || groups[3].len() != 4
        || groups[4].len() != 4
    {
        return Err(Din43849Error::InvalidPresentation);
    }
    let electronic = groups.concat();
    if electronic.len() != ELECTRONIC_LENGTH {
        return Err(Din43849Error::InvalidLength {
            actual: electronic.len(),
        });
    }
    Ok(electronic)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_oms_example_parses_in_electronic_and_grouped_form() {
        let electronic = parse_din_43849("7QDS0111223344").unwrap();
        let grouped = parse_din_43849("7 QDS 01 1122 3344").unwrap();
        assert_eq!(electronic, grouped);
        assert_eq!(electronic.category, Din43849Category::GasMeter);
        assert_eq!(electronic.manufacturer_id, "QDS");
        assert_eq!(electronic.fabrication_block, "01");
        assert_eq!(electronic.fabrication_number, "11223344");
        assert_eq!(
            electronic.manufacturer_status,
            Din43849ManufacturerStatus::Unknown
        );
        assert!(!electronic.checksum_applicable);
    }

    #[test]
    fn newly_documented_categories_are_classified_without_claiming_full_catalog_coverage() {
        assert_eq!(
            parse_din_43849("2ABC0000000001").unwrap().category,
            Din43849Category::DcElectricityMeter
        );
        assert_eq!(
            parse_din_43849("GABC0000000001").unwrap().category,
            Din43849Category::ControlOrSwitchingDevice
        );
        assert_eq!(
            parse_din_43849("ZABC0000000001").unwrap().category,
            Din43849Category::Unclassified('Z')
        );
    }

    #[test]
    fn structural_mutations_and_unicode_are_rejected() {
        assert!(matches!(
            parse_din_43849("71DS0111223344"),
            Err(Din43849Error::InvalidManufacturerCharacter { position: 2, .. })
        ));
        assert!(matches!(
            parse_din_43849("7QDSA111223344"),
            Err(Din43849Error::InvalidFabricationBlockCharacter { position: 5, .. })
        ));
        assert!(matches!(
            parse_din_43849("7QDS011122334A"),
            Err(Din43849Error::InvalidFabricationNumberCharacter { position: 14, .. })
        ));
        assert_eq!(
            parse_din_43849("7QDŚ0111223344"),
            Err(Din43849Error::NonAscii)
        );
    }

    #[test]
    fn many_composed_syntax_values_roundtrip_through_the_formatter() {
        for index in 0..500_u32 {
            let category = char::from(b'0' + (index % 10) as u8);
            let first = char::from(b'A' + (index % 26) as u8);
            let second = char::from(b'A' + ((index / 26) % 26) as u8);
            let third = char::from(b'A' + ((index / (26 * 26)) % 26) as u8);
            let electronic = format!(
                "{category}{first}{second}{third}{:02}{:08}",
                index % 100,
                index
            );
            let parsed = parse_din_43849(&electronic).unwrap();
            assert_eq!(parse_din_43849(&parsed.formatted).unwrap(), parsed);
            assert_eq!(parsed.electronic, electronic);
        }
    }
}
