# Embedded reference data

## German bank codes

`bundesbank_blz_2026-06-08_2026-09-06.csv` is a compact, UTF-8 projection of
the Deutsche Bundesbank bank-code file valid from 2026-06-08 through
2026-09-06. It retains one bank-code-leading record per BLZ and the fields NRG
needs for deterministic test-data generation:

- bank code (BLZ),
- BIC,
- change marker,
- intended-deletion flag,
- successor bank code.

The source URL and SHA-256 of the unmodified Bundesbank CSV are embedded in the
file header. The source CSV is Windows-1252 encoded; the checked-in projection is
UTF-8 with LF line endings. The projection deliberately excludes institution
names and addresses because they are not needed for validation or generation.

The file is a versioned snapshot, not a live directory. A directory hit means
only that the bank code occurs in this snapshot. It does not prove that an
account exists or that a payment can be routed. The header records the
deterministically selected first unassigned BLZ (`00000000` in this snapshot);
the maintenance check recomputes it whenever the snapshot is replaced.

Source: <https://www.bundesbank.de/de/aufgaben/unbarer-zahlungsverkehr/serviceangebot/bankleitzahlen/download-bankleitzahlen-602592>

## International IBAN formats

`iban_registry_release_102.json` is the canonical NRG projection of the
official SWIFT IBAN Registry, release 102 (June 2026). It contains the 89
published country formats, BBAN patterns, bank/branch positions and official
examples used by the international validator and checksum-only generator.
Its SHA-256 identifies the checked-in JSON projection, not the source PDF.

An IBAN that matches the country structure and MOD-97 checksum still does not
prove that its bank or account exists. Outside Germany NRG performs no national
bank-directory or account checks. Official registry examples are explicitly
labelled as examples, never as non-routable sandbox data.

Source: <https://www.swift.com/swift-resource/9606/download>

## BDEW identifier formation rules

`bdew_identifiers_v1.2.json` is a reviewed, canonical projection of the
formation rules implemented for BDEW/DVGW market-partner IDs, NeBe/package IDs
and the four Redispatch resource IDs. It records schema and document versions,
publication and review dates, the official HTTPS source, the SHA-256 of the
reviewed PDF, rule prefixes, lengths, character sets, checksum schemes and
source sections.

The file deliberately contains **no allocation records**. A rule match or a
valid checksum therefore says nothing about actual assignment. The SHA-256 of
the canonical JSON is kept separately in `id-core`, so CI detects changes to
either metadata or rules.

Source: <https://www.bdew.de/media/documents/AWH_Identifikatoren-in-der-Marktkommunikation_Version.1.2.pdf>

## MaStR prefix and role catalog

`mastr_prefixes_2019-05.json` represents the published MaStR number concept as
machine-readable data. It contains all 27 supported three-letter prefixes, the
19 optional role suffixes, their allowed prefix/role matrix, sector, object
group, lifecycle and the common identifier/checksum structure. Legacy `SME`
and `GME` prefixes remain explicitly marked as migrated-unit forms.

This catalog likewise contains no MaStR registrations or allocations. Its
source-document SHA-256 records the PDF reviewed on `2026-08-14`; a separate
canonical-data SHA-256 is linked from `id-core`.

Source: <https://www.marktstammdatenregister.de/MaStRHilfe/files/regHilfen/MaStR-Nummernkonzept.pdf>

## ENTSO-E EIC directory

`entso_e_eic_2026-08-13.tsv` is a deterministic, UTF-8 projection of the
official ENTSO-E `allocated-eic-codes.xml` bulk document created at
`2026-08-13T01:15:13Z`. It contains all 76,256 records in that export: 72,916
with source status `A05` (active) and 3,340 with `A03` (inactive). The
projection retains exactly the EIC code and its timestamped active/inactive
lifecycle status. Every source-provided free-text field is deliberately
discarded, including display and long names, descriptions, functions, dates,
responsible-party data, contacts and addresses. The runtime record type has no
field capable of exposing that discarded content.

