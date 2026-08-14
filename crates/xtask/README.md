# Reference-data xtask

This workspace crate maintains the versioned Deutsche Bundesbank BLZ snapshot,
the canonical JSON projection of the SWIFT IBAN Registry, the BDEW formation
rules, the MaStR prefix/role catalog and the ENTSO-E EIC bulk-directory
projection. It intentionally has no network-dependent tests; production
commands use the system `curl` executable for HTTPS downloads.

```sh
cargo xtask refresh-reference-data --dry-run
cargo xtask refresh-reference-data --check
cargo xtask check-reference-data
cargo xtask refresh-reference-data --refresh-iban-registry /path/to/reviewed.json --dry-run
cargo xtask refresh-reference-data --refresh-bdew-identifiers /path/to/reviewed.json --dry-run
cargo xtask refresh-reference-data --refresh-mastr-prefixes /path/to/reviewed.json --dry-run
EXPECTED_EIC_SOURCE_SHA256='b53840e8e377b11d12c63241a6a06e68ee36c221897930dae97c0c6ff1903a98'
cargo xtask refresh-reference-data --refresh-eic-directory \
  --eic-source-sha256 "$EXPECTED_EIC_SOURCE_SHA256" --dry-run
cargo test -p xtask
```

`refresh-reference-data` chooses the uncompressed CSV whose published validity
interval contains `--as-of` (today by default), validates and decodes the
Windows-1252/ISO-8859-1 source, hashes the unmodified bytes, retains exactly the
records with `Merkmal=1`, and emits a record-level review diff. Normal mode
writes the dated projection through a same-directory temporary file and rename.
`--dry-run` never writes; `--check` additionally exits unsuccessfully when the
dated projection is absent or differs. Every normal refresh also validates the
currently embedded IBAN Registry projection offline.

`check-reference-data` is network-free by default: it resolves every dataset
embedded by `id-core`. For the BLZ snapshot it checks metadata against the Rust constants,
schema, sort order, hash syntax and the declared, deterministically selected
synthetic BLZ. For the IBAN Registry it enforces the strict canonical JSON
schema and metadata, exactly 89 unique country entries in country-code order,
IBAN/BBAN lengths and BBAN character patterns, all official examples and their
streaming MOD-97 results, and the SHA-256 declared by `id-core`. It warns when
the publication month is more than 12 months old; use
`--iban-warning-months` to adjust that review threshold.

The BDEW and MaStR checks strictly deserialize their canonical JSON, reject
unknown fields and allocation claims, verify unique/sorted vocabularies and
exact runtime rule matrices, compare both source-document and canonical-data
hashes with public `id-core` constants, and warn when the recorded review date
is more than 12 months old.

The EIC check validates its strict two-column TSV schema, source metadata,
exact active/inactive counts, unique sort order and the projection hash linked
from `id-core`. The projection contains only EIC code and lifecycle status;
all source-provided free text and potentially sensitive contact or credential
content is discarded before rendering. The ordinary check remains entirely
network-free.

Every EIC refresh mode (`write`, `--dry-run`, and `--check`) requires
`--eic-source-sha256` as an external trust anchor. Obtain that lowercase
SHA-256 through a separately reviewed channel; computing it from the download
performed by this same refresh is not a trust anchor. The command rejects the
download before parsing if the hash differs and enforces a 96 MiB download cap.
It then preserves both `A05` active and `A03` inactive records, prints bounded
lists of added, removed, changed, activated and deactivated codes against the
existing snapshot, and atomically updates the privacy-minimized dated
projection plus core constants:

```sh
export EXPECTED_EIC_SOURCE_SHA256='b53840e8e377b11d12c63241a6a06e68ee36c221897930dae97c0c6ff1903a98'
cargo xtask refresh-reference-data --refresh-eic-directory \
  --eic-source-sha256 "$EXPECTED_EIC_SOURCE_SHA256" --dry-run
cargo xtask refresh-reference-data --refresh-eic-directory \
  --eic-source-sha256 "$EXPECTED_EIC_SOURCE_SHA256"
cargo xtask check-reference-data --verify-eic-source
```

The value shown is the independently reviewed anchor for the checked-in
2026-08-13 source. A later refresh must use the new value obtained through the
separate review channel.

If the total record count moves by more than 5%, the diff is printed and the
refresh aborts. Only after reviewing that diff may the operator repeat the
command with `--accept-large-eic-change`. A first import without any comparison
snapshot is treated as a large change. `--verify-eic-source` uses the reviewed
hash already pinned in the checked-in snapshot and applies the same size cap;
it is opt-in and does not change the offline default.

SWIFT publishes the registry as a human-oriented document rather than a stable
machine-readable JSON feed. The xtask therefore does **not** pretend that PDF
extraction is safe. To refresh it, first prepare and review a local JSON
projection carrying the official source URL, release and publication month,
then run:

```sh
cargo xtask refresh-reference-data \
  --refresh-iban-registry /path/to/reviewed-iban-registry.json \
  --dry-run
cargo xtask refresh-reference-data \
  --refresh-iban-registry /path/to/reviewed-iban-registry.json
```

The import rejects URLs, validates every record, renders canonical JSON, prints
a country-level add/remove/change diff, hashes the exact rendered bytes, and
writes the projection and matching `id-core` metadata atomically one file at a
time. `--check` is available for a no-write CI comparison. The official source
URL must remain HTTPS.

BDEW and MaStR publish their rules as human-oriented PDFs, so these projections
also use reviewed local imports instead of brittle extraction. The commands
print rule/prefix/role add-remove-change summaries and atomically update the
canonical file plus `crates/id-core/src/reference_data/catalogs.rs`:

```sh
cargo xtask refresh-reference-data \
  --refresh-bdew-identifiers /path/to/reviewed-bdew.json \
  --dry-run
cargo xtask refresh-reference-data \
  --refresh-mastr-prefixes /path/to/reviewed-mastr.json \
  --dry-run
```

`--verify-source` additionally downloads the recorded Bundesbank, BDEW and
MaStR sources to verify their SHA-256 values (and re-projects the Bundesbank
CSV). The local check emits a GitHub Actions warning when expiry is at most 30
days away and fails after expiry.

After a refresh, review the printed BLZ diff and the mechanical update to
`crates/id-core/src/reference_data.rs` or `reference_data/catalogs.rs`, then run
`check-reference-data` and the workspace tests. Refresh writes the projection
first and updates the core metadata only after that file is safely in place.
