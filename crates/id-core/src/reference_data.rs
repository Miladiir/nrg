//! Versioned reference data embedded into the core crate.

mod catalogs;

pub use catalogs::*;

pub const BUNDESBANK_BLZ_NAME: &str = "bundesbank_blz";
pub const BUNDESBANK_BLZ_VALID_FROM: &str = "2026-06-08";
pub const BUNDESBANK_BLZ_VALID_TO: &str = "2026-09-06";
pub const BUNDESBANK_BLZ_SOURCE_SHA256: &str =
    "3a5484358b326d6d9ed8bea003601a7d79abbc100cdb81fd0d30e0c8a21898a6";
pub const BUNDESBANK_BLZ_SOURCE_URL: &str = "https://www.bundesbank.de/resource/blob/926192/b27b518a016ea7ca7af321eb7289fcf4/472B63F073F071307366337C94F8C870/blz-aktuell-csv-data.csv";
pub const BUNDESBANK_BLZ_SYNTHETIC_BANK_CODE: &str = "00000000";

/// Metadata for the checked-in projection of ENTSO-E's public allocated EIC
/// codes XML. `CREATED_AT` is the timestamp carried by the source document;
/// it is snapshot metadata, not a promise that every local LIO registry is
/// complete or that a record is still current after that instant.
pub const ENTSO_E_EIC_DIRECTORY_NAME: &str = "entso_e_eic_directory";
pub const ENTSO_E_EIC_DIRECTORY_CREATED_AT: &str = "2026-08-13T01:15:13Z";
pub const ENTSO_E_EIC_DIRECTORY_SOURCE_URL: &str =
    "https://eepublicdownloads.blob.core.windows.net/cio-lio/xml/allocated-eic-codes.xml";
pub const ENTSO_E_EIC_DIRECTORY_SOURCE_SHA256: &str =
    "b53840e8e377b11d12c63241a6a06e68ee36c221897930dae97c0c6ff1903a98";
pub const ENTSO_E_EIC_DIRECTORY_PROJECTION_SHA256: &str =
    "85fecd59968b9f9a30c03cc5259dd82499e3c41f70aeaedf09bde6896bfd3b84";
pub const ENTSO_E_EIC_DIRECTORY_RECORD_COUNT: usize = 76256;
pub const ENTSO_E_EIC_DIRECTORY_ACTIVE_RECORD_COUNT: usize = 72916;
pub const ENTSO_E_EIC_DIRECTORY_INACTIVE_RECORD_COUNT: usize = 3340;

const BUNDESBANK_BLZ_CSV: &str =
    include_str!("../../../data/bundesbank_blz_2026-06-08_2026-09-06.csv");

pub(crate) const ENTSO_E_EIC_DIRECTORY_TSV: &str =
    include_str!("../../../data/entso_e_eic_2026-08-13.tsv");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BundesbankBankRecord<'a> {
    pub bank_code: &'a str,
    pub bic: Option<&'a str>,
    pub change_marker: &'a str,
    pub intended_deletion: bool,
    pub successor_bank_code: Option<&'a str>,
}

impl BundesbankBankRecord<'_> {
    /// Whether this record is suitable for the explicit directory-value test profile.
    ///
    /// `D` denotes a deleted bank-code-leading record. The separate deletion
    /// flag indicates an intended future deletion and does not make a record
    /// absent from this snapshot.
    pub fn is_directory_plausible(&self) -> bool {
        self.change_marker != "D" && self.bic.is_some()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct BundesbankBlzDirectory;

impl BundesbankBlzDirectory {
    pub fn records(self) -> impl Iterator<Item = BundesbankBankRecord<'static>> {
        BUNDESBANK_BLZ_CSV
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .skip(1)
            .filter_map(parse_record)
    }

    pub fn record_count(self) -> usize {
        self.records().count()
    }

    /// Looks up a BLZ as it appears in the snapshot, including deletion metadata.
    pub fn lookup(self, bank_code: &str) -> Option<BundesbankBankRecord<'static>> {
        self.records().find(|record| record.bank_code == bank_code)
    }

    pub fn contains_bank_code(self, bank_code: &str) -> bool {
        self.lookup(bank_code).is_some()
    }

    pub fn directory_record(self, index: usize) -> Option<BundesbankBankRecord<'static>> {
        self.records()
            .filter(BundesbankBankRecord::is_directory_plausible)
            .nth(index)
    }

    pub fn directory_record_count(self) -> usize {
        self.records()
            .filter(BundesbankBankRecord::is_directory_plausible)
            .count()
    }
}

fn parse_record(line: &'static str) -> Option<BundesbankBankRecord<'static>> {
    let mut fields = line.split(',');
    let bank_code = fields.next()?;
    let bic = nonempty(fields.next()?);
    let change_marker = fields.next()?;
    let intended_deletion = match fields.next()? {
        "0" => false,
        "1" => true,
        _ => return None,
    };
    let successor_bank_code = nonempty_or_zero(fields.next()?);
    if fields.next().is_some() {
        return None;
    }

    Some(BundesbankBankRecord {
        bank_code,
        bic,
        change_marker,
        intended_deletion,
        successor_bank_code,
    })
}

fn nonempty(value: &str) -> Option<&str> {
    (!value.is_empty()).then_some(value)
}

fn nonempty_or_zero(value: &str) -> Option<&str> {
    (!value.is_empty() && value != "00000000").then_some(value)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn snapshot_has_expected_shape_and_unique_bank_codes() {
        let directory = BundesbankBlzDirectory;
        let records: Vec<_> = directory.records().collect();
        assert_eq!(records.len(), 3_506);
        assert!(directory.directory_record_count() > 3_000);

        let unique: HashSet<_> = records.iter().map(|record| record.bank_code).collect();
        assert_eq!(unique.len(), records.len());
        assert!(records.iter().all(|record| {
            record.bank_code.len() == 8
                && record.bank_code.bytes().all(|byte| byte.is_ascii_digit())
        }));
    }

    #[test]
    fn known_directory_record_is_available_with_its_bic() {
        let record = BundesbankBlzDirectory.lookup("10000000").unwrap();
        assert_eq!(record.bic, Some("MARKDEF1100"));
        assert_eq!(record.change_marker, "U");
        assert!(!record.intended_deletion);
        assert!(record.is_directory_plausible());
    }

    #[test]
    fn configured_synthetic_bank_code_is_verified_absent_from_this_snapshot() {
        assert!(!BundesbankBlzDirectory.contains_bank_code(BUNDESBANK_BLZ_SYNTHETIC_BANK_CODE));
    }
}
