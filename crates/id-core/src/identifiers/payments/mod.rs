//! Payment and SEPA identifiers.
//!
//! Generators in this module produce deterministic test fixtures. They are not
//! cryptographic random-number generators and must never be used for secrets.

mod reference;

pub mod bic;
pub mod creditor_id;
pub mod end_to_end_id;
pub mod iban;
pub mod international_iban;
pub mod mandate_reference;
pub mod rf_reference;
pub mod uetr;

pub use reference::{validate_sepa_reference, SepaReferenceError};
