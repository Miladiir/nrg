use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    ffi::OsString,
    fmt::Write as _,
    fs::{self, OpenOptions},
    io::{Read, Write as _},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use chrono::{Datelike, Local, NaiveDate};
use csv::{ReaderBuilder, StringRecord};
use encoding_rs::WINDOWS_1252;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

mod eic;
mod rule_catalogs;

const DOWNLOAD_PAGE: &str = "https://www.bundesbank.de/de/aufgaben/unbarer-zahlungsverkehr/serviceangebot/bankleitzahlen/download-bankleitzahlen-602592";
const RAW_HEADERS: [&str; 13] = [
    "Bankleitzahl",
    "Merkmal",
    "Bezeichnung",
    "PLZ",
    "Ort",
    "Kurzbezeichnung",
    "PAN",
    "BIC",
    "Prüfzifferberechnungsmethode",
    "Datensatznummer",
    "Änderungskennzeichen",
    "Bankleitzahllöschung",
    "Nachfolge-Bankleitzahl",
];
const COMPACT_HEADERS: [&str; 5] = [
    "bank_code",
    "bic",
    "change_marker",
    "deletion_flag",
    "successor_bank_code",
];
const SWIFT_IBAN_REGISTRY_NAME: &str = "swift_iban_registry";
const SWIFT_IBAN_REGISTRY_AUTHORITY: &str = "SWIFT, ISO 13616 Registration Authority";
const SWIFT_IBAN_REGISTRY_SOURCE_URL: &str = "https://www.swift.com/swift-resource/9606/download";
const SWIFT_IBAN_REGISTRY_COUNTRY_COUNT: usize = 89;
const DEFAULT_IBAN_REGISTRY_WARNING_MONTHS: i32 = 12;
const EIC_SOURCE_MAX_BYTES: usize = 96 * 1024 * 1024;

pub type DynError = Box<dyn Error + Send + Sync + 'static>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadCandidate {
    pub url: String,
    pub valid_from: NaiveDate,
    pub valid_to: NaiveDate,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CompactRecord {
    pub bank_code: String,
    pub bic: String,
    pub change_marker: String,
    pub deletion_flag: String,
    pub successor_bank_code: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Projection {
    pub source_url: String,
    pub source_sha256: String,
    pub valid_from: NaiveDate,
    pub valid_to: NaiveDate,
    pub synthetic_bank_code: String,
    pub records: Vec<CompactRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotMetadata {
    pub source_url: String,
    pub source_sha256: String,
    pub valid_from: NaiveDate,
    pub valid_to: NaiveDate,
    pub synthetic_bank_code: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RefreshMode {
    Write,
    DryRun,
    Check,
}

#[derive(Debug)]
struct RefreshOptions {
    mode: RefreshMode,
    as_of: NaiveDate,
    page_url: String,
    data_dir: PathBuf,
    iban_registry_import: Option<PathBuf>,
    bdew_identifiers_import: Option<PathBuf>,
    mastr_prefixes_import: Option<PathBuf>,
    refresh_eic_directory: bool,
    eic_source_sha256: Option<String>,
    accept_large_eic_change: bool,
}

#[derive(Debug)]
struct CheckOptions {
    as_of: NaiveDate,
    warning_days: i64,
    verify_source: bool,
    snapshot: Option<PathBuf>,
    iban_registry: Option<PathBuf>,
    iban_warning_months: i32,
    eic_snapshot: Option<PathBuf>,
    verify_eic_source: bool,
}

#[derive(Debug)]
struct CoreUpdate {
    path: PathBuf,
    current: String,
    updated: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IbanRegistrySnapshot {
    pub registry_name: String,
    pub registry_authority: String,
    pub release: u16,
    pub published: String,
    pub source_url: String,
    pub extracted_from_official_registry: bool,
    pub countries: Vec<IbanCountryRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IbanCountryRecord {
    pub country_code: String,
    pub country_name: String,
    pub sepa: bool,
    pub iban_length: usize,
    pub bban_length: usize,
    pub bban_structure: String,
    pub bank_identifier_position: Option<String>,
    pub bank_identifier_length: Option<String>,
    pub branch_identifier_position: Option<String>,
    pub branch_identifier_length: Option<String>,
    pub example_electronic: String,
    pub example_print: Option<String>,
    pub effective_date: Option<String>,
    pub last_update_date: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BbanSegment {
    length: usize,
    class: BbanCharacterClass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BbanCharacterClass {
    Numeric,
    Alphabetic,
    Alphanumeric,
}

pub fn run<I>(args: I) -> Result<(), DynError>
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    let Some(command) = args.next() else {
        print_help();
        return Ok(());
    };
    let rest: Vec<_> = args.collect();

    match command.as_str() {
        "refresh-reference-data" => refresh_reference_data(parse_refresh_options(&rest)?)?,
        "check-reference-data" => check_reference_data(parse_check_options(&rest)?)?,
        "help" | "--help" | "-h" => print_help(),
        unknown => return Err(format!("unknown xtask command `{unknown}`; use --help").into()),
    }
    Ok(())
}

fn print_help() {
    println!(
        "\
Reference-data maintenance

Usage:
  cargo run --manifest-path crates/xtask/Cargo.toml -- refresh-reference-data [OPTIONS]
  cargo run --manifest-path crates/xtask/Cargo.toml -- check-reference-data [OPTIONS]

refresh-reference-data options:
  --check                 Compare without writing; fail if an update is required
  --dry-run               Compare without writing; report changes successfully
  --refresh-iban-registry PATH
                          Review/import a locally prepared canonical registry JSON
  --refresh-bdew-identifiers PATH
                          Review/import a local BDEW formation-rule JSON
  --refresh-mastr-prefixes PATH
                          Review/import a local MaStR prefix/role JSON
  --refresh-eic-directory
                          Download and project the official ENTSO-E EIC bulk XML
  --eic-source-sha256 HASH
                          Required external trust anchor for every EIC refresh
  --accept-large-eic-change
                          Confirm an EIC record-count change greater than 5%
  --as-of YYYY-MM-DD      Select the file valid on this date (default: today)
  --page-url URL          Override the official Bundesbank download page
  --data-dir PATH         Override the repository data directory

check-reference-data options:
  --verify-source         Also verify recorded Bundesbank/BDEW/MaStR sources
  --as-of YYYY-MM-DD      Date used for expiry validation (default: today)
  --warning-days N        GitHub warning horizon (default: 30)
  --snapshot PATH         Check this BLZ projection instead of the core-embedded file
  --iban-registry PATH    Check this IBAN JSON instead of the core-embedded file
  --iban-warning-months N Warn when the registry publication is older than N months
                          (default: 12)
  --eic-snapshot PATH     Check this EIC projection instead of the embedded file
  --verify-eic-source     Download and re-project the official EIC bulk XML"
    );
}

fn parse_refresh_options(args: &[String]) -> Result<RefreshOptions, DynError> {
    let mut mode = RefreshMode::Write;
    let mut as_of = Local::now().date_naive();
    let mut page_url = DOWNLOAD_PAGE.to_owned();
    let mut data_dir = repository_root().join("data");
    let mut iban_registry_import = None;
    let mut bdew_identifiers_import = None;
    let mut mastr_prefixes_import = None;
    let mut refresh_eic_directory = false;
    let mut eic_source_sha256 = None;
    let mut accept_large_eic_change = false;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--check" => set_refresh_mode(&mut mode, RefreshMode::Check)?,
            "--dry-run" => set_refresh_mode(&mut mode, RefreshMode::DryRun)?,
            "--refresh-iban-registry" => {
                iban_registry_import = Some(PathBuf::from(take_value(
                    args,
                    &mut index,
                    "--refresh-iban-registry",
                )?));
            }
            "--refresh-bdew-identifiers" => {
                bdew_identifiers_import = Some(PathBuf::from(take_value(
                    args,
                    &mut index,
                    "--refresh-bdew-identifiers",
                )?));
            }
            "--refresh-mastr-prefixes" => {
                mastr_prefixes_import = Some(PathBuf::from(take_value(
                    args,
                    &mut index,
                    "--refresh-mastr-prefixes",
                )?));
            }
            "--refresh-eic-directory" => refresh_eic_directory = true,
            "--eic-source-sha256" => {
                eic_source_sha256 = Some(take_value(args, &mut index, "--eic-source-sha256")?)
            }
            "--accept-large-eic-change" => accept_large_eic_change = true,
            "--as-of" => as_of = parse_date(&take_value(args, &mut index, "--as-of")?)?,
            "--page-url" => page_url = take_value(args, &mut index, "--page-url")?,
            "--data-dir" => data_dir = PathBuf::from(take_value(args, &mut index, "--data-dir")?),
            "--help" | "-h" => {
                print_help();
                return Err("help requested".into());
            }
            option => return Err(format!("unknown refresh option `{option}`").into()),
        }
        index += 1;
    }

    let selected_imports = usize::from(iban_registry_import.is_some())
        + usize::from(bdew_identifiers_import.is_some())
        + usize::from(mastr_prefixes_import.is_some())
        + usize::from(refresh_eic_directory);
    if selected_imports > 1 {
        return Err("reference refresh/import selectors are mutually exclusive".into());
    }
    if refresh_eic_directory {
        let expected = eic_source_sha256
            .as_deref()
            .ok_or("--refresh-eic-directory requires --eic-source-sha256 HASH from an independently reviewed source")?;
        validate_expected_sha256(expected)?;
    } else if eic_source_sha256.is_some() || accept_large_eic_change {
        return Err(
            "--eic-source-sha256 and --accept-large-eic-change require --refresh-eic-directory"
                .into(),
        );
    }

    Ok(RefreshOptions {
        mode,
        as_of,
        page_url,
        data_dir,
        iban_registry_import,
        bdew_identifiers_import,
        mastr_prefixes_import,
        refresh_eic_directory,
        eic_source_sha256,
        accept_large_eic_change,
    })
}

fn parse_check_options(args: &[String]) -> Result<CheckOptions, DynError> {
    let mut as_of = Local::now().date_naive();
    let mut warning_days = 30;
    let mut verify_source = false;
    let mut snapshot = None;
    let mut iban_registry = None;
    let mut iban_warning_months = DEFAULT_IBAN_REGISTRY_WARNING_MONTHS;
    let mut eic_snapshot = None;
    let mut verify_eic_source = false;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--verify-source" => verify_source = true,
            "--as-of" => as_of = parse_date(&take_value(args, &mut index, "--as-of")?)?,
            "--warning-days" => {
                warning_days = take_value(args, &mut index, "--warning-days")?.parse()?;
                if warning_days < 0 {
                    return Err("--warning-days must not be negative".into());
                }
            }
            "--snapshot" => {
                snapshot = Some(PathBuf::from(take_value(args, &mut index, "--snapshot")?))
            }
            "--iban-registry" => {
                iban_registry = Some(PathBuf::from(take_value(
                    args,
                    &mut index,
                    "--iban-registry",
                )?))
            }
            "--iban-warning-months" => {
                iban_warning_months =
                    take_value(args, &mut index, "--iban-warning-months")?.parse()?;
                if iban_warning_months < 0 {
                    return Err("--iban-warning-months must not be negative".into());
                }
            }
            "--eic-snapshot" => {
                eic_snapshot = Some(PathBuf::from(take_value(
                    args,
                    &mut index,
                    "--eic-snapshot",
                )?))
            }
            "--verify-eic-source" => verify_eic_source = true,
            option => return Err(format!("unknown check option `{option}`").into()),
        }
        index += 1;
    }

    Ok(CheckOptions {
        as_of,
        warning_days,
        verify_source,
        snapshot,
        iban_registry,
        iban_warning_months,
        eic_snapshot,
        verify_eic_source,
    })
}

fn take_value(args: &[String], index: &mut usize, name: &str) -> Result<String, DynError> {
    *index += 1;
    args.get(*index)
        .cloned()
        .ok_or_else(|| format!("{name} requires a value").into())
}

fn set_refresh_mode(current: &mut RefreshMode, requested: RefreshMode) -> Result<(), DynError> {
    if *current != RefreshMode::Write && *current != requested {
        return Err("--check and --dry-run are mutually exclusive".into());
    }
    *current = requested;
    Ok(())
}

fn parse_date(value: &str) -> Result<NaiveDate, DynError> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|error| format!("invalid date `{value}`: {error}").into())
}

