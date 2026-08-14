use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use chrono::{Datelike, NaiveDate};
use id_core::reference_data::{
    parse_bdew_identifiers, parse_mastr_prefixes, BdewIdentifierRule, BdewIdentifiersSnapshot,
    MastrPrefixRecord, MastrPrefixesSnapshot, MastrRoleSuffixRecord, BDEW_IDENTIFIERS_CHECKED_AT,
    BDEW_IDENTIFIERS_DATA_SHA256, BDEW_IDENTIFIERS_METADATA, BDEW_IDENTIFIERS_PUBLISHED,
    BDEW_IDENTIFIERS_SCHEMA_VERSION, BDEW_IDENTIFIERS_SOURCE_SHA256, BDEW_IDENTIFIERS_SOURCE_URL,
    BDEW_IDENTIFIERS_VERSION, MASTR_PREFIXES_CHECKED_AT, MASTR_PREFIXES_DATA_SHA256,
    MASTR_PREFIXES_METADATA, MASTR_PREFIXES_PUBLISHED, MASTR_PREFIXES_SCHEMA_VERSION,
    MASTR_PREFIXES_SOURCE_SHA256, MASTR_PREFIXES_SOURCE_URL, MASTR_PREFIXES_VERSION,
};
use serde::Serialize;

use super::{
    atomic_write, curl_download, extract_rust_string_constant, extract_rust_unsigned_constant,
    replace_rust_string_constant, replace_rust_unsigned_constant, sha256_hex, DynError,
};

const CORE_SOURCE: &str = "crates/id-core/src/reference_data/catalogs.rs";
const DEFAULT_REVIEW_WARNING_MONTHS: i32 = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ImportMode {
    Write,
    DryRun,
    Check,
}

pub(super) fn check_rule_catalogs(
    repo_root: &Path,
    as_of: NaiveDate,
    verify_source: bool,
) -> Result<(), DynError> {
    let core_path = repo_root.join(CORE_SOURCE);
    let core_source = fs::read_to_string(&core_path)
        .map_err(|error| format!("cannot read {}: {error}", core_path.display()))?;

    let bdew_path = resolve_include(&core_path, &core_source, "BDEW_IDENTIFIERS_JSON")?;
    let bdew_bytes = fs::read(&bdew_path)
        .map_err(|error| format!("cannot read {}: {error}", bdew_path.display()))?;
    let bdew = parse_bdew_identifiers(&bdew_bytes)?;
    verify_bdew(&core_source, &bdew_path, &bdew_bytes, &bdew, verify_source)?;
    emit_review_age("BDEW identifier rules", &bdew.checked_at, as_of)?;
    println!(
        "BDEW identifier-rule check passed: version {}, {} rules, canonical SHA-256 {}.",
        bdew.version,
        bdew.rules.len(),
        sha256_hex(&bdew_bytes)
    );

    let mastr_path = resolve_include(&core_path, &core_source, "MASTR_PREFIXES_JSON")?;
    let mastr_bytes = fs::read(&mastr_path)
        .map_err(|error| format!("cannot read {}: {error}", mastr_path.display()))?;
    let mastr = parse_mastr_prefixes(&mastr_bytes)?;
    verify_mastr(
        &core_source,
        &mastr_path,
        &mastr_bytes,
        &mastr,
        verify_source,
    )?;
    emit_review_age("MaStR prefix catalog", &mastr.checked_at, as_of)?;
    println!(
        "MaStR prefix check passed: version {}, {} prefixes, {} role suffixes, canonical SHA-256 {}.",
        mastr.version,
        mastr.prefixes.len(),
        mastr.role_suffixes.len(),
        sha256_hex(&mastr_bytes)
    );
    Ok(())
}

pub(super) fn refresh_bdew_identifiers(
    repo_root: &Path,
    data_dir: &Path,
    import: &Path,
    mode: ImportMode,
) -> Result<(), DynError> {
    ensure_local_import(import, "--refresh-bdew-identifiers")?;
    ensure_repository_data_dir(repo_root, data_dir)?;
    let bytes = fs::read(import).map_err(|error| {
        format!(
            "cannot read reviewed BDEW import {}: {error}",
            import.display()
        )
    })?;
    let snapshot = parse_bdew_identifiers(&bytes)?;
    let rendered = render_canonical_json(&snapshot)?;
    let destination = data_dir.join(format!("bdew_identifiers_v{}.json", snapshot.version));
    let current = resolve_current(repo_root, "BDEW_IDENTIFIERS_JSON")?;
    println!("BDEW identifier-rule import review:");
    print_bdew_diff(current.as_deref(), &snapshot)?;
    apply_import(
        repo_root,
        &destination,
        &rendered,
        mode,
        CatalogUpdate::Bdew(&snapshot),
    )
}

