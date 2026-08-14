//! Centrally allocated register identifiers used in European energy markets.
//!
//! Syntax and checksum validation in this module never proves allocation. A
//! registry lookup is a separate capability and must retain its own reference
//! data version and result status.

pub mod eic;
pub mod mastr;

pub use eic::{
    lookup_eic_directory, parse_eic, validate_eic, EicAllocationStatus, EicDirectory,
    EicDirectoryRecord, EicDirectoryStatus, EicError, EicObjectType, EicParts,
    EIC_IMPLEMENTATION_GUIDE_VERSION, EIC_REFERENCE_MANUAL_VERSION,
};
pub use mastr::{
    calculate_mastr_check_digit, generate_synthetic_mastr, parse_mastr, validate_mastr,
    MastrAllocationStatus, MastrCollisionGuarantee, MastrError, MastrLifecycle, MastrObjectGroup,
    MastrParts, MastrPrefix, MastrRoleSuffix, MastrSector, SyntheticMastrFixture,
    MASTR_NUMBER_CONCEPT_VERSION, MASTR_PREFIXES, MASTR_WEB_SERVICE_VERSION_CHECKED,
};