fn validate_expected_sha256(value: &str) -> Result<(), DynError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(
            "--eic-source-sha256 must contain exactly 64 lowercase hexadecimal characters".into(),
        );
    }
    Ok(())
}

fn verify_eic_source_trust_anchor(source: &[u8], expected: &str) -> Result<(), DynError> {
    validate_expected_sha256(expected)?;
    let actual = eic::sha256_hex(source);
    if actual != expected {
        return Err(format!(
            "EIC source SHA-256 mismatch: external trust anchor {expected}, downloaded {actual}"
        )
        .into());
    }
    Ok(())
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("xtask must live below crates/xtask")
        .to_owned()
}

fn refresh_reference_data(options: RefreshOptions) -> Result<(), DynError> {
    if let Some(import) = options.iban_registry_import.as_deref() {
        return refresh_iban_registry(&options, import);
    }
    if let Some(import) = options.bdew_identifiers_import.as_deref() {
        return rule_catalogs::refresh_bdew_identifiers(
            &repository_root(),
            &options.data_dir,
            import,
            rule_import_mode(options.mode),
        );
    }
    if let Some(import) = options.mastr_prefixes_import.as_deref() {
        return rule_catalogs::refresh_mastr_prefixes(
            &repository_root(),
            &options.data_dir,
            import,
            rule_import_mode(options.mode),
        );
    }
    if options.refresh_eic_directory {
        return refresh_eic_reference_data(&options);
    }

    refresh_bundesbank_reference_data(&options)?;
    let repo_root = repository_root();
    let (registry_path, core_source) = resolve_core_iban_registry(&repo_root)?;
    check_iban_registry_file(
        &registry_path,
        Some(&core_source),
        options.as_of,
        DEFAULT_IBAN_REGISTRY_WARNING_MONTHS,
    )?;
    check_embedded_eic_reference_data(&repo_root, None, false)?;
    rule_catalogs::check_rule_catalogs(&repo_root, options.as_of, false)?;
    Ok(())
}

fn rule_import_mode(mode: RefreshMode) -> rule_catalogs::ImportMode {
    match mode {
        RefreshMode::Write => rule_catalogs::ImportMode::Write,
        RefreshMode::DryRun => rule_catalogs::ImportMode::DryRun,
        RefreshMode::Check => rule_catalogs::ImportMode::Check,
    }
}

fn refresh_bundesbank_reference_data(options: &RefreshOptions) -> Result<(), DynError> {
    println!("Fetching Bundesbank download index: {}", options.page_url);
    let page_bytes = curl_download(&options.page_url)?;
    let page = String::from_utf8(page_bytes)
        .map_err(|error| format!("download page is not valid UTF-8: {error}"))?;
    let candidates = parse_download_candidates(&page, &options.page_url)?;
    let selected = select_current_candidate(&candidates, options.as_of)?;

    println!(
        "Selected {} (valid {} through {})",
        selected.url, selected.valid_from, selected.valid_to
    );
    let source = curl_download(&selected.url)?;
    let projection = project_source(
        &source,
        selected.url.clone(),
        selected.valid_from,
        selected.valid_to,
    )?;
    let rendered = render_projection(&projection);
    let filename = format!(
        "bundesbank_blz_{}_{}.csv",
        projection.valid_from, projection.valid_to
    );
    let destination = options.data_dir.join(filename);
    let repo_root = repository_root();
    let repository_data_dir = repo_root.join("data");
    let core_update = if options.data_dir == repository_data_dir {
        Some(prepare_core_update(&repo_root, &projection, &destination)?)
    } else {
        println!(
            "Custom data directory selected; id-core metadata will not be updated automatically."
        );
        None
    };
    let comparison = comparison_snapshot(&options.data_dir, &destination)?;
    let review = review_diff(comparison.as_deref(), &rendered)?;

    print_projection_metadata(&projection, &destination);
    print!("{review}");

    let unchanged = fs::read(&destination)
        .map(|existing| existing == rendered.as_bytes())
        .unwrap_or(false);
    let core_unchanged = core_update
        .as_ref()
        .is_none_or(|update| update.current == update.updated);
    match options.mode {
        RefreshMode::Check if !unchanged || !core_unchanged => {
            return Err(format!(
                "reference-data update required (projection current: {unchanged}, id-core metadata current: {core_unchanged})"
            )
            .into())
        }
        RefreshMode::Check => println!("Check passed; projection and id-core metadata are current."),
        RefreshMode::DryRun => {
            println!(
                "id-core metadata update required: {}",
                if core_unchanged { "no" } else { "yes" }
            );
            println!("Dry run complete; no files were written.");
        }
        RefreshMode::Write if unchanged && core_unchanged => {
            println!("Projection and id-core metadata are already current; no write needed.")
        }
        RefreshMode::Write => {
            if !unchanged {
                atomic_write(&destination, rendered.as_bytes())?;
                println!("Wrote {} atomically.", destination.display());
            }
            if let Some(update) = core_update.filter(|update| update.current != update.updated) {
                atomic_write(&update.path, update.updated.as_bytes())?;
                println!("Updated {} atomically.", update.path.display());
            }
        }
    }
    Ok(())
}

fn check_reference_data(options: CheckOptions) -> Result<(), DynError> {
    let repo_root = repository_root();
    let (snapshot, core_source) = match options.snapshot.as_ref() {
        Some(snapshot) => (snapshot.to_owned(), None),
        None => resolve_core_snapshot(&repo_root)?,
    };
    let text = fs::read_to_string(&snapshot)
        .map_err(|error| format!("cannot read {}: {error}", snapshot.display()))?;
    let (metadata, records) = parse_compact_snapshot(&text)?;

    if let Some(core_source) = core_source {
        verify_core_metadata(&core_source, &snapshot, &metadata)?;
    }

    let deterministic =
        find_unassigned_bank_code(records.iter().map(|record| record.bank_code.as_str()))?;
    if metadata.synthetic_bank_code != deterministic {
        return Err(format!(
            "snapshot declares synthetic BLZ {}, but deterministic selection is {deterministic}; refresh reference data and fixtures",
            metadata.synthetic_bank_code
        )
        .into());
    }

    if options.verify_source {
        println!("Verifying original source hash: {}", metadata.source_url);
        let source = curl_download(&metadata.source_url)?;
        let actual_hash = sha256_hex(&source);
        if actual_hash != metadata.source_sha256 {
            return Err(format!(
                "source SHA-256 mismatch: expected {}, got {actual_hash}",
                metadata.source_sha256
            )
            .into());
        }
        let projected = project_source(
            &source,
            metadata.source_url.clone(),
            metadata.valid_from,
            metadata.valid_to,
        )?;
        if projected.records != records {
            return Err("compact projection does not match the hashed source CSV".into());
        }
    }

    emit_expiry_status(
        metadata.valid_from,
        metadata.valid_to,
        options.as_of,
        options.warning_days,
    )?;
    println!(
        "Bundesbank reference-data check passed: {} records, SHA-256 {}, synthetic BLZ {}.",
        records.len(),
        metadata.source_sha256,
        deterministic
    );

    let (registry_path, iban_core_source) = match options.iban_registry.as_ref() {
        Some(registry) => (registry.to_owned(), None),
        None => {
            let (registry, source) = resolve_core_iban_registry(&repo_root)?;
            (registry, Some(source))
        }
    };
    check_iban_registry_file(
        &registry_path,
        iban_core_source.as_deref(),
        options.as_of,
        options.iban_warning_months,
    )?;
    check_embedded_eic_reference_data(
        &repo_root,
        options.eic_snapshot.as_deref(),
        options.verify_eic_source,
    )?;
    rule_catalogs::check_rule_catalogs(&repo_root, options.as_of, options.verify_source)?;
    Ok(())
}

fn refresh_eic_reference_data(options: &RefreshOptions) -> Result<(), DynError> {
    let expected_source_sha256 = options
        .eic_source_sha256
        .as_deref()
        .expect("EIC trust anchor is required by option parsing");
    println!("Fetching ENTSO-E EIC bulk XML: {}", eic::EIC_BULK_XML_URL);
    println!("Expected source SHA-256: {expected_source_sha256}");
    let source = curl_download_limited(
        eic::EIC_BULK_XML_URL,
        EIC_SOURCE_MAX_BYTES,
        "ENTSO-E EIC bulk XML",
    )?;
    verify_eic_source_trust_anchor(&source, expected_source_sha256)?;
    let projection = eic::project_eic_xml(&source)?;
    debug_assert_eq!(projection.source_sha256, expected_source_sha256);
    let rendered = eic::render_eic_snapshot(&projection)?;
    let projection_sha256 = eic::sha256_hex(rendered.as_bytes());
    let filename = format!("entso_e_eic_{}.tsv", projection.snapshot_date()?);
    let destination = options.data_dir.join(filename);
    let repo_root = repository_root();
    let repository_data_dir = repo_root.join("data");
    let comparison = eic_comparison_snapshot(&options.data_dir, &destination)?;
    let (review, diff) = review_eic_diff(comparison.as_deref(), &projection.records)?;
    let core_update = if options.data_dir == repository_data_dir {
        Some(prepare_eic_core_update(
            &repo_root,
            &projection,
            &destination,
            &projection_sha256,
        )?)
    } else {
        println!(
            "Custom data directory selected; id-core EIC metadata will not be updated automatically."
        );
        None
    };
    println!("Output: {}", destination.display());
    println!("Source createdDateTime: {}", projection.created_at);
    println!("Source SHA-256: {}", projection.source_sha256);
    println!("Projection SHA-256: {projection_sha256}");
    println!(
        "Records: {} active, {} inactive, {} total",
        projection.active_record_count(),
        projection.inactive_record_count(),
        projection.records.len()
    );
    print!("{review}");
    if diff.has_strong_cardinality_change() && !options.accept_large_eic_change {
        return Err(format!(
            "EIC record count changed from {} to {} (more than {}%); review the diff and rerun with --accept-large-eic-change to confirm",
            diff.previous_count,
            diff.next_count,
            eic::MAX_UNCONFIRMED_CARDINALITY_CHANGE_PERCENT
        )
        .into());
    }
    if diff.has_strong_cardinality_change() {
        println!(
            "Large EIC record-count change explicitly accepted: {} -> {}.",
            diff.previous_count, diff.next_count
        );
    }

    let unchanged = fs::read(&destination)
        .map(|existing| existing == rendered.as_bytes())
        .unwrap_or(false);
    let core_unchanged = core_update
        .as_ref()
        .is_none_or(|update| update.current == update.updated);
    match options.mode {
        RefreshMode::Check if !unchanged || !core_unchanged => {
            return Err(format!(
                "EIC reference-data update required (projection current: {unchanged}, id-core metadata current: {core_unchanged})"
            )
            .into())
        }
        RefreshMode::Check => println!("Check passed; EIC projection and metadata are current."),
        RefreshMode::DryRun => {
            println!(
                "id-core EIC metadata update required: {}",
                if core_unchanged { "no" } else { "yes" }
            );
            println!("Dry run complete; no files were written.");
        }
        RefreshMode::Write if unchanged && core_unchanged => {
            println!("EIC projection and metadata are already current; no write needed.")
        }
        RefreshMode::Write => {
            if !unchanged {
                atomic_write(&destination, rendered.as_bytes())?;
                println!("Wrote {} atomically.", destination.display());
            }
            if let Some(update) = core_update.filter(|update| update.current != update.updated) {
                atomic_write(&update.path, update.updated.as_bytes())?;
                println!("Updated {} atomically.", update.path.display());
            }
        }
    }
    Ok(())
}