pub(super) fn refresh_mastr_prefixes(
    repo_root: &Path,
    data_dir: &Path,
    import: &Path,
    mode: ImportMode,
) -> Result<(), DynError> {
    ensure_local_import(import, "--refresh-mastr-prefixes")?;
    ensure_repository_data_dir(repo_root, data_dir)?;
    let bytes = fs::read(import).map_err(|error| {
        format!(
            "cannot read reviewed MaStR import {}: {error}",
            import.display()
        )
    })?;
    let snapshot = parse_mastr_prefixes(&bytes)?;
    let rendered = render_canonical_json(&snapshot)?;
    let destination = data_dir.join(format!("mastr_prefixes_{}.json", snapshot.version));
    let current = resolve_current(repo_root, "MASTR_PREFIXES_JSON")?;
    println!("MaStR prefix-catalog import review:");
    print_mastr_diff(current.as_deref(), &snapshot)?;
    apply_import(
        repo_root,
        &destination,
        &rendered,
        mode,
        CatalogUpdate::Mastr(&snapshot),
    )
}

enum CatalogUpdate<'a> {
    Bdew(&'a BdewIdentifiersSnapshot),
    Mastr(&'a MastrPrefixesSnapshot),
}

fn apply_import(
    repo_root: &Path,
    destination: &Path,
    rendered: &str,
    mode: ImportMode,
    update: CatalogUpdate<'_>,
) -> Result<(), DynError> {
    let data_hash = sha256_hex(rendered.as_bytes());
    let core_path = repo_root.join(CORE_SOURCE);
    let current_core = fs::read_to_string(&core_path)
        .map_err(|error| format!("cannot read {}: {error}", core_path.display()))?;
    let updated_core = prepare_core_update(&current_core, destination, &data_hash, update)?;
    let data_current = fs::read(destination)
        .map(|bytes| bytes == rendered.as_bytes())
        .unwrap_or(false);
    let core_current = current_core == updated_core;

    println!("Destination: {}", destination.display());
    println!("Canonical data SHA-256: {data_hash}");
    match mode {
        ImportMode::Check if !data_current || !core_current => Err(format!(
            "reference-catalog update required (data current: {data_current}, id-core metadata current: {core_current})"
        )
        .into()),
        ImportMode::Check => {
            println!("Check passed; catalog and id-core metadata are current.");
            Ok(())
        }
        ImportMode::DryRun => {
            println!("Data update required: {}", if data_current { "no" } else { "yes" });
            println!("id-core metadata update required: {}", if core_current { "no" } else { "yes" });
            println!("Dry run complete; no files were written.");
            Ok(())
        }
        ImportMode::Write => {
            if !data_current {
                atomic_write(destination, rendered.as_bytes())?;
                println!("Wrote {} atomically.", destination.display());
            }
            if !core_current {
                atomic_write(&core_path, updated_core.as_bytes())?;
                println!("Updated {} atomically.", core_path.display());
            }
            if data_current && core_current {
                println!("Catalog and id-core metadata are already current; no write needed.");
            }
            Ok(())
        }
    }
}

fn prepare_core_update(
    current: &str,
    destination: &Path,
    data_hash: &str,
    update: CatalogUpdate<'_>,
) -> Result<String, DynError> {
    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("catalog destination has no UTF-8 filename")?;
    let mut output = current.to_owned();
    match update {
        CatalogUpdate::Bdew(snapshot) => {
            replace_rust_unsigned_constant(
                &mut output,
                "BDEW_IDENTIFIERS_SCHEMA_VERSION",
                u64::from(snapshot.schema_version),
            )?;
            for (name, value) in [
                ("BDEW_IDENTIFIERS_VERSION", snapshot.version.as_str()),
                ("BDEW_IDENTIFIERS_PUBLISHED", snapshot.published.as_str()),
                ("BDEW_IDENTIFIERS_CHECKED_AT", snapshot.checked_at.as_str()),
                ("BDEW_IDENTIFIERS_SOURCE_URL", snapshot.source_url.as_str()),
                (
                    "BDEW_IDENTIFIERS_SOURCE_SHA256",
                    snapshot.source_sha256.as_str(),
                ),
                ("BDEW_IDENTIFIERS_DATA_SHA256", data_hash),
            ] {
                replace_rust_string_constant(&mut output, name, value)?;
            }
            replace_named_include(
                &mut output,
                "BDEW_IDENTIFIERS_JSON",
                &format!("../../../../data/{file_name}"),
            )?;
        }
        CatalogUpdate::Mastr(snapshot) => {
            replace_rust_unsigned_constant(
                &mut output,
                "MASTR_PREFIXES_SCHEMA_VERSION",
                u64::from(snapshot.schema_version),
            )?;
            for (name, value) in [
                ("MASTR_PREFIXES_VERSION", snapshot.version.as_str()),
                ("MASTR_PREFIXES_PUBLISHED", snapshot.published.as_str()),
                ("MASTR_PREFIXES_CHECKED_AT", snapshot.checked_at.as_str()),
                ("MASTR_PREFIXES_SOURCE_URL", snapshot.source_url.as_str()),
                (
                    "MASTR_PREFIXES_SOURCE_SHA256",
                    snapshot.source_sha256.as_str(),
                ),
                ("MASTR_PREFIXES_DATA_SHA256", data_hash),
            ] {
                replace_rust_string_constant(&mut output, name, value)?;
            }
            replace_named_include(
                &mut output,
                "MASTR_PREFIXES_JSON",
                &format!("../../../../data/{file_name}"),
            )?;
        }
    }
    Ok(output)
}

fn verify_bdew(
    core_source: &str,
    path: &Path,
    bytes: &[u8],
    snapshot: &BdewIdentifiersSnapshot,
    verify_source: bool,
) -> Result<(), DynError> {
    ensure_canonical(bytes, snapshot, path)?;
    let data_hash = sha256_hex(bytes);
    verify_core_string_constants(
        core_source,
        &[
            ("BDEW_IDENTIFIERS_VERSION", &snapshot.version),
            ("BDEW_IDENTIFIERS_PUBLISHED", &snapshot.published),
            ("BDEW_IDENTIFIERS_CHECKED_AT", &snapshot.checked_at),
            ("BDEW_IDENTIFIERS_SOURCE_URL", &snapshot.source_url),
            ("BDEW_IDENTIFIERS_SOURCE_SHA256", &snapshot.source_sha256),
            ("BDEW_IDENTIFIERS_DATA_SHA256", &data_hash),
        ],
    )?;
    verify_schema_constant(
        core_source,
        "BDEW_IDENTIFIERS_SCHEMA_VERSION",
        snapshot.schema_version,
    )?;
    if BDEW_IDENTIFIERS_METADATA.name != snapshot.name
        || BDEW_IDENTIFIERS_SCHEMA_VERSION != snapshot.schema_version
        || BDEW_IDENTIFIERS_VERSION != snapshot.version
        || BDEW_IDENTIFIERS_PUBLISHED != snapshot.published
        || BDEW_IDENTIFIERS_CHECKED_AT != snapshot.checked_at
        || BDEW_IDENTIFIERS_SOURCE_URL != snapshot.source_url
        || BDEW_IDENTIFIERS_SOURCE_SHA256 != snapshot.source_sha256
        || BDEW_IDENTIFIERS_DATA_SHA256 != data_hash
    {
        return Err("linked id-core BDEW metadata differs from the checked catalog".into());
    }
    verify_filename(
        path,
        &format!("bdew_identifiers_v{}.json", snapshot.version),
    )?;
    if verify_source {
        verify_download_hash(&snapshot.source_url, &snapshot.source_sha256, "BDEW source")?;
    }
    Ok(())
}

fn verify_mastr(
    core_source: &str,
    path: &Path,
    bytes: &[u8],
    snapshot: &MastrPrefixesSnapshot,
    verify_source: bool,
) -> Result<(), DynError> {
    ensure_canonical(bytes, snapshot, path)?;
    let data_hash = sha256_hex(bytes);
    verify_core_string_constants(
        core_source,
        &[
            ("MASTR_PREFIXES_VERSION", &snapshot.version),
            ("MASTR_PREFIXES_PUBLISHED", &snapshot.published),
            ("MASTR_PREFIXES_CHECKED_AT", &snapshot.checked_at),
            ("MASTR_PREFIXES_SOURCE_URL", &snapshot.source_url),
            ("MASTR_PREFIXES_SOURCE_SHA256", &snapshot.source_sha256),
            ("MASTR_PREFIXES_DATA_SHA256", &data_hash),
        ],
    )?;
    verify_schema_constant(
        core_source,
        "MASTR_PREFIXES_SCHEMA_VERSION",
        snapshot.schema_version,
    )?;
    if MASTR_PREFIXES_METADATA.name != snapshot.name
        || MASTR_PREFIXES_SCHEMA_VERSION != snapshot.schema_version
        || MASTR_PREFIXES_VERSION != snapshot.version
        || MASTR_PREFIXES_PUBLISHED != snapshot.published
        || MASTR_PREFIXES_CHECKED_AT != snapshot.checked_at
        || MASTR_PREFIXES_SOURCE_URL != snapshot.source_url
        || MASTR_PREFIXES_SOURCE_SHA256 != snapshot.source_sha256
        || MASTR_PREFIXES_DATA_SHA256 != data_hash
    {
        return Err("linked id-core MaStR metadata differs from the checked catalog".into());
    }
    verify_filename(path, &format!("mastr_prefixes_{}.json", snapshot.version))?;
    if verify_source {
        verify_download_hash(
            &snapshot.source_url,
            &snapshot.source_sha256,
            "MaStR source",
        )?;
    }
    Ok(())
}

fn ensure_canonical<T: Serialize>(bytes: &[u8], value: &T, path: &Path) -> Result<(), DynError> {
    let rendered = render_canonical_json(value)?;
    if bytes != rendered.as_bytes() {
        return Err(format!(
            "{} is valid JSON but not in canonical checked-in form",
            path.display()
        )
        .into());
    }
    Ok(())
}

fn render_canonical_json<T: Serialize>(value: &T) -> Result<String, DynError> {
    let mut rendered = serde_json::to_string_pretty(value)?;
    rendered.push('\n');
    Ok(rendered)
}

fn verify_download_hash(url: &str, expected: &str, label: &str) -> Result<(), DynError> {
    let actual = sha256_hex(&curl_download(url)?);
    if actual != expected {
        return Err(
            format!("{label} SHA-256 mismatch: expected {expected}, downloaded {actual}").into(),
        );
    }
    println!("{label} download SHA-256 verified.");
    Ok(())
}

fn verify_core_string_constants(
    core_source: &str,
    checks: &[(&str, &String)],
) -> Result<(), DynError> {
    for (name, expected) in checks {
        let actual = extract_rust_string_constant(core_source, name)
            .ok_or_else(|| format!("id-core constant {name} is missing"))?;
        if actual != expected.as_str() {
            return Err(
                format!("id-core {name} mismatch: expected `{expected}`, got `{actual}`").into(),
            );
        }
    }
    Ok(())
}

fn verify_schema_constant(core_source: &str, name: &str, expected: u16) -> Result<(), DynError> {
    let actual = extract_rust_unsigned_constant(core_source, name)
        .ok_or_else(|| format!("id-core constant {name} is missing"))?;
    if actual != u64::from(expected) {
        return Err(format!("id-core {name} mismatch: expected {expected}, got {actual}").into());
    }
    Ok(())
}

fn resolve_current(repo_root: &Path, constant: &str) -> Result<Option<PathBuf>, DynError> {
    let core_path = repo_root.join(CORE_SOURCE);
    let source = fs::read_to_string(&core_path)
        .map_err(|error| format!("cannot read {}: {error}", core_path.display()))?;
    Ok(Some(resolve_include(&core_path, &source, constant)?))
}

fn resolve_include(core_path: &Path, source: &str, constant: &str) -> Result<PathBuf, DynError> {
    let name_position = source
        .find(constant)
        .ok_or_else(|| format!("cannot find id-core constant {constant}"))?;
    let suffix = &source[name_position..];
    let marker = "include_str!(\"";
    let start = suffix
        .find(marker)
        .map(|offset| name_position + offset + marker.len())
        .ok_or_else(|| format!("cannot find include_str for {constant}"))?;
    let end = source[start..]
        .find("\")")
        .map(|offset| start + offset)
        .ok_or_else(|| format!("unterminated include_str for {constant}"))?;
    Ok(core_path
        .parent()
        .expect("catalogs.rs has a parent")
        .join(&source[start..end]))
}

fn replace_named_include(
    source: &mut String,
    constant: &str,
    replacement: &str,
) -> Result<(), DynError> {
    let name_position = source
        .find(constant)
        .ok_or_else(|| format!("cannot find id-core constant {constant}"))?;
    let marker = "include_str!(\"";
    let start = source[name_position..]
        .find(marker)
        .map(|offset| name_position + offset + marker.len())
        .ok_or_else(|| format!("cannot find include_str for {constant}"))?;
    let end = source[start..]
        .find("\")")
        .map(|offset| start + offset)
        .ok_or_else(|| format!("unterminated include_str for {constant}"))?;
    source.replace_range(start..end, replacement);
    Ok(())
}

fn ensure_local_import(path: &Path, option: &str) -> Result<(), DynError> {
    let rendered = path.to_string_lossy();
    if rendered.starts_with("http://") || rendered.starts_with("https://") {
        return Err(format!("{option} accepts a reviewed local JSON file, not a URL").into());
    }
    Ok(())
}

fn ensure_repository_data_dir(repo_root: &Path, data_dir: &Path) -> Result<(), DynError> {
    if data_dir != repo_root.join("data") {
        return Err(
            "reviewed BDEW/MaStR imports require the repository data directory so id-core metadata stays atomic"
                .into(),
        );
    }
    Ok(())
}

fn verify_filename(path: &Path, expected: &str) -> Result<(), DynError> {
    let actual = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("reference catalog has no UTF-8 filename")?;
    if actual != expected {
        return Err(format!(
            "embedded reference filename `{actual}` does not match metadata `{expected}`"
        )
        .into());
    }
    Ok(())
}

fn emit_review_age(label: &str, checked_at: &str, as_of: NaiveDate) -> Result<(), DynError> {
    let checked = NaiveDate::parse_from_str(checked_at, "%Y-%m-%d")?;
    if checked > as_of {
        return Err(format!("{label} checked_at {checked_at} is later than {as_of}").into());
    }
    let months = (as_of.year() - checked.year()) * 12
        + i32::try_from(as_of.month()).expect("month fits i32")
        - i32::try_from(checked.month()).expect("month fits i32");
    if months > DEFAULT_REVIEW_WARNING_MONTHS {
        println!(
            "::warning::{label} was last reviewed {checked_at} ({months} months old; threshold: {DEFAULT_REVIEW_WARNING_MONTHS})"
        );
    }
    Ok(())
}

fn print_bdew_diff(
    current_path: Option<&Path>,
    next: &BdewIdentifiersSnapshot,
) -> Result<(), DynError> {
    let previous = current_path
        .filter(|path| path.exists())
        .map(fs::read)
        .transpose()?
        .map(|bytes| parse_bdew_identifiers(&bytes))
        .transpose()?;
    let previous_rules = previous
        .as_ref()
        .map(|snapshot| bdew_rule_map(&snapshot.rules))
        .unwrap_or_default();
    let next_rules = bdew_rule_map(&next.rules);
    print_map_diff("rules", &previous_rules, &next_rules);
    if let Some(previous) = previous {
        println!(
            "Metadata: {} ({}) -> {} ({})",
            previous.version, previous.checked_at, next.version, next.checked_at
        );
    } else {
        println!(
            "Metadata: new catalog {} ({})",
            next.version, next.checked_at
        );
    }
    Ok(())
}

fn print_mastr_diff(
    current_path: Option<&Path>,
    next: &MastrPrefixesSnapshot,
) -> Result<(), DynError> {
    let previous = current_path
        .filter(|path| path.exists())
        .map(fs::read)
        .transpose()?
        .map(|bytes| parse_mastr_prefixes(&bytes))
        .transpose()?;
    let previous_prefixes = previous
        .as_ref()
        .map(|snapshot| mastr_prefix_map(&snapshot.prefixes))
        .unwrap_or_default();
    let next_prefixes = mastr_prefix_map(&next.prefixes);
    print_map_diff("prefixes", &previous_prefixes, &next_prefixes);
    let previous_roles = previous
        .as_ref()
        .map(|snapshot| mastr_role_map(&snapshot.role_suffixes))
        .unwrap_or_default();
    let next_roles = mastr_role_map(&next.role_suffixes);
    print_map_diff("role suffixes", &previous_roles, &next_roles);
    if let Some(previous) = previous {
        println!(
            "Metadata: {} ({}) -> {} ({})",
            previous.version, previous.checked_at, next.version, next.checked_at
        );
    } else {
        println!(
            "Metadata: new catalog {} ({})",
            next.version, next.checked_at
        );
    }
    Ok(())
}

fn bdew_rule_map(rules: &[BdewIdentifierRule]) -> BTreeMap<String, serde_json::Value> {
    rules
        .iter()
        .map(|rule| {
            (
                serde_json::to_value(rule.kind)
                    .expect("enum serializes")
                    .as_str()
                    .expect("enum serializes to a string")
                    .to_owned(),
                serde_json::to_value(rule).expect("rule serializes"),
            )
        })
        .collect()
}

fn mastr_prefix_map(records: &[MastrPrefixRecord]) -> BTreeMap<String, serde_json::Value> {
    records
        .iter()
        .map(|record| {
            (
                record.code.clone(),
                serde_json::to_value(record).expect("prefix serializes"),
            )
        })
        .collect()
}

fn mastr_role_map(records: &[MastrRoleSuffixRecord]) -> BTreeMap<String, serde_json::Value> {
    records
        .iter()
        .map(|record| {
            (
                record.code.clone(),
                serde_json::to_value(record).expect("role suffix serializes"),
            )
        })
        .collect()
}

fn print_map_diff(
    label: &str,
    previous: &BTreeMap<String, serde_json::Value>,
    next: &BTreeMap<String, serde_json::Value>,
) {
    let previous_keys: BTreeSet<_> = previous.keys().collect();
    let next_keys: BTreeSet<_> = next.keys().collect();
    let added = next_keys
        .difference(&previous_keys)
        .map(|key| key.as_str())
        .collect::<Vec<_>>();
    let removed = previous_keys
        .difference(&next_keys)
        .map(|key| key.as_str())
        .collect::<Vec<_>>();
    let changed = previous_keys
        .intersection(&next_keys)
        .filter(|key| previous.get(**key) != next.get(**key))
        .map(|key| key.as_str())
        .collect::<Vec<_>>();
    println!("Added {label}: {}", display_keys(&added));
    println!("Removed {label}: {}", display_keys(&removed));
    println!("Changed {label}: {}", display_keys(&changed));
}

fn display_keys(values: &[&str]) -> String {
    if values.is_empty() {
        "none".to_owned()
    } else {
        values.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .unwrap()
            .to_owned()
    }

    #[test]
    fn checked_in_rule_catalogs_are_canonical_and_match_core() {
        check_rule_catalogs(
            &repo_root(),
            NaiveDate::from_ymd_opt(2026, 8, 14).unwrap(),
            false,
        )
        .unwrap();
    }

    #[test]
    fn local_import_guard_rejects_urls_and_custom_data_directories() {
        assert!(
            ensure_local_import(Path::new("https://example.invalid/catalog.json"), "--test")
                .is_err()
        );
        assert!(ensure_repository_data_dir(&repo_root(), Path::new("elsewhere")).is_err());
    }

    #[test]
    fn named_include_replacement_does_not_touch_the_other_catalog() {
        let mut source = concat!(
            "const FIRST: &str = include_str!(\"first.json\");\n",
            "const SECOND: &str = include_str!(\"second.json\");\n",
        )
        .to_owned();
        replace_named_include(&mut source, "SECOND", "new.json").unwrap();
        assert!(source.contains("FIRST: &str = include_str!(\"first.json\")"));
        assert!(source.contains("SECOND: &str = include_str!(\"new.json\")"));
    }
}
