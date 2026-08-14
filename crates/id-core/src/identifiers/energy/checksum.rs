//! Compatibility names for the shared BDEW checksum modules.

pub use crate::checksum::bdew_ascii::calculate as calculate_bdew_ascii_checksum;
pub use crate::checksum::bdew_lok_waggon::calculate as calculate_lok_waggon_checksum;
pub use crate::checksum::ChecksumInputError;

pub(crate) use crate::checksum::bdew_ascii::from_valid_upper_alphanumeric as bdew_ascii_from_valid_upper_alphanumeric;
pub(crate) use crate::checksum::bdew_lok_waggon::from_valid_ascii_digits as lok_waggon_from_valid_ascii_digits;
