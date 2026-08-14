//! OBIS (Object Identification System) parser and a curated market subset.
//!
//! The six OBIS value groups A through F are structural identifiers, not a
//! checksum scheme. The parser accepts full display notation
//! `A-B:C.D.E*F`, the commonly reduced `C.D.E*F` form and six-byte logical-name
//! notation `A.B.C.D.E.F`. The embedded catalog is deliberately a small,
//! versioned subset and must never be presented as the complete DLMS or German
//! market catalog.

use std::{error::Error, fmt};

pub const OBIS_STRUCTURE_VERSION: &str = "DLMS UA 1000-1 Ed. 17 Part 1 (2025-02-28)";
pub const OBIS_MARKET_CATALOG_VERSION: &str = "BNetzA EDI@Energy 2.5c (2025-10-01)";
pub const OBIS_MARKET_CATALOG_SCOPE: &str =
    "curated non-exhaustive electricity subset; channels and group F are not exhaustive";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObisGroup {
    A,
    B,
    C,
    D,
    E,
    F,
}

impl fmt::Display for ObisGroup {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::D => "D",
            Self::E => "E",
            Self::F => "F",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObisMedia {
    Abstract,
    AcElectricity,
    DcElectricity,
    Reserved(u8),
    HeatCostAllocator,
    ThermalEnergy(u8),
    Gas,
    ColdWater,
    HotWater,
    OtherMedia,
}

impl ObisMedia {
    pub const fn from_group_a(value: u8) -> Self {
        match value {
            0 => Self::Abstract,
            1 => Self::AcElectricity,
            2 => Self::DcElectricity,
            4 => Self::HeatCostAllocator,
            5 | 6 => Self::ThermalEnergy(value),
            7 => Self::Gas,
            8 => Self::ColdWater,
            9 => Self::HotWater,
            15 => Self::OtherMedia,
            other => Self::Reserved(other),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObisCode {
    a: Option<u8>,
    b: Option<u8>,
    c: u8,
    d: u8,
    e: u8,
    f: Option<u8>,
}

impl ObisCode {
    pub const fn a(self) -> Option<u8> {
        self.a
    }

    pub const fn b(self) -> Option<u8> {
        self.b
    }

    pub const fn c(self) -> u8 {
        self.c
    }

    pub const fn d(self) -> u8 {
        self.d
    }

    pub const fn e(self) -> u8 {
        self.e
    }

    pub const fn f(self) -> Option<u8> {
        self.f
    }

    pub const fn media(self) -> Option<ObisMedia> {
        match self.a {
            Some(value) => Some(ObisMedia::from_group_a(value)),
            None => None,
        }
    }

    pub fn format_display(self) -> String {
        let mut result = match (self.a, self.b) {
            (Some(a), Some(b)) => format!("{a}-{b}:{}.{}.{}", self.c, self.d, self.e),
            (None, None) => format!("{}.{}.{}", self.c, self.d, self.e),
            _ => unreachable!("the parser never creates partially present A/B groups"),
        };
        if let Some(f) = self.f {
            result.push('*');
            result.push_str(&f.to_string());
        }
        result
    }

    pub fn format_logical_name(self) -> Option<String> {
        Some(format!(
            "{}.{}.{}.{}.{}.{}",
            self.a?, self.b?, self.c, self.d, self.e, self.f?
        ))
    }

    pub const fn is_manufacturer_specific(self) -> bool {
        matches!(self.b, Some(128..=199))
            || matches!(self.c, 128..=199 | 240)
            || matches!(self.d, 128..=254)
            || matches!(self.e, 128..=254)
            || matches!(self.f, Some(128..=254))
    }
}

impl fmt::Display for ObisCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.format_display())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObisError {
    Empty,
    NonAscii,
    InvalidLayout,
    EmptyGroup {
        group: ObisGroup,
    },
    NonNumericGroup {
        group: ObisGroup,
        value: String,
    },
    OutOfRange {
        group: ObisGroup,
        value: u16,
        maximum: u16,
    },
}

impl fmt::Display for ObisError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("OBIS code must not be empty"),
            Self::NonAscii => formatter.write_str("OBIS code must contain only ASCII"),
            Self::InvalidLayout => formatter
                .write_str("OBIS code must use A-B:C.D.E*F, C.D.E*F, or A.B.C.D.E.F notation"),
            Self::EmptyGroup { group } => write!(formatter, "OBIS group {group} is empty"),
            Self::NonNumericGroup { group, value } => {
                write!(
                    formatter,
                    "OBIS group {group} must be decimal, got {value:?}"
                )
            }
            Self::OutOfRange {
                group,
                value,
                maximum,
            } => write!(
                formatter,
                "OBIS group {group} must be in 0..={maximum}, got {value}"
            ),
        }
    }
}

impl Error for ObisError {}