The source SHA-256 identifies the downloaded XML; the separate projection
SHA-256 identifies the exact checked-in TSV. A hit means only that the exact
code occurs in this timestamped bulk snapshot with the recorded lifecycle
status. A miss does not prove non-allocation: separate LIO registries and later
changes can differ. The API therefore keeps the unqualified allocation status
`unknown` for both results.

Sources:

- <https://eepublicdownloads.blob.core.windows.net/cio-lio/xml/allocated-eic-codes.xml>
- <https://www.entsoe.eu/data/energy-identification-codes-eic/eic-approved-codes/>
- <https://eepublicdownloads.entsoe.eu/clean-documents/EDI/Library/cim_based/EIC_Data_Exchange_IG_v1.1.pdf>

## Maintenance

The normal CI check is deliberately network-free. It validates the embedded
filename, metadata, schema, ordering, hash syntax and the absence of the
selected synthetic bank code, and warns when the snapshot expires within 30
days:

```sh
cargo xtask check-reference-data
```

To verify the compact projection against the published source as well, opt in
to the network request explicitly:

```sh
cargo xtask check-reference-data --verify-source
```

The EIC projection is always two-column-schema-, count-, order- and
hash-checked offline.
An explicit network verification downloads and deterministically re-projects
the current ENTSO-E bulk XML:

```sh
cargo xtask check-reference-data --verify-eic-source
export EXPECTED_EIC_SOURCE_SHA256='b53840e8e377b11d12c63241a6a06e68ee36c221897930dae97c0c6ff1903a98'
cargo xtask refresh-reference-data --refresh-eic-directory \
  --eic-source-sha256 "$EXPECTED_EIC_SOURCE_SHA256" --dry-run
cargo xtask refresh-reference-data --refresh-eic-directory \
  --eic-source-sha256 "$EXPECTED_EIC_SOURCE_SHA256"
```

All EIC refresh modes require the expected source SHA-256 from a separately
reviewed channel and enforce a 96 MiB download limit. The task prints record
add/remove/change and active/inactive lifecycle transitions against the current
snapshot. A record-count change above 5% aborts unless the operator reviews the
diff and explicitly repeats the command with `--accept-large-eic-change`.
Normal `check-reference-data` remains network-free.
The example hash is the reviewed anchor for the checked-in 2026-08-13 source;
replace it only with a new value obtained through the independent review.

The BDEW rule and MaStR prefix sources are human-oriented PDFs. Updating their
machine-readable projections therefore requires an explicitly reviewed local
JSON file. Imports reject URLs, validate the complete vocabulary and rule
matrix, print an add/remove/change review, canonicalize JSON, and update the
matching core metadata and hash:

```sh
cargo xtask refresh-reference-data \
  --refresh-bdew-identifiers /path/to/reviewed-bdew.json \
  --dry-run
cargo xtask refresh-reference-data \
  --refresh-mastr-prefixes /path/to/reviewed-mastr.json \
  --dry-run
```

`cargo xtask check-reference-data --verify-source` additionally downloads the
recorded BDEW and MaStR PDFs and verifies their exact source hashes. Normal CI
remains network-free.

Refreshes select the Bundesbank file valid on the current date, validate its
schema, hash the original bytes, regenerate the compact projection, recompute
the first unassigned test BLZ and update the `id-core` metadata. Inspect without
writing first, then review the record-level diff before accepting a write:

```sh
cargo xtask refresh-reference-data --dry-run
cargo xtask refresh-reference-data
```

SWIFT publishes a human-oriented registry document rather than a stable JSON
feed. A reviewed local JSON projection can be validated and compared before it
is atomically imported:

```sh
cargo xtask refresh-reference-data \
  --refresh-iban-registry /path/to/reviewed-iban-registry.json \
  --dry-run
```
