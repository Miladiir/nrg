//! German energy-market identifiers whose normative shape is published by BDEW.
//!
//! The validators in this module deliberately stop at syntax and checksum
//! validation. MP-IDs and resource IDs are allocated centrally, so neither a
//! successful validation nor a value produced by the deterministic fixture
//! generators proves that an identifier is allocated (or unallocated).
//!
//! Normative source: BDEW, "Identifikatoren in der Marktkommunikation:
//! Bildungsvorschriften und Vergabeprozesse", version 1.2, 2025-02-07.
//! See `SOURCES.md` next to this file for the exact source URLs and scope.

pub mod checksum;
pub mod market_partner;
pub mod network;
pub mod resource;

pub use market_partner::{
    calculate_market_partner_check_digit, generate_bdew_market_partner_id,
    generate_dvgw_market_partner_id, generate_market_partner_id, validate_bdew_market_partner_id,
    validate_dvgw_market_partner_id, validate_market_partner_id,
    validate_market_partner_id_for_kind, MarketPartnerIdError, MarketPartnerIdKind,
};
pub use network::{
    calculate_network_identifier_check_digit, generate_nebe_id, generate_network_identifier,
    generate_package_id, validate_nebe_id, validate_network_identifier,
    validate_network_identifier_for_kind, validate_package_id, NetworkIdentifierError,
    NetworkIdentifierKind,
};
pub use resource::{
    calculate_resource_check_digit, generate_cr_id, generate_resource_id, generate_sg_id,
    generate_sr_id, generate_tr_id, validate_cr_id, validate_resource_id,
    validate_resource_id_for_kind, validate_sg_id, validate_sr_id, validate_tr_id, ResourceIdError,
    ResourceIdKind,
};

/// Allocation information available without querying an authoritative registry.
///
/// The core package performs no registry lookup. Keeping this as an enum leaves
/// room for a separate, explicitly reference-data-backed layer later without
/// letting format validation imply allocation today.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CentralAllocationStatus {
    Unknown,
}

/// A deterministic test value with a valid syntax and checksum.
///
/// Generated values have no collision guarantee against centrally allocated
/// identifiers. `allocation_status` is therefore always `Unknown` in this core
/// module and the value must not be presented as assigned or non-routable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedEnergyIdentifier<K> {
    pub value: String,
    pub kind: K,
    pub check_digit: u8,
    pub allocation_status: CentralAllocationStatus,
    pub generator_version: &'static str,
}

impl<K> GeneratedEnergyIdentifier<K> {
    pub(crate) fn new(value: String, kind: K, check_digit: u8) -> Self {
        Self {
            value,
            kind,
            check_digit,
            allocation_status: CentralAllocationStatus::Unknown,
            generator_version: crate::GENERATOR_VERSION,
        }
    }
}

/// A value that passed all syntax and checksum rules implemented by this core.
///
/// This report is deliberately not an allocation or existence result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedEnergyIdentifier<K> {
    pub value: String,
    pub kind: K,
    pub check_digit: u8,
    pub allocation_status: CentralAllocationStatus,
}

impl<K> ValidatedEnergyIdentifier<K> {
    pub(crate) fn new(value: String, kind: K, check_digit: u8) -> Self {
        Self {
            value,
            kind,
            check_digit,
            allocation_status: CentralAllocationStatus::Unknown,
        }
    }
}