fn check_embedded_eic_reference_data(
    repo_root: &Path,
    custom_snapshot: Option<&Path>,
    verify_source: bool,
) -> Result<(), DynError> {
    let (snapshot_path, core_source) = match custom_snapshot {
        Some(path) => (path.to_owned(), None),
        None => {
            let (path, source) = resolve_core_eic_snapshot(repo_root)?;
            (path, Some(source))
        }
    };
    let bytes = fs::read(&snapshot_path)
        .map_err(|error| format!("cannot read {}: {error}", snapshot_path.display()))?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|error| format!("{} is not UTF-8: {error}", snapshot_path.display()))?;
    let (metadata, records) = eic::parse_eic_snapshot(text)?;
    let projection_sha256 = eic::sha256_hex(&bytes);
    if let Some(core_source) = core_source.as_deref() {
        verify_eic_core_metadata(core_source, &snapshot_path, &metadata, &projection_sha256)?;
    }
    if verify_source {
        println!("Verifying ENTSO-E EIC source: {}", metadata.source_url);
        let source = curl_download_limited(
            &metadata.source_url,
            EIC_SOURCE_MAX_BYTES,
            "ENTSO-E EIC bulk XML",
        )?;
        let actual_source_hash = eic::sha256_hex(&source);
        if actual_source_hash != metadata.source_sha256 {
            return Err(format!(
                "EIC source SHA-256 mismatch: expected {}, got {actual_source_hash}",
                metadata.source_sha256
            )
            .into());
        }
        let projection = eic::project_eic_xml(&source)?;
        let rendered = eic::render_eic_snapshot(&projection)?;
        if rendered.as_bytes() != bytes {
            return Err("EIC projection does not match the hashed official bulk XML".into());
        }
    }
    println!(
        "ENTSO-E EIC reference-data check passed: {} records ({} active, {} inactive), created {}, projection SHA-256 {}.",
        records.len(),
        metadata.active_record_count,
        metadata.inactive_record_count,
        metadata.created_at,
        projection_sha256
    );
    Ok(())
}

fn resolve_core_eic_snapshot(repo_root: &Path) -> Result<(PathBuf, String), DynError> {
    let source_path = repo_root.join("crates/id-core/src/reference_data.rs");
    let source = fs::read_to_string(&source_path)
        .map_err(|error| format!("cannot read {}: {error}", source_path.display()))?;
    let name_position = source
        .find("ENTSO_E_EIC_DIRECTORY_TSV")
        .ok_or("cannot find embedded EIC snapshot constant in id-core")?;
    let include = extract_between(&source[name_position..], "include_str!(\"", "\")")
        .ok_or("cannot find embedded EIC snapshot include_str in id-core")?;
    let snapshot = source_path
        .parent()
        .expect("reference_data.rs has a parent")
        .join(include);
    Ok((snapshot, source))
}

fn prepare_eic_core_update(
    repo_root: &Path,
    projection: &eic::EicProjection,
    destination: &Path,
    projection_sha256: &str,
) -> Result<CoreUpdate, DynError> {
    let path = repo_root.join("crates/id-core/src/reference_data.rs");
    let current = fs::read_to_string(&path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("EIC projection destination has no UTF-8 filename")?;
    let mut updated = current.clone();
    for (name, value) in [
        ("ENTSO_E_EIC_DIRECTORY_NAME", eic::EIC_DIRECTORY_NAME),
        (
            "ENTSO_E_EIC_DIRECTORY_CREATED_AT",
            projection.created_at.as_str(),
        ),
        ("ENTSO_E_EIC_DIRECTORY_SOURCE_URL", eic::EIC_BULK_XML_URL),
        (
            "ENTSO_E_EIC_DIRECTORY_SOURCE_SHA256",
            projection.source_sha256.as_str(),
        ),
        ("ENTSO_E_EIC_DIRECTORY_PROJECTION_SHA256", projection_sha256),
    ] {
        replace_rust_string_constant(&mut updated, name, value)?;
    }
    for (name, value) in [
        (
            "ENTSO_E_EIC_DIRECTORY_RECORD_COUNT",
            projection.records.len() as u64,
        ),
        (
            "ENTSO_E_EIC_DIRECTORY_ACTIVE_RECORD_COUNT",
            projection.active_record_count() as u64,
        ),
        (
            "ENTSO_E_EIC_DIRECTORY_INACTIVE_RECORD_COUNT",
            projection.inactive_record_count() as u64,
        ),
    ] {
        replace_rust_unsigned_constant(&mut updated, name, value)?;
    }
    replace_named_include_path(
        &mut updated,
        "ENTSO_E_EIC_DIRECTORY_TSV",
        &format!("../../../data/{file_name}"),
    )?;
    Ok(CoreUpdate {
        path,
        current,
        updated,
    })
}

fn verify_eic_core_metadata(
    core_source: &str,
    snapshot_path: &Path,
    metadata: &eic::EicSnapshotMetadata,
    projection_sha256: &str,
) -> Result<(), DynError> {
    for (name, expected) in [
        ("ENTSO_E_EIC_DIRECTORY_NAME", eic::EIC_DIRECTORY_NAME),
        (
            "ENTSO_E_EIC_DIRECTORY_CREATED_AT",
            metadata.created_at.as_str(),
        ),
        (
            "ENTSO_E_EIC_DIRECTORY_SOURCE_URL",
            metadata.source_url.as_str(),
        ),
        (
            "ENTSO_E_EIC_DIRECTORY_SOURCE_SHA256",
            metadata.source_sha256.as_str(),
        ),
        ("ENTSO_E_EIC_DIRECTORY_PROJECTION_SHA256", projection_sha256),
    ] {
        let actual = extract_rust_string_constant(core_source, name)
            .ok_or_else(|| format!("id-core constant {name} is missing"))?;
        if actual != expected {
            return Err(
                format!("id-core {name} mismatch: expected `{expected}`, got `{actual}`").into(),
            );
        }
    }
    for (name, expected) in [
        (
            "ENTSO_E_EIC_DIRECTORY_RECORD_COUNT",
            metadata.record_count as u64,
        ),
        (
            "ENTSO_E_EIC_DIRECTORY_ACTIVE_RECORD_COUNT",
            metadata.active_record_count as u64,
        ),
        (
            "ENTSO_E_EIC_DIRECTORY_INACTIVE_RECORD_COUNT",
            metadata.inactive_record_count as u64,
        ),
    ] {
        let actual = extract_rust_unsigned_constant(core_source, name)
            .ok_or_else(|| format!("id-core constant {name} is missing or invalid"))?;
        if actual != expected {
            return Err(
                format!("id-core {name} mismatch: expected {expected}, got {actual}").into(),
            );
        }
    }
    let date = metadata
        .created_at
        .get(..10)
        .ok_or("EIC snapshot created_at is too short")?;
    let expected_filename = format!("entso_e_eic_{date}.tsv");
    let actual_filename = snapshot_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("embedded EIC snapshot has no UTF-8 filename")?;
    if actual_filename != expected_filename {
        return Err(format!(
            "embedded EIC filename `{actual_filename}` does not match `{expected_filename}`"
        )
        .into());
    }
    Ok(())
}

fn refresh_iban_registry(options: &RefreshOptions, import: &Path) -> Result<(), DynError> {
    if import.to_string_lossy().contains("://") {
        return Err("--refresh-iban-registry accepts a reviewed local JSON file, not a URL".into());
    }
    let input = fs::read(import).map_err(|error| {
        format!(
            "cannot read IBAN registry import {}: {error}",
            import.display()
        )
    })?;
    let snapshot = parse_iban_registry(&input)?;
    let rendered = render_iban_registry(&snapshot)?;
    let data_hash = sha256_hex(rendered.as_bytes());
    let destination = options
        .data_dir
        .join(format!("iban_registry_release_{}.json", snapshot.release));
    let repo_root = repository_root();
    let repository_data_dir = repo_root.join("data");
    let core_update = if options.data_dir == repository_data_dir {
        Some(prepare_iban_core_update(
            &repo_root,
            &snapshot,
            &destination,
            &data_hash,
        )?)
    } else {
        println!(
            "Custom data directory selected; id-core IBAN metadata will not be updated automatically."
        );
        None
    };
    let comparison = iban_registry_comparison(&options.data_dir, &destination)?;
    let review = review_iban_registry_diff(comparison.as_deref(), &snapshot)?;

    println!("IBAN registry import: {}", import.display());
    println!("Output: {}", destination.display());
    println!("Release: {} ({})", snapshot.release, snapshot.published);
    println!("Countries: {}", snapshot.countries.len());
    println!("Canonical JSON SHA-256: {data_hash}");
    print!("{review}");
    emit_iban_registry_age_status(
        &snapshot.published,
        options.as_of,
        DEFAULT_IBAN_REGISTRY_WARNING_MONTHS,
    )?;

    let unchanged = fs::read(&destination)
        .map(|existing| existing == rendered.as_bytes())
        .unwrap_or(false);
    let core_unchanged = core_update
        .as_ref()
        .is_none_or(|update| update.current == update.updated);
    match options.mode {
        RefreshMode::Check if !unchanged || !core_unchanged => {
            return Err(format!(
                "IBAN registry update required (projection current: {unchanged}, id-core metadata current: {core_unchanged})"
            )
            .into())
        }
        RefreshMode::Check => {
            println!("Check passed; IBAN projection and id-core metadata are current.")
        }
        RefreshMode::DryRun => {
            println!(
                "id-core IBAN metadata update required: {}",
                if core_unchanged { "no" } else { "yes" }
            );
            println!("Dry run complete; no files were written.");
        }
        RefreshMode::Write if unchanged && core_unchanged => {
            println!("IBAN projection and id-core metadata are already current; no write needed.")
        }
        RefreshMode::Write => {
            if !unchanged {
                atomic_write(&destination, rendered.as_bytes())?;
                println!("Wrote {} atomically.", destination.display());
            }
            if let Some(update) = core_update.filter(|update| update.current != update.updated) {
                atomic_write(&update.path, update.updated.as_bytes())?;
                println!("Updated {} atomically.", update.path.display());
            }
        }
    }
    Ok(())
}

pub fn parse_iban_registry(bytes: &[u8]) -> Result<IbanRegistrySnapshot, DynError> {
    let snapshot: IbanRegistrySnapshot = serde_json::from_slice(bytes)
        .map_err(|error| format!("invalid IBAN registry JSON schema: {error}"))?;
    validate_iban_registry(&snapshot)?;
    Ok(snapshot)
}

pub fn validate_iban_registry(snapshot: &IbanRegistrySnapshot) -> Result<(), DynError> {
    if snapshot.registry_name != SWIFT_IBAN_REGISTRY_NAME {
        return Err(format!("IBAN registry_name must be `{SWIFT_IBAN_REGISTRY_NAME}`").into());
    }
    if snapshot.registry_authority != SWIFT_IBAN_REGISTRY_AUTHORITY {
        return Err(
            format!("IBAN registry_authority must be `{SWIFT_IBAN_REGISTRY_AUTHORITY}`").into(),
        );
    }
    if snapshot.release == 0 {
        return Err("IBAN registry release must be greater than zero".into());
    }
    parse_published_month(&snapshot.published)?;
    if snapshot.source_url != SWIFT_IBAN_REGISTRY_SOURCE_URL
        || !snapshot.source_url.starts_with("https://")
    {
        return Err(format!(
            "IBAN registry source_url must be the official HTTPS SWIFT resource `{SWIFT_IBAN_REGISTRY_SOURCE_URL}`"
        )
        .into());
    }
    if !snapshot.extracted_from_official_registry {
        return Err("IBAN registry must declare extraction from the official registry".into());
    }
    if snapshot.countries.len() != SWIFT_IBAN_REGISTRY_COUNTRY_COUNT {
        return Err(format!(
            "IBAN registry release {} must contain exactly {SWIFT_IBAN_REGISTRY_COUNTRY_COUNT} countries, got {}",
            snapshot.release,
            snapshot.countries.len()
        )
        .into());
    }

    let mut previous_country: Option<&str> = None;
    for country in &snapshot.countries {
        validate_iban_country(country)?;
        if previous_country.is_some_and(|previous| previous >= country.country_code.as_str()) {
            return Err(format!(
                "IBAN countries must be unique and strictly sorted; found `{}` after `{}`",
                country.country_code,
                previous_country.unwrap_or_default()
            )
            .into());
        }
        previous_country = Some(&country.country_code);
    }
    Ok(())
}

fn validate_iban_country(country: &IbanCountryRecord) -> Result<(), DynError> {
    if country.country_code.len() != 2
        || !country
            .country_code
            .bytes()
            .all(|byte| byte.is_ascii_uppercase())
    {
        return Err(format!(
            "IBAN country_code {:?} must contain two uppercase ASCII letters",
            country.country_code
        )
        .into());
    }
    if country.country_name.trim().is_empty() {
        return Err(format!("{} country_name must not be empty", country.country_code).into());
    }
    if country.iban_length != country.bban_length + 4 {
        return Err(format!(
            "{} IBAN length {} must equal BBAN length {} plus four",
            country.country_code, country.iban_length, country.bban_length
        )
        .into());
    }
    if !(15..=34).contains(&country.iban_length) {
        return Err(format!(
            "{} IBAN length {} is outside the ISO 13616 range 15..=34",
            country.country_code, country.iban_length
        )
        .into());
    }

    let segments = parse_bban_structure(&country.bban_structure)?;
    let pattern_length = segments.iter().map(|segment| segment.length).sum::<usize>();
    if pattern_length != country.bban_length {
        return Err(format!(
            "{} BBAN structure {:?} describes {pattern_length} characters, expected {}",
            country.country_code, country.bban_structure, country.bban_length
        )
        .into());
    }

    let example = &country.example_electronic;
    if example.len() != country.iban_length
        || !example
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
        || !example.starts_with(&country.country_code)
        || !example
            .as_bytes()
            .get(2..4)
            .is_some_and(|digits| digits.iter().all(u8::is_ascii_digit))
    {
        return Err(format!(
            "{} official electronic example has invalid syntax or length",
            country.country_code
        )
        .into());
    }
    validate_bban_against_segments(&country.country_code, &example[4..], &segments)?;
    validate_registry_identifier(
        &country.country_code,
        "bank identifier",
        country.bank_identifier_position.as_deref(),
        country.bank_identifier_length.as_deref(),
        &example[4..],
    )?;
    validate_registry_identifier(
        &country.country_code,
        "branch identifier",
        country.branch_identifier_position.as_deref(),
        country.branch_identifier_length.as_deref(),
        &example[4..],
    )?;
    if !iban_mod97_is_valid(example)? {
        return Err(format!(
            "{} official electronic example fails MOD-97",
            country.country_code
        )
        .into());
    }
    if let Some(print) = &country.example_print {
        if print
            .chars()
            .any(|character| character.is_whitespace() && character != ' ')
            || print.replace(' ', "") != *example
        {
            return Err(format!(
                "{} official print example does not normalize to the electronic example",
                country.country_code
            )
            .into());
        }
    }
    validate_registry_date(
        &country.country_code,
        "effective_date",
        country.effective_date.as_deref(),
    )?;
    validate_registry_date(
        &country.country_code,
        "last_update_date",
        country.last_update_date.as_deref(),
    )?;
    Ok(())
}

fn validate_registry_identifier(
    country_code: &str,
    label: &str,
    position: Option<&str>,
    structure: Option<&str>,
    example_bban: &str,
) -> Result<(), DynError> {
    match (position, structure) {
        (None, None) => return Ok(()),
        (Some("N/A"), Some("N/A")) => return Ok(()),
        (Some(position), Some(structure)) if position != "N/A" && structure != "N/A" => {
            let (start, end) = parse_registry_position(position)?;
            if start == 0 || start > end || end > example_bban.len() {
                return Err(format!(
                    "{country_code} {label} position {position:?} is outside its BBAN"
                )
                .into());
            }
            let segments = parse_bban_structure(structure)?;
            let declared_length = segments.iter().map(|segment| segment.length).sum::<usize>();
            if declared_length != end - start + 1 {
                return Err(format!(
                    "{country_code} {label} structure {structure:?} does not match position {position:?}"
                )
                .into());
            }
            validate_bban_against_segments(country_code, &example_bban[start - 1..end], &segments)?;
            return Ok(());
        }
        _ => {}
    }
    Err(format!(
        "{country_code} {label} position and length metadata must both be absent, both N/A, or both concrete"
    )
    .into())
}

fn parse_registry_position(value: &str) -> Result<(usize, usize), DynError> {
    let (start, end) = value
        .split_once('-')
        .ok_or_else(|| format!("invalid IBAN registry position {value:?}"))?;
    if end.contains('-') {
        return Err(format!("invalid IBAN registry position {value:?}").into());
    }
    let start = start.parse()?;
    let end = end.parse()?;
    Ok((start, end))
}

fn parse_bban_structure(pattern: &str) -> Result<Vec<BbanSegment>, DynError> {
    let bytes = pattern.as_bytes();
    let mut segments = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let length_start = index;
        while bytes.get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
        if length_start == index || bytes.get(index) != Some(&b'!') {
            return Err(format!("invalid BBAN structure {pattern:?}").into());
        }
        let length: usize = pattern[length_start..index].parse()?;
        if length == 0 {
            return Err(
                format!("BBAN structure {pattern:?} contains a zero-length segment").into(),
            );
        }
        index += 1;
        let class = match bytes.get(index) {
            Some(b'n') => BbanCharacterClass::Numeric,
            Some(b'a') => BbanCharacterClass::Alphabetic,
            Some(b'c') => BbanCharacterClass::Alphanumeric,
            _ => return Err(format!("invalid BBAN structure {pattern:?}").into()),
        };
        index += 1;
        segments.push(BbanSegment { length, class });
    }
    if segments.is_empty() {
        return Err("BBAN structure must not be empty".into());
    }
    Ok(segments)
}

