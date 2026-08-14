//! Metering identifiers and measurement-data codes.
//!
//! The parsers distinguish structural validity from directory/catalog
//! membership. Neither a DLMS FLAG ID nor a physical device is looked up here.

pub mod din_43849;
pub mod obis;

pub use din_43849::{
    parse_din_43849, validate_din_43849, Din43849Category, Din43849Error, Din43849Identifier,
    Din43849ManufacturerStatus, DIN_43849_EDITION, DIN_43849_PUBLIC_STRUCTURE_SOURCE,
};
pub use obis::{
    lookup_curated_obis, parse_obis, validate_obis, ObisCatalogEntry, ObisCode, ObisError,
    ObisGroup, ObisMedia, CURATED_OBIS_CATALOG, OBIS_MARKET_CATALOG_SCOPE,
    OBIS_MARKET_CATALOG_VERSION, OBIS_STRUCTURE_VERSION,
};