pub fn parse_obis(input: &str) -> Result<ObisCode, ObisError> {
    if input.is_empty() {
        return Err(ObisError::Empty);
    }
    if !input.is_ascii() {
        return Err(ObisError::NonAscii);
    }
    if input.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return Err(ObisError::InvalidLayout);
    }

    let star_parts: Vec<_> = input.split('*').collect();
    if star_parts.len() > 2 {
        return Err(ObisError::InvalidLayout);
    }
    let left = star_parts[0];
    let f = if star_parts.len() == 2 {
        Some(parse_group(star_parts[1], ObisGroup::F, u8::MAX)?)
    } else {
        None
    };

    if let Some((prefix, measurement)) = left.split_once(':') {
        if left.matches(':').count() != 1 {
            return Err(ObisError::InvalidLayout);
        }
        let (a, b) = prefix.split_once('-').ok_or(ObisError::InvalidLayout)?;
        if prefix.matches('-').count() != 1 {
            return Err(ObisError::InvalidLayout);
        }
        let [c, d, e] = three_groups(measurement)?;
        Ok(ObisCode {
            a: Some(parse_group(a, ObisGroup::A, 15)?),
            b: Some(parse_group(b, ObisGroup::B, u8::MAX)?),
            c: parse_group(c, ObisGroup::C, u8::MAX)?,
            d: parse_group(d, ObisGroup::D, u8::MAX)?,
            e: parse_group(e, ObisGroup::E, u8::MAX)?,
            f,
        })
    } else {
        let groups: Vec<_> = left.split('.').collect();
        match groups.as_slice() {
            [c, d, e] => Ok(ObisCode {
                a: None,
                b: None,
                c: parse_group(c, ObisGroup::C, u8::MAX)?,
                d: parse_group(d, ObisGroup::D, u8::MAX)?,
                e: parse_group(e, ObisGroup::E, u8::MAX)?,
                f,
            }),
            [a, b, c, d, e, logical_f] if f.is_none() => Ok(ObisCode {
                a: Some(parse_group(a, ObisGroup::A, 15)?),
                b: Some(parse_group(b, ObisGroup::B, u8::MAX)?),
                c: parse_group(c, ObisGroup::C, u8::MAX)?,
                d: parse_group(d, ObisGroup::D, u8::MAX)?,
                e: parse_group(e, ObisGroup::E, u8::MAX)?,
                f: Some(parse_group(logical_f, ObisGroup::F, u8::MAX)?),
            }),
            _ => Err(ObisError::InvalidLayout),
        }
    }
}

pub fn validate_obis(input: &str) -> Result<ObisCode, ObisError> {
    parse_obis(input)
}

fn three_groups(input: &str) -> Result<[&str; 3], ObisError> {
    let groups: Vec<_> = input.split('.').collect();
    groups.try_into().map_err(|_| ObisError::InvalidLayout)
}

fn parse_group(input: &str, group: ObisGroup, maximum: u8) -> Result<u8, ObisError> {
    if input.is_empty() {
        return Err(ObisError::EmptyGroup { group });
    }
    if !input.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(ObisError::NonNumericGroup {
            group,
            value: input.to_string(),
        });
    }
    let value = input.parse::<u16>().map_err(|_| ObisError::OutOfRange {
        group,
        value: u16::MAX,
        maximum: u16::from(maximum),
    })?;
    if value > u16::from(maximum) {
        return Err(ObisError::OutOfRange {
            group,
            value,
            maximum: u16::from(maximum),
        });
    }
    Ok(value as u8)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObisCatalogEntry {
    pub pattern: &'static str,
    pub a: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub label_de: &'static str,
    pub unit: &'static str,
}

impl ObisCatalogEntry {
    pub fn matches(self, code: ObisCode) -> bool {
        code.a == Some(self.a)
            && code.c == self.c
            && code.d == self.d
            && code.e == self.e
            && code.b.is_some()
            && matches!(code.f, None | Some(255))
    }
}