fn validate_bban_against_segments(
    country_code: &str,
    bban: &str,
    segments: &[BbanSegment],
) -> Result<(), DynError> {
    let mut offset = 0;
    for segment in segments {
        let end = offset + segment.length;
        let Some(value) = bban.as_bytes().get(offset..end) else {
            return Err(format!("{country_code} official example BBAN is too short").into());
        };
        let valid = match segment.class {
            BbanCharacterClass::Numeric => value.iter().all(u8::is_ascii_digit),
            BbanCharacterClass::Alphabetic => value.iter().all(u8::is_ascii_uppercase),
            BbanCharacterClass::Alphanumeric => value
                .iter()
                .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit()),
        };
        if !valid {
            return Err(format!(
                "{country_code} official example BBAN does not match its character structure at position {}",
                offset + 1
            )
            .into());
        }
        offset = end;
    }
    if offset != bban.len() {
        return Err(format!("{country_code} official example BBAN has trailing characters").into());
    }
    Ok(())
}

fn iban_mod97_is_valid(iban: &str) -> Result<bool, DynError> {
    if iban.len() < 4 || !iban.is_ascii() {
        return Ok(false);
    }
    let mut remainder = 0_u32;
    for byte in iban.as_bytes()[4..]
        .iter()
        .chain(iban.as_bytes()[..4].iter())
    {
        match byte {
            b'0'..=b'9' => remainder = (remainder * 10 + u32::from(byte - b'0')) % 97,
            b'A'..=b'Z' => {
                let value = u32::from(byte - b'A') + 10;
                remainder = (remainder * 10 + value / 10) % 97;
                remainder = (remainder * 10 + value % 10) % 97;
            }
            _ => return Err("IBAN MOD-97 input contains an invalid character".into()),
        }
    }
    Ok(remainder == 1)
}

fn validate_registry_date(
    country_code: &str,
    field: &str,
    value: Option<&str>,
) -> Result<(), DynError> {
    let Some(value) = value else {
        return Ok(());
    };
    let bytes = value.as_bytes();
    let valid_month = matches!(
        bytes.get(..3),
        Some(
            b"Jan"
                | b"Feb"
                | b"Mar"
                | b"Apr"
                | b"May"
                | b"Jun"
                | b"Jul"
                | b"Aug"
                | b"Sep"
                | b"Oct"
                | b"Nov"
                | b"Dec"
        )
    );
    if bytes.len() != 6
        || !valid_month
        || bytes.get(3) != Some(&b'-')
        || !bytes[4..].iter().all(u8::is_ascii_digit)
    {
        return Err(format!(
            "{country_code} {field} must use the official Mon-YY form, got {value:?}"
        )
        .into());
    }
    Ok(())
}

fn render_iban_registry(snapshot: &IbanRegistrySnapshot) -> Result<String, DynError> {
    let mut rendered = serde_json::to_string_pretty(snapshot)?;
    rendered.push('\n');
    Ok(rendered)
}

fn parse_published_month(value: &str) -> Result<NaiveDate, DynError> {
    let bytes = value.as_bytes();
    if bytes.len() != 7
        || bytes.get(4) != Some(&b'-')
        || !bytes[..4].iter().all(u8::is_ascii_digit)
        || !bytes[5..].iter().all(u8::is_ascii_digit)
    {
        return Err(
            format!("invalid IBAN registry publication month {value:?}; expected YYYY-MM").into(),
        );
    }
    NaiveDate::parse_from_str(&format!("{value}-01"), "%Y-%m-%d").map_err(|error| {
        format!("invalid IBAN registry publication month {value:?}: {error}").into()
    })
}

fn emit_iban_registry_age_status(
    published: &str,
    as_of: NaiveDate,
    warning_months: i32,
) -> Result<(), DynError> {
    let publication = parse_published_month(published)?;
    if publication > as_of {
        return Err(format!(
            "IBAN registry publication {published} is later than check date {as_of}"
        )
        .into());
    }
    let age_months = (as_of.year() - publication.year()) * 12
        + i32::try_from(as_of.month()).expect("month fits i32")
        - i32::try_from(publication.month()).expect("month fits i32");
    if age_months > warning_months {
        println!(
            "::warning::SWIFT IBAN registry release published {published} is {age_months} months old (warning threshold: {warning_months})"
        );
    }
    Ok(())
}

fn check_iban_registry_file(
    path: &Path,
    core_source: Option<&str>,
    as_of: NaiveDate,
    warning_months: i32,
) -> Result<IbanRegistrySnapshot, DynError> {
    let bytes = fs::read(path)
        .map_err(|error| format!("cannot read IBAN registry {}: {error}", path.display()))?;
    let snapshot = parse_iban_registry(&bytes)?;
    let canonical = render_iban_registry(&snapshot)?;
    if bytes != canonical.as_bytes() {
        return Err(format!(
            "IBAN registry {} is valid JSON but not in canonical checked-in form",
            path.display()
        )
        .into());
    }
    let data_hash = sha256_hex(&bytes);
    if let Some(core_source) = core_source {
        verify_iban_core_metadata(core_source, path, &snapshot, &data_hash)?;
    }
    emit_iban_registry_age_status(&snapshot.published, as_of, warning_months)?;
    println!(
        "IBAN registry check passed: release {}, {} countries, canonical SHA-256 {}.",
        snapshot.release,
        snapshot.countries.len(),
        data_hash
    );
    Ok(snapshot)
}

