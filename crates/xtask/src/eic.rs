use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
    io::Cursor,
};

use quick_xml::{events::Event, Reader};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::DynError;

pub const EIC_DIRECTORY_NAME: &str = "entso_e_eic_directory";
pub const EIC_BULK_XML_URL: &str =
    "https://eepublicdownloads.blob.core.windows.net/cio-lio/xml/allocated-eic-codes.xml";
pub const MAX_UNCONFIRMED_CARDINALITY_CHANGE_PERCENT: usize = 5;
/// Privacy-minimized fields persisted from the public bulk export.
///
/// Every other source field is deliberately ignored. In particular, names,
/// descriptions, functions, dates, contacts, addresses and responsible-party
/// data must never enter the checked-in projection or a lookup response.
pub const EIC_FIELDS: [&str; 2] = ["eic_code", "status"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EicStatus {
    Active,
    Inactive,
}

impl EicStatus {
    pub const fn snapshot_value(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
        }
    }

    fn from_source(value: &str) -> Result<Self, DynError> {
        match value {
            "A05" => Ok(Self::Active),
            "A03" => Ok(Self::Inactive),
            value => Err(format!("unsupported EIC docStatus `{value}`").into()),
        }
    }

    fn from_snapshot(value: &str) -> Result<Self, DynError> {
        match value {
            "active" => Ok(Self::Active),
            "inactive" => Ok(Self::Inactive),
            value => Err(format!("unsupported EIC snapshot status `{value}`").into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EicRecord {
    pub eic_code: String,
    pub status: EicStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EicProjection {
    pub created_at: String,
    pub source_sha256: String,
    pub records: Vec<EicRecord>,
}

impl EicProjection {
    pub fn active_record_count(&self) -> usize {
        self.records
            .iter()
            .filter(|record| record.status == EicStatus::Active)
            .count()
    }

    pub fn inactive_record_count(&self) -> usize {
        self.records.len() - self.active_record_count()
    }

    pub fn snapshot_date(&self) -> Result<&str, DynError> {
        let date = self
            .created_at
            .get(..10)
            .ok_or("EIC createdDateTime is too short")?;
        validate_iso_date(date, "createdDateTime")?;
        Ok(date)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EicSnapshotMetadata {
    pub created_at: String,
    pub source_url: String,
    pub source_sha256: String,
    pub record_count: usize,
    pub active_record_count: usize,
    pub inactive_record_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EicDiff {
    pub previous_count: usize,
    pub next_count: usize,
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub changed: Vec<String>,
    pub activated: Vec<String>,
    pub deactivated: Vec<String>,
}

impl EicDiff {
    pub fn has_strong_cardinality_change(&self) -> bool {
        if self.previous_count == 0 {
            return self.next_count != 0;
        }
        let delta = self.previous_count.abs_diff(self.next_count) as u128;
        delta * 100
            > self.previous_count as u128 * MAX_UNCONFIRMED_CARDINALITY_CHANGE_PERCENT as u128
    }
}

#[derive(Debug, Default)]
struct RawRecord {
    eic_code: String,
    status: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Capture {
    DocumentCreatedAt,
    Code,
    Status,
}

impl Capture {
    fn for_element(name: &[u8], inside_record: bool) -> Option<Self> {
        if !inside_record {
            return (name == b"createdDateTime").then_some(Self::DocumentCreatedAt);
        }
        match name {
            b"mRID" => Some(Self::Code),
            b"value" => Some(Self::Status),
            _ => None,
        }
    }
}

pub fn project_eic_xml(source: &[u8]) -> Result<EicProjection, DynError> {
    let mut reader = Reader::from_reader(Cursor::new(source));
    reader.config_mut().trim_text(false);
    let mut buffer = Vec::new();
    let mut created_at = String::new();
    let mut current_record: Option<RawRecord> = None;
    let mut capture: Option<Capture> = None;
    let mut captured_text = String::new();
    let mut records = Vec::new();

    loop {
        buffer.clear();
        match reader.read_event_into(&mut buffer)? {
            Event::Start(event) => {
                let name = event.local_name();
                if name.as_ref() == b"EICCode_MarketDocument" {
                    if current_record.is_some() {
                        return Err("nested EICCode_MarketDocument element".into());
                    }
                    current_record = Some(RawRecord::default());
                } else if let Some(field) =
                    Capture::for_element(name.as_ref(), current_record.is_some())
                {
                    capture = Some(field);
                    captured_text.clear();
                }
            }
            Event::Text(text) => {
                if capture.is_some() {
                    captured_text.push_str(&text.decode()?);
                }
            }
            Event::GeneralRef(reference) => {
                if capture.is_some() {
                    if let Some(character) = reference.resolve_char_ref()? {
                        captured_text.push(character);
                    } else {
                        let name = reference.decode()?;
                        let replacement = quick_xml::escape::resolve_predefined_entity(&name)
                            .ok_or_else(|| format!("unsupported XML entity `&{name};`"))?;
                        captured_text.push_str(replacement);
                    }
                }
            }
            Event::CData(text) => {
                if capture.is_some() {
                    captured_text.push_str(&text.decode()?);
                }
            }
            Event::End(event) => {
                let name = event.local_name();
                if name.as_ref() == b"EICCode_MarketDocument" {
                    let raw = current_record
                        .take()
                        .ok_or("EICCode_MarketDocument closed without an open record")?;
                    records.push(finalize_record(raw)?);
                    capture = None;
                    captured_text.clear();
                } else if capture.is_some_and(|field| {
                    Capture::for_element(name.as_ref(), current_record.is_some()) == Some(field)
                }) {
                    let field = capture.take().expect("capture checked above");
                    let value = normalize_text(&captured_text);
                    assign_captured(field, value, &mut created_at, current_record.as_mut())?;
                    captured_text.clear();
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }

    if current_record.is_some() {
        return Err("unterminated EICCode_MarketDocument element".into());
    }
    validate_created_at(&created_at)?;
    if records.is_empty() {
        return Err("ENTSO-E EIC XML contains no records".into());
    }
    records.sort_by(|left, right| left.eic_code.cmp(&right.eic_code));
    ensure_unique_sorted(&records)?;

    Ok(EicProjection {
        created_at,
        source_sha256: hex::encode(Sha256::digest(source)),
        records,
    })
}

fn assign_captured(
    field: Capture,
    value: String,
    created_at: &mut String,
    record: Option<&mut RawRecord>,
) -> Result<(), DynError> {
    if field == Capture::DocumentCreatedAt {
        if !created_at.is_empty() && *created_at != value {
            return Err("multiple conflicting EIC createdDateTime values".into());
        }
        *created_at = value;
        return Ok(());
    }
    let record = record.ok_or("EIC record field occurred outside a record")?;
    match field {
        Capture::DocumentCreatedAt => unreachable!(),
        Capture::Code => record.eic_code = value,
        Capture::Status => record.status = value,
    }
    Ok(())
}

fn finalize_record(raw: RawRecord) -> Result<EicRecord, DynError> {
    validate_eic_code(&raw.eic_code)?;
    let status = EicStatus::from_source(&raw.status)?;
    Ok(EicRecord {
        eic_code: raw.eic_code,
        status,
    })
}

pub fn render_eic_snapshot(projection: &EicProjection) -> Result<String, DynError> {
    ensure_unique_sorted(&projection.records)?;
    let mut output = String::new();
    writeln!(&mut output, "# dataset={EIC_DIRECTORY_NAME}")?;
    writeln!(&mut output, "# created_at={}", projection.created_at)?;
    writeln!(&mut output, "# source={EIC_BULK_XML_URL}")?;
    writeln!(&mut output, "# source_sha256={}", projection.source_sha256)?;
    writeln!(&mut output, "# record_count={}", projection.records.len())?;
    writeln!(
        &mut output,
        "# active_record_count={}",
        projection.active_record_count()
    )?;
    writeln!(
        &mut output,
        "# inactive_record_count={}",
        projection.inactive_record_count()
    )?;
    writeln!(&mut output, "{}", EIC_FIELDS.join("\t"))?;
    for record in &projection.records {
        writeln!(
            &mut output,
            "{}\t{}",
            record.eic_code,
            record.status.snapshot_value(),
        )?;
    }
    Ok(output)
}

pub fn parse_eic_snapshot(text: &str) -> Result<(EicSnapshotMetadata, Vec<EicRecord>), DynError> {
    let comments: BTreeMap<_, _> = text
        .lines()
        .take_while(|line| line.starts_with('#'))
        .map(|line| {
            line.strip_prefix("# ")
                .and_then(|line| line.split_once('='))
                .ok_or_else(|| format!("invalid EIC snapshot metadata line `{line}`"))
        })
        .collect::<Result<_, _>>()?;
    if required_comment(&comments, "dataset")? != EIC_DIRECTORY_NAME {
        return Err(format!("EIC snapshot dataset must be `{EIC_DIRECTORY_NAME}`").into());
    }
    let created_at = required_comment(&comments, "created_at")?.to_owned();
    validate_created_at(&created_at)?;
    let source_url = required_comment(&comments, "source")?.to_owned();
    if source_url != EIC_BULK_XML_URL {
        return Err(format!("EIC snapshot source must be `{EIC_BULK_XML_URL}`").into());
    }
    let source_sha256 = required_comment(&comments, "source_sha256")?.to_owned();
    validate_sha256(&source_sha256)?;
    let record_count = parse_count(&comments, "record_count")?;
    let active_record_count = parse_count(&comments, "active_record_count")?;
    let inactive_record_count = parse_count(&comments, "inactive_record_count")?;
    if record_count != active_record_count + inactive_record_count {
        return Err("EIC active and inactive counts do not add up to record_count".into());
    }

    let mut lines = text.lines().skip_while(|line| line.starts_with('#'));
    let header = lines.next().ok_or("EIC snapshot contains metadata only")?;
    if header != EIC_FIELDS.join("\t") {
        return Err(format!("invalid EIC snapshot header `{header}`").into());
    }
    let mut records = Vec::with_capacity(record_count);
    for (offset, line) in lines.enumerate() {
        if line.is_empty() {
            return Err(format!("empty EIC snapshot row {}", offset + 2).into());
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() != EIC_FIELDS.len() {
            return Err(format!(
                "EIC snapshot row {} has {} fields, expected {}",
                offset + 2,
                fields.len(),
                EIC_FIELDS.len()
            )
            .into());
        }
        validate_eic_code(fields[0])?;
        let status = EicStatus::from_snapshot(fields[1])?;
        records.push(EicRecord {
            eic_code: fields[0].to_owned(),
            status,
        });
    }
    ensure_unique_sorted(&records)?;
    if records.len() != record_count {
        return Err(format!(
            "EIC snapshot declares {record_count} records, parsed {}",
            records.len()
        )
        .into());
    }
    let actual_active = records
        .iter()
        .filter(|record| record.status == EicStatus::Active)
        .count();
    if actual_active != active_record_count {
        return Err(format!(
            "EIC snapshot declares {active_record_count} active records, parsed {actual_active}"
        )
        .into());
    }

    Ok((
        EicSnapshotMetadata {
            created_at,
            source_url,
            source_sha256,
            record_count,
            active_record_count,
            inactive_record_count,
        },
        records,
    ))
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

pub fn diff_records(previous: &[EicRecord], next: &[EicRecord]) -> EicDiff {
    let previous_map: BTreeMap<_, _> = previous
        .iter()
        .map(|record| (record.eic_code.as_str(), record))
        .collect();
    let next_map: BTreeMap<_, _> = next
        .iter()
        .map(|record| (record.eic_code.as_str(), record))
        .collect();

    let added = next_map
        .keys()
        .filter(|code| !previous_map.contains_key(*code))
        .map(|code| (*code).to_owned())
        .collect();
    let removed = previous_map
        .keys()
        .filter(|code| !next_map.contains_key(*code))
        .map(|code| (*code).to_owned())
        .collect();
    let changed = next_map
        .iter()
        .filter(|(code, record)| {
            previous_map
                .get(*code)
                .is_some_and(|previous| previous != *record)
        })
        .map(|(code, _)| (*code).to_owned())
        .collect();
    let activated = next_map
        .iter()
        .filter(|(code, record)| {
            record.status == EicStatus::Active
                && previous_map
                    .get(*code)
                    .is_some_and(|previous| previous.status == EicStatus::Inactive)
        })
        .map(|(code, _)| (*code).to_owned())
        .collect();
    let deactivated = next_map
        .iter()
        .filter(|(code, record)| {
            record.status == EicStatus::Inactive
                && previous_map
                    .get(*code)
                    .is_some_and(|previous| previous.status == EicStatus::Active)
        })
        .map(|(code, _)| (*code).to_owned())
        .collect();

    EicDiff {
        previous_count: previous.len(),
        next_count: next.len(),
        added,
        removed,
        changed,
        activated,
        deactivated,
    }
}

fn ensure_unique_sorted(records: &[EicRecord]) -> Result<(), DynError> {
    let mut codes = BTreeSet::new();
    for record in records {
        if !codes.insert(record.eic_code.as_str()) {
            return Err(format!("duplicate EIC code `{}`", record.eic_code).into());
        }
    }
    if !records
        .windows(2)
        .all(|window| window[0].eic_code < window[1].eic_code)
    {
        return Err("EIC records must be strictly sorted by code".into());
    }
    Ok(())
}

fn normalize_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn validate_eic_code(value: &str) -> Result<(), DynError> {
    if value.len() != 16 || !value.is_ascii() {
        return Err(format!("EIC code `{value}` must contain 16 ASCII characters").into());
    }
    let bytes = value.as_bytes();
    // The official bulk file retains a handful of inactive legacy identifiers
    // containing `_` or a lowercase letter. Preserve those records exactly;
    // the public EIC validator remains deliberately stricter.
    if !bytes
        .iter()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'_'))
    {
        return Err(format!("EIC code `{value}` contains invalid characters").into());
    }
    if !matches!(bytes[2], b'X' | b'Y' | b'Z' | b'W' | b'T' | b'V' | b'A') {
        return Err(format!("EIC code `{value}` has an unsupported object type").into());
    }
    Ok(())
}

fn validate_created_at(value: &str) -> Result<(), DynError> {
    if value.len() != 20
        || value.as_bytes().get(10) != Some(&b'T')
        || !value.ends_with('Z')
        || value.bytes().enumerate().any(|(index, byte)| match index {
            4 | 7 => byte != b'-',
            10 => byte != b'T',
            13 | 16 => byte != b':',
            19 => byte != b'Z',
            _ => !byte.is_ascii_digit(),
        })
    {
        return Err(format!("invalid EIC createdDateTime `{value}`").into());
    }
    validate_iso_date(&value[..10], "createdDateTime")
}

fn validate_iso_date(value: &str, label: &str) -> Result<(), DynError> {
    chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map(|_| ())
        .map_err(|error| format!("invalid EIC {label} `{value}`: {error}").into())
}

fn validate_sha256(value: &str) -> Result<(), DynError> {
    let decoded = hex::decode(value).map_err(|error| format!("invalid EIC SHA-256: {error}"))?;
    if decoded.len() != 32 || value != value.to_ascii_lowercase() {
        return Err("EIC SHA-256 must be 64 lowercase hexadecimal characters".into());
    }
    Ok(())
}

fn required_comment<'a>(
    comments: &'a BTreeMap<&str, &str>,
    name: &str,
) -> Result<&'a str, DynError> {
    comments
        .get(name)
        .copied()
        .ok_or_else(|| format!("EIC snapshot metadata `{name}` is missing").into())
}

fn parse_count(comments: &BTreeMap<&str, &str>, name: &str) -> Result<usize, DynError> {
    required_comment(comments, name)?
        .parse()
        .map_err(|error| format!("invalid EIC {name}: {error}").into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn xml_fixture() -> Vec<u8> {
        br#"<?xml version="1.0" encoding="utf-8"?>
<EIC_MarketDocument xmlns="urn:iec62325.351:tc57wg16:451-n:eicdocument:1:2">
  <createdDateTime>2026-08-13T01:15:13Z</createdDateTime>
  <EICCode_MarketDocument>
    <mRID>10X---ENTSOE---L</mRID>
    <docStatus><value>A05</value></docStatus>
    <attributeInstanceComponent.attribute>International</attributeInstanceComponent.attribute>
    <long_Names.name>European &amp; Network</long_Names.name>
    <display_Names.name>ENTSO-E</display_Names.name>
    <lastRequest_DateAndOrTime.date>2024-02-01</lastRequest_DateAndOrTime.date>
    <deactivationRequested_DateAndOrTime.date />
    <description>FREE_TEXT_SHOULD_NOT_PERSIST</description>
    <eICResponsible_MarketParticipant.mRID>10X1001A1001A450</eICResponsible_MarketParticipant.mRID>
    <Function_Names><name>Coordinator</name></Function_Names>
    <Function_Names><name>Network operator</name></Function_Names>
  </EICCode_MarketDocument>
  <EICCode_MarketDocument>
    <mRID>10X1001A1001A450</mRID>
    <docStatus><value>A03</value></docStatus>
    <attributeInstanceComponent.attribute>International</attributeInstanceComponent.attribute>
    <long_Names.name>Central Issuing Office</long_Names.name>
    <display_Names.name>CIO</display_Names.name>
    <lastRequest_DateAndOrTime.date>2020-01-01</lastRequest_DateAndOrTime.date>
    <deactivationRequested_DateAndOrTime.date>2026-01-01</deactivationRequested_DateAndOrTime.date>
    <description>CIO</description>
    <eICResponsible_MarketParticipant.mRID />
  </EICCode_MarketDocument>
</EIC_MarketDocument>"#
            .to_vec()
    }

    #[test]
    fn official_xml_projection_is_sorted_and_roundtrips() {
        let projection = project_eic_xml(&xml_fixture()).unwrap();
        assert_eq!(projection.created_at, "2026-08-13T01:15:13Z");
        assert_eq!(projection.records.len(), 2);
        assert_eq!(projection.active_record_count(), 1);

        let rendered = render_eic_snapshot(&projection).unwrap();
        assert!(!rendered.contains("European & Network"));
        assert!(!rendered.contains("FREE_TEXT_SHOULD_NOT_PERSIST"));
        assert_eq!(rendered.lines().nth(7), Some("eic_code\tstatus"));
        let (metadata, records) = parse_eic_snapshot(&rendered).unwrap();
        assert_eq!(metadata.record_count, 2);
        assert_eq!(metadata.active_record_count, 1);
        assert_eq!(records, projection.records);
    }

    #[test]
    fn snapshot_rejects_duplicate_codes_and_bad_counts() {
        let projection = project_eic_xml(&xml_fixture()).unwrap();
        let mut rendered = render_eic_snapshot(&projection).unwrap();

        let legacy_free_text_schema =
            rendered.replace("eic_code\tstatus\n", "eic_code\tstatus\tdescription\n");
        assert!(parse_eic_snapshot(&legacy_free_text_schema).is_err());

        rendered = rendered.replace("# active_record_count=1", "# active_record_count=2");
        assert!(parse_eic_snapshot(&rendered).is_err());

        let mut duplicate = projection;
        duplicate.records.push(duplicate.records[0].clone());
        duplicate
            .records
            .sort_by(|left, right| left.eic_code.cmp(&right.eic_code));
        assert!(render_eic_snapshot(&duplicate).is_err());
    }

    #[test]
    fn record_diff_reports_lifecycle_transitions() {
        let projection = project_eic_xml(&xml_fixture()).unwrap();
        let mut next = projection.records.clone();
        next[0].status = EicStatus::Inactive;
        next.push(EicRecord {
            eic_code: "10X1001A1001A55Y".to_owned(),
            status: EicStatus::Active,
        });
        next.sort_by(|left, right| left.eic_code.cmp(&right.eic_code));

        let diff = diff_records(&projection.records, &next);
        assert_eq!(diff.added, ["10X1001A1001A55Y"]);
        assert!(diff.removed.is_empty());
        assert_eq!(diff.changed.len(), 1);
        assert_eq!(diff.deactivated, [projection.records[0].eic_code.clone()]);
        assert!(diff.activated.is_empty());
    }

    #[test]
    fn cardinality_guard_blocks_changes_above_five_percent() {
        let at_limit = EicDiff {
            previous_count: 100,
            next_count: 105,
            added: Vec::new(),
            removed: Vec::new(),
            changed: Vec::new(),
            activated: Vec::new(),
            deactivated: Vec::new(),
        };
        assert!(!at_limit.has_strong_cardinality_change());

        let above_limit = EicDiff {
            next_count: 106,
            ..at_limit
        };
        assert!(above_limit.has_strong_cardinality_change());
    }
}
