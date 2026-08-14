//! Business and legal-entity identifiers.
//!
//! These identifiers are allocated by public or supervised registries.  The
//! offline modules therefore keep structural and checksum evidence separate
//! from live registry evidence and deliberately expose no value generators.

pub mod lei;
pub mod vat_id;