pub fn parse_download_candidates(
    html: &str,
    page_url: &str,
) -> Result<Vec<DownloadCandidate>, DynError> {
    let mut candidates = Vec::new();
    let mut rest = html;

    while let Some(href_offset) = rest.find("href=\"") {
        rest = &rest[href_offset + 6..];
        let Some(href_end) = rest.find('"') else {
            break;
        };
        let href = &rest[..href_end];
        rest = &rest[href_end + 1..];

        let path_without_query = href.split('?').next().unwrap_or(href);
        if !path_without_query.ends_with("-csv-data.csv") {
            continue;
        }
        let Some(anchor_end) = rest.find("</a>") else {
            return Err(format!("unterminated link for Bundesbank CSV `{href}`").into());
        };
        let anchor = &rest[..anchor_end];
        let (valid_from, valid_to) = parse_german_validity(anchor)?;
        candidates.push(DownloadCandidate {
            url: absolute_url(page_url, href)?,
            valid_from,
            valid_to,
        });
    }

    candidates.sort_by_key(|candidate| (candidate.valid_from, candidate.valid_to));
    candidates.dedup();
    if candidates.is_empty() {
        return Err("no uncompressed Bundesbank CSV download links found".into());
    }
    Ok(candidates)
}

fn parse_german_validity(anchor: &str) -> Result<(NaiveDate, NaiveDate), DynError> {
    let marker = "gültig vom ";
    let start = anchor
        .find(marker)
        .ok_or("CSV download link has no validity interval")?
        + marker.len();
    let date_text = anchor
        .get(start..)
        .ok_or("invalid validity interval position")?;
    let from_text = date_text
        .get(..10)
        .ok_or("valid-from date is shorter than DD.MM.YYYY")?;
    let separator = date_text
        .get(10..15)
        .ok_or("validity separator is missing")?;
    if separator != " bis " {
        return Err(format!("unexpected validity separator `{separator}`").into());
    }
    let to_text = date_text
        .get(15..25)
        .ok_or("valid-to date is shorter than DD.MM.YYYY")?;
    let from = NaiveDate::parse_from_str(from_text, "%d.%m.%Y")?;
    let to = NaiveDate::parse_from_str(to_text, "%d.%m.%Y")?;
    if from > to {
        return Err(format!("invalid validity interval {from} through {to}").into());
    }
    Ok((from, to))
}

fn absolute_url(page_url: &str, href: &str) -> Result<String, DynError> {
    if href.starts_with("https://") {
        return Ok(href.to_owned());
    }
    if href.starts_with("http://") {
        return Err("refusing an insecure HTTP source URL".into());
    }
    if href.starts_with('/') {
        let scheme_end = page_url
            .find("://")
            .ok_or_else(|| format!("page URL has no scheme: {page_url}"))?;
        let authority_end = page_url[scheme_end + 3..]
            .find('/')
            .map(|offset| scheme_end + 3 + offset)
            .unwrap_or(page_url.len());
        return Ok(format!("{}{}", &page_url[..authority_end], href));
    }
    Ok(format!(
        "{}/{}",
        page_url.rsplit_once('/').map_or(page_url, |(base, _)| base),
        href
    ))
}

pub fn select_current_candidate(
    candidates: &[DownloadCandidate],
    as_of: NaiveDate,
) -> Result<DownloadCandidate, DynError> {
    let matches: Vec<_> = candidates
        .iter()
        .filter(|candidate| candidate.valid_from <= as_of && as_of <= candidate.valid_to)
        .cloned()
        .collect();
    match matches.as_slice() {
        [selected] => Ok(selected.clone()),
        [] => {
            let known = candidates
                .iter()
                .map(|candidate| format!("{}..={}", candidate.valid_from, candidate.valid_to))
                .collect::<Vec<_>>()
                .join(", ");
            Err(
                format!("no Bundesbank CSV is valid on {as_of}; published intervals: {known}")
                    .into(),
            )
        }
        _ => Err(format!("multiple Bundesbank CSV files are valid on {as_of}").into()),
    }
}

pub fn project_source(
    source: &[u8],
    source_url: String,
    valid_from: NaiveDate,
    valid_to: NaiveDate,
) -> Result<Projection, DynError> {
    if valid_from > valid_to {
        return Err("source validity start is after its end".into());
    }
    let decoded = WINDOWS_1252
        .decode_without_bom_handling_and_without_replacement(source)
        .ok_or("source CSV is not valid ISO-8859-1/Windows-1252")?;
    let mut reader = ReaderBuilder::new()
        .delimiter(b';')
        .has_headers(true)
        .flexible(false)
        .from_reader(decoded.as_bytes());
    let headers = reader.headers()?.clone();
    validate_headers(&headers, &RAW_HEADERS, "Bundesbank source")?;

    let mut records = Vec::new();
    let mut seen_leading = BTreeSet::new();
    for (offset, row) in reader.records().enumerate() {
        let row_number = offset + 2;
        let row = row.map_err(|error| format!("invalid source CSV row {row_number}: {error}"))?;
        validate_source_row(&row, row_number)?;
        if &row[1] != "1" {
            continue;
        }
        let bank_code = row[0].to_owned();
        if !seen_leading.insert(bank_code.clone()) {
            return Err(format!("duplicate leading record for BLZ {bank_code}").into());
        }
        records.push(CompactRecord {
            bank_code,
            bic: row[7].to_owned(),
            change_marker: row[10].to_owned(),
            deletion_flag: row[11].to_owned(),
            successor_bank_code: row[12].to_owned(),
        });
    }
    if records.is_empty() {
        return Err("source CSV contains no bank-code-leading records".into());
    }
    records.sort_by(|left, right| left.bank_code.cmp(&right.bank_code));
    let synthetic_bank_code =
        find_unassigned_bank_code(records.iter().map(|record| record.bank_code.as_str()))?;

    Ok(Projection {
        source_url,
        source_sha256: sha256_hex(source),
        valid_from,
        valid_to,
        synthetic_bank_code,
        records,
    })
}

fn validate_source_row(row: &StringRecord, row_number: usize) -> Result<(), DynError> {
    validate_bank_code(&row[0], row_number, "Bankleitzahl")?;
    if !matches!(row.get(1), Some("1" | "2")) {
        return Err(format!("row {row_number}: Merkmal must be 1 or 2").into());
    }
    validate_bic(&row[7], row_number)?;
    if !matches!(row.get(10), Some("A" | "D" | "M" | "U")) {
        return Err(format!(
            "row {row_number}: unsupported Änderungskennzeichen `{}`",
            &row[10]
        )
        .into());
    }
    if !matches!(row.get(11), Some("0" | "1")) {
        return Err(format!("row {row_number}: Bankleitzahllöschung must be 0 or 1").into());
    }
    validate_bank_code(&row[12], row_number, "Nachfolge-Bankleitzahl")?;
    Ok(())
}

fn validate_bank_code(value: &str, row_number: usize, field: &str) -> Result<(), DynError> {
    if value.len() != 8 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(format!("row {row_number}: {field} must contain exactly eight digits").into());
    }
    Ok(())
}

fn validate_bic(value: &str, row_number: usize) -> Result<(), DynError> {
    if value.is_empty() {
        return Ok(());
    }
    if !matches!(value.len(), 8 | 11) {
        return Err(format!("row {row_number}: BIC must contain 8 or 11 characters").into());
    }
    let bytes = value.as_bytes();
    let is_alphanumeric = |byte: &u8| byte.is_ascii_uppercase() || byte.is_ascii_digit();
    if !bytes[..4].iter().all(is_alphanumeric)
        || &bytes[4..6] != b"DE"
        || !bytes[6..8].iter().all(is_alphanumeric)
        || (bytes.len() == 11 && !bytes[8..11].iter().all(is_alphanumeric))
    {
        return Err(
            format!("row {row_number}: BIC must follow the German ISO 9362 structure").into(),
        );
    }
    Ok(())
}

fn validate_headers<const N: usize>(
    actual: &StringRecord,
    expected: &[&str; N],
    description: &str,
) -> Result<(), DynError> {
    let actual_values: Vec<_> = actual.iter().collect();
    if actual_values != expected {
        return Err(format!(
            "{description} schema mismatch; expected {:?}, got {:?}",
            expected, actual_values
        )
        .into());
    }
    Ok(())
}

pub fn find_unassigned_bank_code<'a>(
    assigned: impl IntoIterator<Item = &'a str>,
) -> Result<String, DynError> {
    let assigned: BTreeSet<_> = assigned.into_iter().collect();
    for candidate in 0_u32..=99_999_999 {
        let candidate = format!("{candidate:08}");
        if !assigned.contains(candidate.as_str()) {
            return Ok(candidate);
        }
    }
    Err("all eight-digit bank codes are assigned".into())
}

pub fn render_projection(projection: &Projection) -> String {
    let mut output = String::new();
    writeln!(&mut output, "# source={}", projection.source_url).unwrap();
    writeln!(&mut output, "# source_sha256={}", projection.source_sha256).unwrap();
    writeln!(&mut output, "# valid_from={}", projection.valid_from).unwrap();
    writeln!(&mut output, "# valid_to={}", projection.valid_to).unwrap();
    writeln!(
        &mut output,
        "# synthetic_non_routable_bank_code={}",
        projection.synthetic_bank_code
    )
    .unwrap();
    writeln!(&mut output, "{}", COMPACT_HEADERS.join(",")).unwrap();
    for record in &projection.records {
        writeln!(
            &mut output,
            "{},{},{},{},{}",
            record.bank_code,
            record.bic,
            record.change_marker,
            record.deletion_flag,
            record.successor_bank_code
        )
        .unwrap();
    }
    output
}

pub fn parse_compact_snapshot(
    text: &str,
) -> Result<(SnapshotMetadata, Vec<CompactRecord>), DynError> {
    let comments: BTreeMap<_, _> = text
        .lines()
        .take_while(|line| line.starts_with('#'))
        .map(|line| {
            line.strip_prefix("# ")
                .and_then(|line| line.split_once('='))
                .ok_or_else(|| format!("invalid snapshot metadata line `{line}`"))
        })
        .collect::<Result<_, _>>()?;
    let source_url = required_comment(&comments, "source")?.to_owned();
    if !source_url.starts_with("https://") {
        return Err("snapshot source URL must use HTTPS".into());
    }
    let source_sha256 = required_comment(&comments, "source_sha256")?.to_owned();
    let decoded_hash = hex::decode(&source_sha256)
        .map_err(|error| format!("invalid source_sha256 metadata: {error}"))?;
    if decoded_hash.len() != 32 || source_sha256 != source_sha256.to_ascii_lowercase() {
        return Err("source_sha256 must be 64 lowercase hexadecimal characters".into());
    }
    let valid_from = parse_date(required_comment(&comments, "valid_from")?)?;
    let valid_to = parse_date(required_comment(&comments, "valid_to")?)?;
    if valid_from > valid_to {
        return Err("snapshot valid_from is after valid_to".into());
    }
    let synthetic_bank_code = required_comment(&comments, "synthetic_non_routable_bank_code")?;
    validate_bank_code(synthetic_bank_code, 0, "synthetic_non_routable_bank_code")?;
    let synthetic_bank_code = synthetic_bank_code.to_owned();

    let csv_start = text
        .lines()
        .position(|line| !line.starts_with('#'))
        .ok_or("snapshot contains metadata only")?;
    let csv_text = text.lines().skip(csv_start).collect::<Vec<_>>().join("\n");
    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(csv_text.as_bytes());
    let headers = reader.headers()?.clone();
    validate_headers(&headers, &COMPACT_HEADERS, "compact snapshot")?;
    let mut records = Vec::new();
    let mut bank_codes = BTreeSet::new();
    for (offset, row) in reader.records().enumerate() {
        let row_number = csv_start + offset + 2;
        let row = row.map_err(|error| format!("invalid compact row {row_number}: {error}"))?;
        validate_compact_row(&row, row_number)?;
        if !bank_codes.insert(row[0].to_owned()) {
            return Err(format!("duplicate compact BLZ {}", &row[0]).into());
        }
        records.push(CompactRecord {
            bank_code: row[0].to_owned(),
            bic: row[1].to_owned(),
            change_marker: row[2].to_owned(),
            deletion_flag: row[3].to_owned(),
            successor_bank_code: row[4].to_owned(),
        });
    }
    if records.is_empty() {
        return Err("compact snapshot contains no records".into());
    }
    if !records
        .windows(2)
        .all(|window| window[0].bank_code < window[1].bank_code)
    {
        return Err("compact snapshot must be strictly sorted by BLZ".into());
    }

    Ok((
        SnapshotMetadata {
            source_url,
            source_sha256,
            valid_from,
            valid_to,
            synthetic_bank_code,
        },
        records,
    ))
}

