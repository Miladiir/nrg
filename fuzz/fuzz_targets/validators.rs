#![no_main]

use std::str;

use id_core::identifiers::business::{
    lei::{gleif_record_api_url, parse_lei, validate_lei},
    vat_id::{normalize_german_vat_id, parse_german_vat_id, validate_german_vat_id},
};
use id_core::identifiers::energy::{
    validate_bdew_market_partner_id, validate_cr_id, validate_dvgw_market_partner_id,
    validate_market_partner_id, validate_market_partner_id_for_kind, validate_nebe_id,
    validate_network_identifier, validate_network_identifier_for_kind, validate_package_id,
    validate_resource_id, validate_resource_id_for_kind, validate_sg_id, validate_sr_id,
    validate_tr_id, MarketPartnerIdKind, NetworkIdentifierKind, ResourceIdKind,
};
use id_core::identifiers::metering::{
    lookup_curated_obis, parse_din_43849, parse_obis, validate_din_43849, validate_obis,
};
use id_core::identifiers::payments::{
    bic::{is_test_training_bic_pattern, parse_bic, validate_bic},
    creditor_id::{parse_german_creditor_id, validate_creditor_id, validate_german_creditor_id},
    end_to_end_id::validate_end_to_end_id,
    iban::{
        parse_german_iban, validate_german_iban, validate_german_iban_with_directory, validate_iban,
    },
    international_iban::{parse_international_iban, validate_international_iban},
    mandate_reference::validate_mandate_reference,
    rf_reference::{parse_rf_reference, validate_rf_reference},
    uetr::{parse_uetr, validate_uetr},
    validate_sepa_reference,
};
use id_core::identifiers::registers::{parse_eic, parse_mastr, validate_eic, validate_mastr};
use id_core::reference_data::BundesbankBlzDirectory;
use id_core::{validate_malo, validate_melo, validate_nelo};
use libfuzzer_sys::fuzz_target;

/// Exercise every public parsing/validation path with one arbitrary string.
///
/// Every result is intentionally ignored: the invariant under test is that no
/// input, including malformed Unicode, may panic or access an invalid slice.
fn exercise_validators(value: &str) {
    // Legacy energy identifiers.
    let _ = validate_malo(value);
    let _ = validate_melo(value);
    let _ = validate_nelo(value);

    // Current energy-market identifiers, including all strict kind wrappers.
    let _ = validate_market_partner_id(value);
    let _ = validate_bdew_market_partner_id(value);
    let _ = validate_dvgw_market_partner_id(value);
    for kind in [
        MarketPartnerIdKind::BdewElectricity,
        MarketPartnerIdKind::DvgwGas,
    ] {
        let _ = validate_market_partner_id_for_kind(value, kind);
    }

    let _ = validate_network_identifier(value);
    let _ = validate_nebe_id(value);
    let _ = validate_package_id(value);
    for kind in [
        NetworkIdentifierKind::NetworkArea,
        NetworkIdentifierKind::Package,
    ] {
        let _ = validate_network_identifier_for_kind(value, kind);
    }

    let _ = validate_resource_id(value);
    let _ = validate_cr_id(value);
    let _ = validate_sg_id(value);
    let _ = validate_sr_id(value);
    let _ = validate_tr_id(value);
    for kind in [
        ResourceIdKind::ClusterResource,
        ResourceIdKind::ControlGroup,
        ResourceIdKind::ControllableResource,
        ResourceIdKind::TechnicalResource,
    ] {
        let _ = validate_resource_id_for_kind(value, kind);
    }

    // Payments and SEPA references.
    let _ = parse_german_iban(value);
    let _ = validate_german_iban(value);
    let _ = validate_iban(value);
    let _ = validate_german_iban_with_directory(value, &BundesbankBlzDirectory);
    let _ = parse_international_iban(value);
    let _ = validate_international_iban(value);

    let _ = parse_bic(value);
    let _ = validate_bic(value);
    let _ = is_test_training_bic_pattern(value);

    let _ = parse_german_creditor_id(value);
    let _ = validate_german_creditor_id(value);
    let _ = validate_creditor_id(value);
    let _ = validate_sepa_reference(value);
    let _ = validate_mandate_reference(value);
    let _ = validate_end_to_end_id(value);
    let _ = parse_rf_reference(value);
    let _ = validate_rf_reference(value);
    let _ = parse_uetr(value);
    let _ = validate_uetr(value);

    // Business identifiers and external-lookup preparation.
    let _ = normalize_german_vat_id(value);
    let _ = parse_german_vat_id(value);
    let _ = validate_german_vat_id(value);
    let _ = parse_lei(value);
    let _ = validate_lei(value);
    let _ = gleif_record_api_url(value);

    // Centrally allocated register identifiers.
    let _ = parse_mastr(value);
    let _ = validate_mastr(value);
    let _ = parse_eic(value);
    let _ = validate_eic(value);

    // Metering identifiers and the local OBIS lookup path.
    if let Ok(code) = parse_obis(value) {
        let _ = lookup_curated_obis(code);
    }
    let _ = validate_obis(value);
    let _ = parse_din_43849(value);
    let _ = validate_din_43849(value);
}

/// Map arbitrary bytes to valid Unicode scalar values. This complements the
/// raw UTF-8 path: libFuzzer can reach non-ASCII and supplementary-plane input
/// without first discovering a complete valid multi-byte encoding.
fn project_to_unicode(data: &[u8]) -> String {
    data.chunks(4)
        .map(|chunk| {
            let mut bytes = [0_u8; 4];
            bytes[..chunk.len()].copy_from_slice(chunk);
            let scalar = u32::from_le_bytes(bytes) % 0x11_0000;
            char::from_u32(scalar).unwrap_or('\u{fffd}')
        })
        .collect()
}

fuzz_target!(|data: &[u8]| {
    // Lossy conversion preserves every arbitrary byte input as a safe string.
    let lossy = String::from_utf8_lossy(data);
    exercise_validators(&lossy);

    // Preserve exact valid UTF-8 input where possible.
    if let Ok(valid_utf8) = str::from_utf8(data) {
        exercise_validators(valid_utf8);
    }

    // Exercise deliberately constructed Unicode independently of UTF-8 corpus
    // evolution, including astral code points and replacement characters.
    let unicode = project_to_unicode(data);
    exercise_validators(&unicode);
});