/// A deliberately small subset of values explicitly used by the current
/// German EDI@Energy catalog. `b` means any concrete channel. The absence of an
/// entry says nothing about whether another OBIS combination is standardised.
pub const CURATED_OBIS_CATALOG: &[ObisCatalogEntry] = &[
    ObisCatalogEntry {
        pattern: "1-b:1.8.0",
        a: 1,
        c: 1,
        d: 8,
        e: 0,
        label_de: "Wirkarbeit Bezug (+), Zählerstand total, tariflos",
        unit: "kWh",
    },
    ObisCatalogEntry {
        pattern: "1-b:1.8.1",
        a: 1,
        c: 1,
        d: 8,
        e: 1,
        label_de: "Wirkarbeit Bezug (+), Zählerstand Tarif 1",
        unit: "kWh",
    },
    ObisCatalogEntry {
        pattern: "1-b:1.8.2",
        a: 1,
        c: 1,
        d: 8,
        e: 2,
        label_de: "Wirkarbeit Bezug (+), Zählerstand Tarif 2",
        unit: "kWh",
    },
    ObisCatalogEntry {
        pattern: "1-b:2.8.0",
        a: 1,
        c: 2,
        d: 8,
        e: 0,
        label_de: "Wirkarbeit Lieferung (-), Zählerstand total, tariflos",
        unit: "kWh",
    },
    ObisCatalogEntry {
        pattern: "1-b:2.8.1",
        a: 1,
        c: 2,
        d: 8,
        e: 1,
        label_de: "Wirkarbeit Lieferung (-), Zählerstand Tarif 1",
        unit: "kWh",
    },
    ObisCatalogEntry {
        pattern: "1-b:2.8.2",
        a: 1,
        c: 2,
        d: 8,
        e: 2,
        label_de: "Wirkarbeit Lieferung (-), Zählerstand Tarif 2",
        unit: "kWh",
    },
    ObisCatalogEntry {
        pattern: "1-b:1.6.0",
        a: 1,
        c: 1,
        d: 6,
        e: 0,
        label_de: "Wirkleistung Bezug (+), Maximum, tariflos",
        unit: "kW",
    },
    ObisCatalogEntry {
        pattern: "1-b:1.29.0",
        a: 1,
        c: 1,
        d: 29,
        e: 0,
        label_de: "Wirkarbeit Bezug (+), Lastgang total, tariflos",
        unit: "kWh",
    },
];

pub fn lookup_curated_obis(code: ObisCode) -> Option<&'static ObisCatalogEntry> {
    CURATED_OBIS_CATALOG
        .iter()
        .find(|entry| entry.matches(code))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_reduced_and_logical_name_forms_roundtrip() {
        let full = parse_obis("1-0:1.8.0*255").unwrap();
        assert_eq!(full.format_display(), "1-0:1.8.0*255");
        assert_eq!(full.format_logical_name().as_deref(), Some("1.0.1.8.0.255"));
        assert_eq!(parse_obis("1.0.1.8.0.255").unwrap(), full);

        let reduced = parse_obis("1.8.0").unwrap();
        assert_eq!(reduced.format_display(), "1.8.0");
        assert_eq!(reduced.format_logical_name(), None);
        assert_eq!(reduced.media(), None);
    }

    #[test]
    fn structure_ranges_are_enforced_and_unicode_never_panics() {
        assert!(matches!(
            parse_obis("16-0:1.8.0"),
            Err(ObisError::OutOfRange {
                group: ObisGroup::A,
                ..
            })
        ));
        assert!(matches!(
            parse_obis("1-256:1.8.0"),
            Err(ObisError::OutOfRange {
                group: ObisGroup::B,
                ..
            })
        ));
        assert_eq!(parse_obis("1-0:1.8.０"), Err(ObisError::NonAscii));
        for malformed in ["", "1", "1-:1.8.0", "1-0:1..0", "1-0:1.8.0*1*2"] {
            assert!(parse_obis(malformed).is_err(), "accepted {malformed:?}");
        }
    }

    #[test]
    fn generated_structural_values_roundtrip_like_a_property_test() {
        for a in 0..=15_u8 {
            for value in [0_u8, 1, 64, 127, 128, 199, 200, 240, 254, 255] {
                let code = ObisCode {
                    a: Some(a),
                    b: Some(value),
                    c: value,
                    d: value,
                    e: value,
                    f: Some(value),
                };
                let display = code.format_display();
                let logical = code.format_logical_name().unwrap();
                assert_eq!(parse_obis(&display).unwrap(), code);
                assert_eq!(parse_obis(&logical).unwrap(), code);
            }
        }
    }

    #[test]
    fn manufacturer_specific_ranges_follow_the_public_blue_book_excerpt() {
        assert!(parse_obis("1-128:1.8.0")
            .unwrap()
            .is_manufacturer_specific());
        assert!(parse_obis("1-0:240.8.0")
            .unwrap()
            .is_manufacturer_specific());
        assert!(parse_obis("1-0:1.128.0")
            .unwrap()
            .is_manufacturer_specific());
        assert!(!parse_obis("1-0:1.8.0").unwrap().is_manufacturer_specific());
    }

    #[test]
    fn curated_catalog_matches_channels_but_is_explicitly_small() {
        let base = parse_obis("1-0:1.8.0").unwrap();
        let channel_65 = parse_obis("1-65:1.8.0*255").unwrap();
        assert_eq!(lookup_curated_obis(base).unwrap().pattern, "1-b:1.8.0");
        assert_eq!(
            lookup_curated_obis(channel_65).unwrap().pattern,
            "1-b:1.8.0"
        );
        assert!(lookup_curated_obis(parse_obis("7-0:3.0.0").unwrap()).is_none());
        assert!(OBIS_MARKET_CATALOG_SCOPE.contains("non-exhaustive"));
        for (index, entry) in CURATED_OBIS_CATALOG.iter().enumerate() {
            assert!(CURATED_OBIS_CATALOG[index + 1..]
                .iter()
                .all(|other| other.pattern != entry.pattern));
        }
    }
}