fn validate_compact_row(row: &StringRecord, row_number: usize) -> Result<(), DynError> {
    validate_bank_code(&row[0], row_number, "bank_code")?;
    validate_bic(&row[1], row_number)?;
    if !matches!(row.get(2), Some("A" | "D" | "M" | "U")) {
        return Err(format!("row {row_number}: invalid change_marker `{}`", &row[2]).into());
    }
    if !matches!(row.get(3), Some("0" | "1")) {
        return Err(format!("row {row_number}: deletion_flag must be 0 or 1").into());
    }
    validate_bank_code(&row[4], row_number, "successor_bank_code")
}

fn required_comment<'a>(
    comments: &'a BTreeMap<&str, &str>,
    name: &str,
) -> Result<&'a str, DynError> {
    comments
        .get(name)
        .copied()
        .ok_or_else(|| format!("snapshot metadata `{name}` is missing").into())
}

fn resolve_core_snapshot(repo_root: &Path) -> Result<(PathBuf, Option<String>), DynError> {
    let source_path = repo_root.join("crates/id-core/src/reference_data.rs");
    let source = fs::read_to_string(&source_path)
        .map_err(|error| format!("cannot read {}: {error}", source_path.display()))?;
    let include = extract_between(&source, "include_str!(\"", "\")")
        .ok_or("cannot find embedded Bundesbank snapshot include_str in id-core")?;
    let snapshot = source_path
        .parent()
        .expect("reference_data.rs has a parent")
        .join(include);
    Ok((snapshot, Some(source)))
}

fn resolve_core_iban_registry(repo_root: &Path) -> Result<(PathBuf, String), DynError> {
    let source_path =
        repo_root.join("crates/id-core/src/identifiers/payments/international_iban.rs");
    let source = fs::read_to_string(&source_path)
        .map_err(|error| format!("cannot read {}: {error}", source_path.display()))?;
    let include = extract_between(
        &source,
        "const IBAN_REGISTRY_JSON: &str = include_str!(\"",
        "\");",
    )
    .ok_or("cannot find embedded IBAN registry include_str in id-core")?;
    let snapshot = source_path
        .parent()
        .expect("international_iban.rs has a parent")
        .join(include);
    Ok((snapshot, source))
}

fn prepare_iban_core_update(
    repo_root: &Path,
    snapshot: &IbanRegistrySnapshot,
    destination: &Path,
    data_hash: &str,
) -> Result<CoreUpdate, DynError> {
    let path = repo_root.join("crates/id-core/src/identifiers/payments/international_iban.rs");
    let current = fs::read_to_string(&path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("IBAN registry destination has no UTF-8 filename")?;
    let mut updated = current.clone();
    replace_rust_unsigned_constant(
        &mut updated,
        "IBAN_REGISTRY_RELEASE",
        u64::from(snapshot.release),
    )?;
    for (name, value) in [
        ("IBAN_REGISTRY_NAME", snapshot.registry_name.as_str()),
        ("IBAN_REGISTRY_PUBLISHED", snapshot.published.as_str()),
        ("IBAN_REGISTRY_SOURCE_URL", snapshot.source_url.as_str()),
        ("IBAN_REGISTRY_DATA_SHA256", data_hash),
    ] {
        replace_rust_string_constant(&mut updated, name, value)?;
    }
    replace_include_path(&mut updated, &format!("../../../../../data/{file_name}"))?;
    Ok(CoreUpdate {
        path,
        current,
        updated,
    })
}

fn prepare_core_update(
    repo_root: &Path,
    projection: &Projection,
    destination: &Path,
) -> Result<CoreUpdate, DynError> {
    let path = repo_root.join("crates/id-core/src/reference_data.rs");
    let current = fs::read_to_string(&path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("projection destination has no UTF-8 filename")?;
    let mut updated = current.clone();
    for (name, value) in [
        (
            "BUNDESBANK_BLZ_VALID_FROM",
            projection.valid_from.to_string(),
        ),
        ("BUNDESBANK_BLZ_VALID_TO", projection.valid_to.to_string()),
        (
            "BUNDESBANK_BLZ_SOURCE_SHA256",
            projection.source_sha256.clone(),
        ),
        ("BUNDESBANK_BLZ_SOURCE_URL", projection.source_url.clone()),
        (
            "BUNDESBANK_BLZ_SYNTHETIC_BANK_CODE",
            projection.synthetic_bank_code.clone(),
        ),
    ] {
        replace_rust_string_constant(&mut updated, name, &value)?;
    }
    replace_include_path(&mut updated, &format!("../../../data/{file_name}"))?;
    Ok(CoreUpdate {
        path,
        current,
        updated,
    })
}

fn replace_rust_string_constant(
    source: &mut String,
    name: &str,
    replacement: &str,
) -> Result<(), DynError> {
    let name_position = source
        .find(name)
        .ok_or_else(|| format!("id-core constant {name} is missing"))?;
    let assignment = source[name_position..]
        .find('=')
        .map(|offset| name_position + offset)
        .ok_or_else(|| format!("id-core constant {name} has no assignment"))?;
    let value_start = source[assignment..]
        .find('"')
        .map(|offset| assignment + offset + 1)
        .ok_or_else(|| format!("id-core constant {name} has no string value"))?;
    let value_end = source[value_start..]
        .find('"')
        .map(|offset| value_start + offset)
        .ok_or_else(|| format!("id-core constant {name} has an unterminated string value"))?;
    source.replace_range(value_start..value_end, replacement);
    Ok(())
}

fn replace_rust_unsigned_constant(
    source: &mut String,
    name: &str,
    replacement: u64,
) -> Result<(), DynError> {
    let name_position = source
        .find(name)
        .ok_or_else(|| format!("id-core constant {name} is missing"))?;
    let assignment = source[name_position..]
        .find('=')
        .map(|offset| name_position + offset + 1)
        .ok_or_else(|| format!("id-core constant {name} has no assignment"))?;
    let value_start = source[assignment..]
        .find(|character: char| !character.is_ascii_whitespace())
        .map(|offset| assignment + offset)
        .ok_or_else(|| format!("id-core constant {name} has no value"))?;
    let value_end = source[value_start..]
        .find(|character: char| !character.is_ascii_digit())
        .map(|offset| value_start + offset)
        .ok_or_else(|| format!("id-core constant {name} has an unterminated value"))?;
    if value_start == value_end {
        return Err(format!("id-core constant {name} is not an unsigned integer").into());
    }
    source.replace_range(value_start..value_end, &replacement.to_string());
    Ok(())
}

fn replace_include_path(source: &mut String, replacement: &str) -> Result<(), DynError> {
    let prefix = "include_str!(\"";
    let start = source
        .find(prefix)
        .map(|offset| offset + prefix.len())
        .ok_or("cannot find id-core snapshot include_str")?;
    let end = source[start..]
        .find("\")")
        .map(|offset| start + offset)
        .ok_or("id-core snapshot include_str is unterminated")?;
    source.replace_range(start..end, replacement);
    Ok(())
}

fn replace_named_include_path(
    source: &mut String,
    constant_name: &str,
    replacement: &str,
) -> Result<(), DynError> {
    let constant_position = source
        .find(constant_name)
        .ok_or_else(|| format!("id-core constant {constant_name} is missing"))?;
    let prefix = "include_str!(\"";
    let start = source[constant_position..]
        .find(prefix)
        .map(|offset| constant_position + offset + prefix.len())
        .ok_or_else(|| format!("id-core constant {constant_name} has no include_str"))?;
    let end = source[start..]
        .find("\")")
        .map(|offset| start + offset)
        .ok_or_else(|| format!("id-core constant {constant_name} include_str is unterminated"))?;
    source.replace_range(start..end, replacement);
    Ok(())
}

fn verify_core_metadata(
    core_source: &str,
    snapshot: &Path,
    metadata: &SnapshotMetadata,
) -> Result<(), DynError> {
    let checks = vec![
        ("BUNDESBANK_BLZ_NAME", "bundesbank_blz".to_owned()),
        ("BUNDESBANK_BLZ_VALID_FROM", metadata.valid_from.to_string()),
        ("BUNDESBANK_BLZ_VALID_TO", metadata.valid_to.to_string()),
        (
            "BUNDESBANK_BLZ_SOURCE_SHA256",
            metadata.source_sha256.clone(),
        ),
        ("BUNDESBANK_BLZ_SOURCE_URL", metadata.source_url.clone()),
        (
            "BUNDESBANK_BLZ_SYNTHETIC_BANK_CODE",
            metadata.synthetic_bank_code.clone(),
        ),
    ];
    for (name, expected) in checks {
        let actual = extract_rust_string_constant(core_source, name)
            .ok_or_else(|| format!("id-core constant {name} is missing"))?;
        if actual != expected {
            return Err(
                format!("id-core {name} mismatch: expected `{expected}`, got `{actual}`").into(),
            );
        }
    }

    let file_name = snapshot
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("embedded snapshot has no UTF-8 filename")?;
    let expected_file = format!(
        "bundesbank_blz_{}_{}.csv",
        metadata.valid_from, metadata.valid_to
    );
    if file_name != expected_file {
        return Err(format!(
            "embedded filename `{file_name}` does not match metadata `{expected_file}`"
        )
        .into());
    }
    Ok(())
}

fn verify_iban_core_metadata(
    core_source: &str,
    snapshot_path: &Path,
    snapshot: &IbanRegistrySnapshot,
    data_hash: &str,
) -> Result<(), DynError> {
    let checks = [
        ("IBAN_REGISTRY_NAME", snapshot.registry_name.as_str()),
        ("IBAN_REGISTRY_PUBLISHED", snapshot.published.as_str()),
        ("IBAN_REGISTRY_SOURCE_URL", snapshot.source_url.as_str()),
        ("IBAN_REGISTRY_DATA_SHA256", data_hash),
    ];
    for (name, expected) in checks {
        let actual = extract_rust_string_constant(core_source, name)
            .ok_or_else(|| format!("id-core constant {name} is missing"))?;
        if actual != expected {
            return Err(
                format!("id-core {name} mismatch: expected `{expected}`, got `{actual}`").into(),
            );
        }
    }
    let release = extract_rust_unsigned_constant(core_source, "IBAN_REGISTRY_RELEASE")
        .ok_or("id-core constant IBAN_REGISTRY_RELEASE is missing or invalid")?;
    if release != u64::from(snapshot.release) {
        return Err(format!(
            "id-core IBAN_REGISTRY_RELEASE mismatch: expected {}, got {release}",
            snapshot.release
        )
        .into());
    }
    let file_name = snapshot_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("embedded IBAN registry has no UTF-8 filename")?;
    let expected_file = format!("iban_registry_release_{}.json", snapshot.release);
    if file_name != expected_file {
        return Err(format!(
            "embedded IBAN registry filename `{file_name}` does not match `{expected_file}`"
        )
        .into());
    }
    Ok(())
}

fn extract_rust_string_constant<'a>(source: &'a str, name: &str) -> Option<&'a str> {
    let name_position = source.find(name)?;
    let assignment = source[name_position..].find('=')? + name_position;
    let value_start = source[assignment..].find('"')? + assignment + 1;
    let value_end = source[value_start..].find('"')? + value_start;
    Some(&source[value_start..value_end])
}

fn extract_rust_unsigned_constant(source: &str, name: &str) -> Option<u64> {
    let name_position = source.find(name)?;
    let assignment = source[name_position..].find('=')? + name_position + 1;
    let value = source[assignment..].trim_start();
    let end = value
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(value.len());
    value[..end].parse().ok()
}

fn extract_between<'a>(source: &'a str, prefix: &str, suffix: &str) -> Option<&'a str> {
    let start = source.find(prefix)? + prefix.len();
    let end = source[start..].find(suffix)? + start;
    Some(&source[start..end])
}

fn emit_expiry_status(
    valid_from: NaiveDate,
    valid_to: NaiveDate,
    as_of: NaiveDate,
    warning_days: i64,
) -> Result<(), DynError> {
    if as_of < valid_from {
        return Err(
            format!("snapshot is not valid until {valid_from}; check date is {as_of}").into(),
        );
    }
    let remaining = (valid_to - as_of).num_days();
    if remaining < 0 {
        println!("::warning::Bundesbank BLZ snapshot expired on {valid_to}");
        return Err(format!("snapshot expired on {valid_to}").into());
    }
    if remaining <= warning_days {
        println!(
            "::warning::Bundesbank BLZ snapshot expires on {valid_to} ({remaining} days remaining)"
        );
    }
    Ok(())
}

fn curl_download(url: &str) -> Result<Vec<u8>, DynError> {
    validate_download_url(url)?;
    let output = Command::new("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--proto",
            "=https,file",
            "--proto-redir",
            "=https",
            "--max-time",
            "60",
            "--user-agent",
            "nrg-reference-data-xtask/1",
            url,
        ])
        .output()
        .map_err(|error| format!("failed to execute curl: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "curl failed for {url} with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into());
    }
    Ok(output.stdout)
}

fn curl_download_limited(url: &str, max_bytes: usize, label: &str) -> Result<Vec<u8>, DynError> {
    validate_download_url(url)?;
    let max_bytes_argument = max_bytes.to_string();
    let mut child = Command::new("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--proto",
            "=https,file",
            "--proto-redir",
            "=https",
            "--max-time",
            "60",
            "--max-filesize",
            &max_bytes_argument,
            "--user-agent",
            "nrg-reference-data-xtask/1",
            url,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("failed to execute curl: {error}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or("curl stdout pipe was not available")?;
    let bytes = match read_to_end_limited(stdout, max_bytes, label) {
        Ok(bytes) => bytes,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    };
    let output = child
        .wait_with_output()
        .map_err(|error| format!("failed to wait for curl: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "curl failed for {url} with status {} (hard download limit: {max_bytes} bytes): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into());
    }
    Ok(bytes)
}

fn read_to_end_limited(
    reader: impl Read,
    max_bytes: usize,
    label: &str,
) -> Result<Vec<u8>, DynError> {
    let read_limit = max_bytes
        .checked_add(1)
        .ok_or("download-size limit is too large")?;
    let mut bytes = Vec::new();
    reader
        .take(u64::try_from(read_limit)?)
        .read_to_end(&mut bytes)?;
    if bytes.len() > max_bytes {
        return Err(
            format!("{label} exceeded the hard download limit of {max_bytes} bytes").into(),
        );
    }
    Ok(bytes)
}

fn validate_download_url(url: &str) -> Result<(), DynError> {
    if !url.starts_with("https://") && !url.starts_with("file://") {
        return Err(format!("refusing unsupported download URL `{url}`").into());
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn comparison_snapshot(data_dir: &Path, destination: &Path) -> Result<Option<PathBuf>, DynError> {
    if destination.is_file() {
        return Ok(Some(destination.to_owned()));
    }
    if !data_dir.exists() {
        return Ok(None);
    }
    let mut candidates = fs::read_dir(data_dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("bundesbank_blz_") && name.ends_with(".csv"))
        })
        .collect::<Vec<_>>();
    candidates.sort();
    Ok(candidates.pop())
}

fn iban_registry_comparison(
    data_dir: &Path,
    destination: &Path,
) -> Result<Option<PathBuf>, DynError> {
    if destination.is_file() {
        return Ok(Some(destination.to_owned()));
    }
    if !data_dir.exists() {
        return Ok(None);
    }
    let candidates = fs::read_dir(data_dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter_map(|path| {
            let name = path.file_name()?.to_str()?;
            let release = name
                .strip_prefix("iban_registry_release_")?
                .strip_suffix(".json")?
                .parse::<u16>()
                .ok()?;
            Some((release, path))
        })
        .collect::<Vec<_>>();
    Ok(candidates
        .into_iter()
        .max_by_key(|(release, _)| *release)
        .map(|(_, path)| path))
}

fn eic_comparison_snapshot(
    data_dir: &Path,
    destination: &Path,
) -> Result<Option<PathBuf>, DynError> {
    if destination.is_file() {
        return Ok(Some(destination.to_owned()));
    }
    if !data_dir.exists() {
        return Ok(None);
    }
    let mut candidates = fs::read_dir(data_dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("entso_e_eic_") && name.ends_with(".tsv"))
        })
        .collect::<Vec<_>>();
    candidates.sort();
    Ok(candidates.pop())
}

fn review_eic_diff(
    previous: Option<&Path>,
    next: &[eic::EicRecord],
) -> Result<(String, eic::EicDiff), DynError> {
    let Some(previous) = previous else {
        let diff = eic::diff_records(&[], next);
        let mut output = format!(
            "Review diff: no existing EIC snapshot; +{} records (0 -> {}).\n",
            diff.added.len(),
            diff.next_count
        );
        append_eic_review_codes(&mut output, "added", &diff.added);
        return Ok((output, diff));
    };
    let previous_text = fs::read_to_string(previous)
        .map_err(|error| format!("cannot read {}: {error}", previous.display()))?;
    let (previous_metadata, previous_records) = eic::parse_eic_snapshot(&previous_text)?;
    let diff = eic::diff_records(&previous_records, next);
    let mut output = format!(
        "Review diff against {} (created {}): +{} -{} ~{} records; total {} -> {}.\nLifecycle transitions: {} activated, {} deactivated.\n",
        previous.display(),
        previous_metadata.created_at,
        diff.added.len(),
        diff.removed.len(),
        diff.changed.len(),
        diff.previous_count,
        diff.next_count,
        diff.activated.len(),
        diff.deactivated.len(),
    );
    append_eic_review_codes(&mut output, "added", &diff.added);
    append_eic_review_codes(&mut output, "removed", &diff.removed);
    append_eic_review_codes(&mut output, "changed", &diff.changed);
    append_eic_review_codes(&mut output, "activated", &diff.activated);
    append_eic_review_codes(&mut output, "deactivated", &diff.deactivated);
    Ok((output, diff))
}

fn append_eic_review_codes(output: &mut String, label: &str, codes: &[String]) {
    if codes.is_empty() {
        return;
    }
    let shown = codes
        .iter()
        .take(50)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(", ");
    let suffix = if codes.len() > 50 {
        format!(" (and {} more)", codes.len() - 50)
    } else {
        String::new()
    };
    let _ = writeln!(output, "  {label}: {shown}{suffix}");
}

fn review_iban_registry_diff(
    previous: Option<&Path>,
    next: &IbanRegistrySnapshot,
) -> Result<String, DynError> {
    let Some(previous) = previous else {
        return Ok(format!(
            "Review diff: new IBAN registry with {} countries.\n",
            next.countries.len()
        ));
    };
    let previous_bytes = fs::read(previous)?;
    let previous_snapshot = parse_iban_registry(&previous_bytes)?;
    let previous_map: BTreeMap<_, _> = previous_snapshot
        .countries
        .iter()
        .map(|country| (country.country_code.as_str(), country))
        .collect();
    let next_map: BTreeMap<_, _> = next
        .countries
        .iter()
        .map(|country| (country.country_code.as_str(), country))
        .collect();
    let added = next_map
        .keys()
        .filter(|country| !previous_map.contains_key(*country))
        .copied()
        .collect::<Vec<_>>();
    let removed = previous_map
        .keys()
        .filter(|country| !next_map.contains_key(*country))
        .copied()
        .collect::<Vec<_>>();
    let changed = next_map
        .keys()
        .filter(|country| {
            previous_map
                .get(*country)
                .is_some_and(|old| Some(old) != next_map.get(*country))
        })
        .copied()
        .collect::<Vec<_>>();
    let mut output = format!(
        "Review diff against {} (release {}, published {}): +{} -{} ~{} countries.\n",
        previous.display(),
        previous_snapshot.release,
        previous_snapshot.published,
        added.len(),
        removed.len(),
        changed.len()
    );
    append_review_values(&mut output, "added", &added);
    append_review_values(&mut output, "removed", &removed);
    append_review_values(&mut output, "changed", &changed);
    Ok(output)
}

fn append_review_values(output: &mut String, label: &str, values: &[&str]) {
    if values.is_empty() {
        return;
    }
    let _ = writeln!(output, "  {label}: {}", values.join(", "));
}

fn review_diff(previous: Option<&Path>, rendered: &str) -> Result<String, DynError> {
    let (_, next_records) = parse_compact_snapshot(rendered)?;
    let Some(previous) = previous else {
        return Ok(format!(
            "Review diff: new snapshot with {} BLZ records.\n",
            next_records.len()
        ));
    };
    let previous_text = fs::read_to_string(previous)?;
    let (previous_metadata, previous_records) = parse_compact_snapshot(&previous_text)?;
    let previous_map: BTreeMap<_, _> = previous_records
        .into_iter()
        .map(|record| (record.bank_code.clone(), record))
        .collect();
    let next_map: BTreeMap<_, _> = next_records
        .into_iter()
        .map(|record| (record.bank_code.clone(), record))
        .collect();
    let added: Vec<_> = next_map
        .keys()
        .filter(|key| !previous_map.contains_key(*key))
        .collect();
    let removed: Vec<_> = previous_map
        .keys()
        .filter(|key| !next_map.contains_key(*key))
        .collect();
    let changed: Vec<_> = next_map
        .keys()
        .filter(|key| {
            previous_map
                .get(*key)
                .is_some_and(|old| Some(old) != next_map.get(*key))
        })
        .collect();

    let mut output = format!(
        "Review diff against {} (valid through {}): +{} -{} ~{} BLZ records.\n",
        previous.display(),
        previous_metadata.valid_to,
        added.len(),
        removed.len(),
        changed.len()
    );
    append_review_codes(&mut output, "added", &added);
    append_review_codes(&mut output, "removed", &removed);
    append_review_codes(&mut output, "changed", &changed);
    Ok(output)
}

fn append_review_codes(output: &mut String, label: &str, codes: &[&String]) {
    if codes.is_empty() {
        return;
    }
    let shown = codes
        .iter()
        .take(50)
        .map(|code| code.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let suffix = if codes.len() > 50 {
        format!(" (and {} more)", codes.len() - 50)
    } else {
        String::new()
    };
    let _ = writeln!(output, "  {label}: {shown}{suffix}");
}

fn print_projection_metadata(projection: &Projection, destination: &Path) {
    println!("Output: {}", destination.display());
    println!("Records: {}", projection.records.len());
    println!("Source SHA-256: {}", projection.source_sha256);
    println!("Valid from: {}", projection.valid_from);
    println!("Valid to: {}", projection.valid_to);
    println!(
        "Synthetic non-routable BLZ: {} (verified absent)",
        projection.synthetic_bank_code
    );
}

fn atomic_write(path: &Path, contents: &[u8]) -> Result<(), DynError> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", path.display()))?;
    fs::create_dir_all(parent)?;
    let file_name = path
        .file_name()
        .ok_or_else(|| format!("{} has no filename", path.display()))?;
    let mut temp_name = OsString::from(".");
    temp_name.push(file_name);
    temp_name.push(format!(".tmp.{}", std::process::id()));
    let temp = parent.join(temp_name);
    let result = (|| -> Result<(), DynError> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(contents)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(value: &str) -> NaiveDate {
        NaiveDate::parse_from_str(value, "%Y-%m-%d").unwrap()
    }

    fn raw_fixture() -> Vec<u8> {
        let text = concat!(
            "Bankleitzahl;Merkmal;Bezeichnung;PLZ;Ort;Kurzbezeichnung;PAN;BIC;Prüfzifferberechnungsmethode;Datensatznummer;Änderungskennzeichen;Bankleitzahllöschung;Nachfolge-Bankleitzahl\r\n",
            "\"10000000\";\"1\";\"Bundesbank\";\"10591\";\"Berlin\";\"BBk Berlin\";\"20100\";\"MARKDEF1100\";\"09\";\"011380\";\"U\";\"0\";\"00000000\"\r\n",
            "\"10000000\";\"2\";\"Filiale\";\"10115\";\"Berlin\";\"Filiale\";;\"MARKDEF1100\";\"09\";\"011381\";\"U\";\"0\";\"00000000\"\r\n",
            "\"20000000\";\"1\";\"Testbank\";\"20095\";\"Hamburg\";\"Testbank\";;\"TESTDEHHXXX\";\"00\";\"011382\";\"M\";\"1\";\"10000000\"\r\n",
        );
        text.chars()
            .map(|character| match character {
                'ü' => 0xfc,
                'Ä' => 0xc4,
                'ö' => 0xf6,
                other if other.is_ascii() => other as u8,
                other => panic!("unmapped test character {other}"),
            })
            .collect()
    }

    fn checked_in_iban_registry() -> IbanRegistrySnapshot {
        let path = repository_root().join("data/iban_registry_release_102.json");
        parse_iban_registry(&fs::read(path).unwrap()).unwrap()
    }

    #[test]
    fn parses_downloads_and_selects_interval_containing_date() {
        let html = r#"
            <a href="/resource/blz-aktuell-csv-data.csv">
              <small>gültig vom 08.06.2026 bis 06.09.2026</small><span>CSV</span>
            </a>
            <a href="/resource/blz-neu-csv-data.csv">
              <small>gültig vom 07.09.2026 bis 06.12.2026</small><span>CSV</span>
            </a>
        "#;
        let candidates = parse_download_candidates(html, DOWNLOAD_PAGE).unwrap();
        assert_eq!(candidates.len(), 2);
        let selected = select_current_candidate(&candidates, date("2026-08-14")).unwrap();
        assert_eq!(selected.valid_from, date("2026-06-08"));
        assert_eq!(
            selected.url,
            "https://www.bundesbank.de/resource/blz-aktuell-csv-data.csv"
        );
    }

    #[test]
    fn rejects_ambiguous_or_missing_current_download() {
        let one = DownloadCandidate {
            url: "https://example.invalid/one.csv".into(),
            valid_from: date("2026-01-01"),
            valid_to: date("2026-03-31"),
        };
        assert!(select_current_candidate(std::slice::from_ref(&one), date("2026-04-01")).is_err());
        assert!(select_current_candidate(&[one.clone(), one], date("2026-02-01")).is_err());
    }

    #[test]
    fn projects_only_leading_records_and_decodes_windows_1252() {
        let source = raw_fixture();
        let projection = project_source(
            &source,
            "https://example.invalid/source.csv".into(),
            date("2026-06-08"),
            date("2026-09-06"),
        )
        .unwrap();
        assert_eq!(projection.records.len(), 2);
        assert_eq!(projection.records[0].bank_code, "10000000");
        assert_eq!(projection.records[1].bank_code, "20000000");
        assert_eq!(projection.synthetic_bank_code, "00000000");
        assert_eq!(projection.source_sha256, sha256_hex(&source));
    }

    #[test]
    fn projection_round_trips_through_compact_schema() {
        let projection = project_source(
            &raw_fixture(),
            "https://example.invalid/source.csv".into(),
            date("2026-06-08"),
            date("2026-09-06"),
        )
        .unwrap();
        let rendered = render_projection(&projection);
        let (metadata, records) = parse_compact_snapshot(&rendered).unwrap();
        assert_eq!(metadata.source_sha256, projection.source_sha256);
        assert_eq!(metadata.synthetic_bank_code, "00000000");
        assert_eq!(records, projection.records);
    }

    #[test]
    fn rejects_source_schema_drift() {
        let source = raw_fixture();
        let decoded = WINDOWS_1252
            .decode(&source)
            .0
            .replace("Bankleitzahl", "BLZ");
        assert!(project_source(
            decoded.as_bytes(),
            "https://example.invalid/source.csv".into(),
            date("2026-06-08"),
            date("2026-09-06"),
        )
        .is_err());
    }

    #[test]
    fn rejects_bics_that_would_fail_the_runtime_parser() {
        assert!(validate_bic("MARKDEF1100", 2).is_ok());
        assert!(validate_bic("MARK1EF1100", 2).is_err());
        assert!(validate_bic("MARKDE!1100", 2).is_err());
        assert!(validate_bic("MARKDEF1!!0", 2).is_err());
    }

    #[test]
    fn unassigned_bank_code_is_deterministic() {
        assert_eq!(
            find_unassigned_bank_code(["00000000", "00000002", "10000000"]).unwrap(),
            "00000001"
        );
        assert_eq!(
            find_unassigned_bank_code(["10000000", "20000000"]).unwrap(),
            "00000000"
        );
    }

    #[test]
    fn parses_multiline_rust_string_constants() {
        let source = r#"
            pub const HASH: &str =
                "abc123";
        "#;
        assert_eq!(extract_rust_string_constant(source, "HASH"), Some("abc123"));
    }

    #[test]
    fn checked_in_iban_registry_is_canonical_and_matches_id_core() {
        let repo_root = repository_root();
        let (path, core_source) = resolve_core_iban_registry(&repo_root).unwrap();
        let snapshot = check_iban_registry_file(
            &path,
            Some(&core_source),
            date("2026-08-14"),
            DEFAULT_IBAN_REGISTRY_WARNING_MONTHS,
        )
        .unwrap();
        assert_eq!(snapshot.release, 102);
        assert_eq!(snapshot.countries.len(), SWIFT_IBAN_REGISTRY_COUNTRY_COUNT);
    }

    #[test]
    fn rejects_unsorted_duplicate_or_checksum_invalid_registry_entries() {
        let mut unsorted = checked_in_iban_registry();
        unsorted.countries.swap(0, 1);
        assert!(validate_iban_registry(&unsorted).is_err());

        let mut duplicate = checked_in_iban_registry();
        duplicate.countries[1] = duplicate.countries[0].clone();
        assert!(validate_iban_registry(&duplicate).is_err());

        let mut invalid_checksum = checked_in_iban_registry();
        let example = &mut invalid_checksum.countries[0].example_electronic;
        let replacement = if example.ends_with('0') { "1" } else { "0" };
        example.replace_range(example.len() - 1.., replacement);
        assert!(validate_iban_registry(&invalid_checksum).is_err());
    }

    #[test]
    fn validates_numeric_alphabetic_and_alphanumeric_bban_segments() {
        let segments = parse_bban_structure("2!n3!a4!c").unwrap();
        validate_bban_against_segments("ZZ", "12ABC9Z8Y", &segments).unwrap();
        assert!(validate_bban_against_segments("ZZ", "1AABC9Z8Y", &segments).is_err());
        assert!(parse_bban_structure("2n3!a").is_err());
        assert!(parse_bban_structure("0!n").is_err());
        assert!(parse_bban_structure("2!x").is_err());
        assert!(parse_published_month("é000-01").is_err());
        validate_registry_identifier("ZZ", "bank identifier", Some("1-4"), Some("2!a2!n"), "AB12")
            .unwrap();
        assert!(validate_registry_identifier(
            "ZZ",
            "bank identifier",
            Some("1-4"),
            Some("4!n"),
            "AB12"
        )
        .is_err());
    }

    #[test]
    fn rejects_unknown_iban_registry_json_fields() {
        let invalid = br#"{
            "registry_name":"swift_iban_registry",
            "registry_authority":"SWIFT, ISO 13616 Registration Authority",
            "release":102,
            "published":"2026-06",
            "source_url":"https://www.swift.com/swift-resource/9606/download",
            "extracted_from_official_registry":true,
            "countries":[],
            "unexpected":true
        }"#;
        assert!(parse_iban_registry(invalid).is_err());
    }

    #[test]
    fn parses_iban_registry_import_options_and_unsigned_constants() {
        let options = parse_refresh_options(&[
            "--refresh-iban-registry".into(),
            "reviewed.json".into(),
            "--dry-run".into(),
        ])
        .unwrap();
        assert_eq!(
            options.iban_registry_import,
            Some(PathBuf::from("reviewed.json"))
        );
        assert_eq!(options.mode, RefreshMode::DryRun);

        let mut source = "pub const RELEASE: u16 = 101;".to_owned();
        replace_rust_unsigned_constant(&mut source, "RELEASE", 102).unwrap();
        assert_eq!(
            extract_rust_unsigned_constant(&source, "RELEASE"),
            Some(102)
        );
    }

    #[test]
    fn parses_rule_catalog_imports_and_rejects_multiple_refresh_selectors() {
        let bdew = parse_refresh_options(&[
            "--refresh-bdew-identifiers".into(),
            "reviewed-bdew.json".into(),
            "--check".into(),
        ])
        .unwrap();
        assert_eq!(
            bdew.bdew_identifiers_import,
            Some(PathBuf::from("reviewed-bdew.json"))
        );
        assert_eq!(bdew.mode, RefreshMode::Check);

        let mastr = parse_refresh_options(&[
            "--refresh-mastr-prefixes".into(),
            "reviewed-mastr.json".into(),
        ])
        .unwrap();
        assert_eq!(
            mastr.mastr_prefixes_import,
            Some(PathBuf::from("reviewed-mastr.json"))
        );

        assert!(parse_refresh_options(&[
            "--refresh-bdew-identifiers".into(),
            "bdew.json".into(),
            "--refresh-mastr-prefixes".into(),
            "mastr.json".into(),
        ])
        .is_err());
    }

    #[test]
    fn every_eic_refresh_mode_requires_an_external_sha256_trust_anchor() {
        for mode in [None, Some("--dry-run"), Some("--check")] {
            let mut args = vec!["--refresh-eic-directory".to_owned()];
            if let Some(mode) = mode {
                args.push(mode.to_owned());
            }
            assert!(parse_refresh_options(&args).is_err());

            args.extend([
                "--eic-source-sha256".to_owned(),
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_owned(),
            ]);
            let options = parse_refresh_options(&args).unwrap();
            assert!(options.refresh_eic_directory);
            assert!(options.eic_source_sha256.is_some());
        }
    }

    #[test]
    fn eic_trust_anchor_and_large_change_confirmation_are_strictly_scoped() {
        assert!(parse_refresh_options(&[
            "--refresh-eic-directory".into(),
            "--eic-source-sha256".into(),
            "ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789".into(),
        ])
        .is_err());
        assert!(parse_refresh_options(&["--accept-large-eic-change".into()]).is_err());

        let options = parse_refresh_options(&[
            "--refresh-eic-directory".into(),
            "--eic-source-sha256".into(),
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
            "--accept-large-eic-change".into(),
        ])
        .unwrap();
        assert!(options.accept_large_eic_change);
    }

    #[test]
    fn bounded_reader_never_accepts_more_than_the_hard_limit() {
        assert_eq!(
            read_to_end_limited(std::io::Cursor::new(b"1234"), 4, "fixture").unwrap(),
            b"1234"
        );
        let error = read_to_end_limited(std::io::Cursor::new(b"12345"), 4, "fixture")
            .unwrap_err()
            .to_string();
        assert!(error.contains("hard download limit"));
    }

    #[test]
    fn eic_source_bytes_must_match_the_external_trust_anchor() {
        let expected = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        verify_eic_source_trust_anchor(b"abc", expected).unwrap();
        let error = verify_eic_source_trust_anchor(b"changed", expected)
            .unwrap_err()
            .to_string();
        assert!(error.contains("external trust anchor"));
        assert!(error.contains("downloaded"));
    }
}
