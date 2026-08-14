# nrg

NRG generates, validates, parses, and classifies test identifiers for German
energy-market, metering, business-register, and payment workflows. A central
identifier catalog drives the versioned HTTP API, OpenAPI metadata, and the
browser UI; the frontend does not maintain a second identifier list.

Generated values are test data. A valid syntax, checksum, or directory match
never proves real-world allocation, account existence, reachability, ownership,
or production usability. In particular, NRG never claims that an IBAN belongs
to an existing account.

## Supported identifiers

`GET /api/v1/catalog` is the authoritative, machine-readable list of current
capabilities, routes, roles, sectors, checksum schemes, allocation models, and
generation profiles. The current catalog includes:

| Area | Identifier | Capabilities and scope |
| --- | --- | --- |
| Energy · locations | MaLo, MeLo, NeLo, NeBe | Generate and validate; typed parts and separate syntax/checksum/allocation states |
| Energy · market partners | BDEW/DVGW MP-ID | Generate and validate for electricity or gas |
| Energy · resources | CR-ID, SG-ID, SR-ID, TR-ID, package ID | Generate checksum-valid BDEW ASCII fixtures |
| Energy · registers | MaStR identifier | Generate and validate the supported official prefixes and role suffixes |
| Energy · registers | EIC | Parse and validate; exact lookup in the versioned embedded ENTSO-E bulk snapshot with lifecycle status |
| Payments · accounts | IBAN | Generate, format, parse, and validate all 89 countries in SWIFT IBAN Registry release 102; Germany additionally supports the embedded Bundesbank BLZ directory |
| Payments · institutions | BIC | Generate and validate 8- and 11-character values, Test & Training patterns, and directory-backed BLZ/BIC pairs |
| Payments · SEPA | German creditor ID | Validate and return the Bundesbank official test fixture |
| Payments · references | Mandate reference, End-to-End ID, RF creditor reference, UETR | Deterministic fixture generation; RF uses MOD 97 and UETR uses UUID v4; validators are exposed where the standard defines one |
| Business | German VAT ID | Offline syntax parsing only; no public checksum or BZSt/VIES query is claimed |
| Business | LEI | Syntax and MOD-97 validation plus an explicit live lookup in the public GLEIF JSON:API |
| Metering | OBIS | Parse and validate, with lookup in an explicitly non-exhaustive embedded catalog |
| Metering | DIN 43849 device ID | Conservative parser and validator; no allocation claim |

The six original `/api/{malo,melo,nelo}/{generate,validate}` operations remain
available as deprecated compatibility aliases. New integrations should use the
explicit `/api/v1/...` operations and their stable `operationId` values.

## Test-data semantics

Every descriptor advertises only the profiles that make sense for that
identifier:

| Profile | Guarantee |
| --- | --- |
| `official_test_fixture` | A value explicitly published for testing by the responsible authority; currently used for the German SEPA creditor ID |
| `synthetic_non_routable` | Formally valid test data deliberately made non-routable against the embedded reference snapshot |
| `directory_plausible` | Uses real directory components; collision with a real-world object cannot be excluded |
| `checksum_only` | Format and checksum are valid; assignment and existence were not checked |
| `test_training_pattern` | Matches the BIC Test & Training marker convention; it is not represented as a SWIFT-assigned T&T BIC |
| `syntax_only` | Only the standardized shape and character repertoire are asserted |
| `directory_value` | Uses a value from the embedded directory; that is not evidence of an account or present-day assignment |
| `official_example` | Returns an example from the checked-in official IBAN registry; it is not guaranteed to be non-routable |

Generation responses include the canonical and optional formatted value,
`generator_version`, the effective fixture seed, profile, decomposed parts,
reference-data metadata, warnings, and independent `syntax`, `checksum`,
`directory`, and `assignment` checks. `synthetic` and `production_usable` are
reported separately. `account_existence` and `collision_guarantee` make the
generator's limits machine-readable; self-assigned reference batches are
checked for within-batch uniqueness. Centrally allocated identifiers remain
`allocation_status: unknown` unless a real registry was actually queried.

## Versioned API

All generator endpoints use `POST` and accept one to 100 values, an optional
deterministic seed, an output format, and identifier-specific options. The same
request, seed, generator version, and reference-data version reproduces the
same fixture batch.

Generate five non-routable German IBAN fixtures:

```bash
curl -sS http://localhost:8080/api/v1/payments/accounts/iban/generate \
  -H 'content-type: application/json' \
  --data '{
    "profile": "synthetic_non_routable",
    "count": 5,
    "fixture_seed": "integration-test-4711",
    "format": "electronic",
    "country": "DE"
  }'
```

For every registry country, `checksum_only` can create a country-format and
MOD-97-valid IBAN and `official_example` can return the checked-in registry
example. Germany additionally offers `synthetic_non_routable` and
`directory_plausible`; no international bank directory beyond the German BLZ
snapshot is implied.

Validators accept a deliberately small common request:

```bash
curl -sS http://localhost:8080/api/v1/payments/accounts/iban/validate \
  -H 'content-type: application/json' \
  --data '{"id":"DE79000000001234567890"}'
```

The validation report distinguishes valid syntax, country length, BBAN shape,
checksum, directory evidence, and assignment knowledge. Even a fully
checksum-valid and directory-plausible IBAN leaves account existence unknown.

The LEI lookup validates first and then queries the public GLEIF JSON:API with a
bounded timeout and response size. It separates `found`, `not_found`,
`unknown`, and `upstream_error`; a found registry record is not independent
proof of an entity's identity or suitability for a transaction. Positive
results are cached for 15 minutes and negative results for five minutes in a
256-entry LRU-style process/isolate cache. Upstream failures are never cached.
`cache_status` and `cache_ttl_seconds` make this behavior visible to clients.
Only cache misses consume the app-side GLEIF budget of 60 upstream requests per
minute; `429` responses carry `Retry-After` and `retry_after_seconds`. EIC lookup
checks the exact code against the embedded ENTSO-E bulk snapshot and returns
its active/inactive lifecycle record when present. A miss remains explicitly
snapshot-scoped and does not prove non-allocation in decentralized LIO
registries. The OBIS lookup is local and reports both misses and the embedded
catalog's non-exhaustive scope.

### Reproducible scenarios

`GET /api/v1/scenarios` describes the six built-in fixture graphs:

- `supplier_basic`
- `supplier_direct_debit`
- `grid_operator_electricity`
- `grid_operator_gas`
- `metering_point_operator`
- `redispatch_resource_bundle`

Generate a coherent scenario with one seed:

```bash
curl -sS http://localhost:8080/api/v1/scenarios/generate \
  -H 'content-type: application/json' \
  --data '{
    "scenario": "supplier_direct_debit",
    "sector": "electricity",
    "profile": "synthetic_non_routable",
    "fixture_seed": "nrg-demo-1"
  }'
```

Each item reports its dependencies. If a requested profile has no valid
meaning for one item, the scenario uses the descriptor's safe profile and
returns a warning instead of overstating the guarantee.

### Verified negative fixtures

Every generating identifier and every supported validator-only identifier
also exposes an explicit endpoint at
`POST /api/v1/test-data/negative/{slug}/generate`. It first creates a valid,
reviewed source fixture, applies one requested `length`, `character_set`, or
`checksum` defect, and verifies that the matching validator rejects the result.
Checksum mutations return `422` for identifiers without a standardized
checksum.

```bash
curl -sS http://localhost:8080/api/v1/test-data/negative/iban/generate \
  -H 'content-type: application/json' \
  --data '{
    "mutation": "checksum",
    "fixture_seed": "negative-test-1",
    "profile": "synthetic_non_routable",
    "country": "DE"
  }'
```

## Browser UI and OpenAPI

The catalog-driven frontend provides role and sector facets, cross-sector
matching, full-text search, domain navigation, a common identifier detail
view, batch generation, fixture seeds, electronic/formatted output, request
previews, validation/part display, lookup actions, verified negative fixtures,
scenario generation, and text/JSON/CSV copy. Adding a descriptor and its public
operations makes it discoverable without another hard-coded frontend list.

- `http://localhost:8080` — frontend
- `http://localhost:8080/swagger-ui` — API documentation
- `http://localhost:8080/api-docs/openapi.json` — OpenAPI document

Each OpenAPI operation has one primary area tag and stable `operationId`.
Catalog-derived `x-nrg-*` extensions carry the domain, roles, sectors,
capabilities, allocation model, and profiles without duplicating operations
across normal Swagger tags.

## Embedded reference data

NRG performs format and directory checks against versioned, local data. It does
not download reference snapshots during an API request; only an explicit live
lookup operation such as LEI contacts its named upstream registry.

- `data/bundesbank_blz_2026-06-08_2026-09-06.csv` is a compact projection of
  the Bundesbank BLZ file for the stated validity period. Responses expose its
  validity interval and source hash. The selected non-routable BLZ is checked
  against every new snapshot.
- `data/iban_registry_release_102.json` is the reviewed canonical projection of
  SWIFT IBAN Registry release 102 (June 2026), covering 89 countries. Responses
  expose its release and canonical data hash.
- `data/bdew_identifiers_v1.2.json` captures the implemented BDEW/DVGW
  formation rules without allocation records. Its metadata pins both the
  reviewed source-document hash and the canonical JSON hash.
- `data/mastr_prefixes_2019-05.json` contains all supported MaStR prefixes,
  optional role suffixes and their compatibility matrix, likewise without
  registrations or allocation claims.
- `data/entso_e_eic_2026-08-13.tsv` is a privacy-minimized projection of the
  official ENTSO-E bulk export. It retains only EIC code and exact
  active/inactive lifecycle status; all free text, names, descriptions,
  functions, dates, responsible-party, contact and address data are omitted.
  Source and projection hashes remain pinned.

`synthetic_non_routable` means that the generated German bank code is absent
from the exact embedded BLZ snapshot. `directory_plausible` may use a real BLZ
and BIC, so collision with a real account is possible and account existence
remains unknown.

The maintenance task validates every embedded snapshot and rule catalog
offline:

```bash
nix develop -c cargo xtask check-reference-data
nix develop -c cargo xtask refresh-reference-data --dry-run
```

The BLZ refresh downloads and validates the current Bundesbank source, selects
the effective interval, recalculates the synthetic BLZ, and prints a review
diff before an atomic update. SWIFT publishes a human-oriented registry, so an
IBAN refresh deliberately requires a separately prepared and reviewed local
JSON projection. The human-oriented BDEW and MaStR source PDFs use the same
reviewed-local-import model:

```bash
nix develop -c cargo xtask refresh-reference-data \
  --refresh-iban-registry /path/to/reviewed-iban-registry.json \
  --dry-run
nix develop -c cargo xtask refresh-reference-data \
  --refresh-bdew-identifiers /path/to/reviewed-bdew.json \
  --dry-run
nix develop -c cargo xtask refresh-reference-data \
  --refresh-mastr-prefixes /path/to/reviewed-mastr.json \
  --dry-run
EXPECTED_EIC_SOURCE_SHA256='b53840e8e377b11d12c63241a6a06e68ee36c221897930dae97c0c6ff1903a98'
nix develop -c cargo xtask refresh-reference-data \
  --refresh-eic-directory \
  --eic-source-sha256 "$EXPECTED_EIC_SOURCE_SHA256" \
  --dry-run
```

EIC refreshes require that expected source SHA-256 from a separately reviewed
channel in write, dry-run, and check mode. They enforce a 96 MiB download cap,
print record and lifecycle diffs against the current snapshot, and stop on a
record-count change above 5% unless it is explicitly confirmed with
`--accept-large-eic-change`. The ordinary reference-data check stays
network-free.
The value shown above is the reviewed trust anchor for the checked-in
2026-08-13 source and must be replaced only after independent review.

See `crates/xtask/README.md` for check mode, write mode, expiry warnings,
canonicalization, and import invariants.

## Development environment

[Nix](https://nixos.org/download/) with flakes enabled is the only host
prerequisite. Enter the reproducible shell before running project commands:

```bash
nix develop
```

The committed flake lock provides Rust, the WebAssembly target and linker,
Node.js 24, pnpm 11.21, `wasm-pack`, `worker-build`, Wrangler's native helper
dependencies, the Docker client, and native build libraries. The shell supports
Apple Silicon macOS and ARM64/x86-64 Linux; the pinned Nixpkgs revision no
longer supports Intel macOS.

Install the pinned JavaScript dependency once:

```bash
pnpm install --frozen-lockfile
```

Build the browser WebAssembly artifact and run the native Axum server:

```bash
make wasm
cargo run -p server
```

The current frontend invokes the shared versioned HTTP API. `frontend/pkg` is a
separately built browser-WASM distribution of `id-core`, while both the native
server and Cloudflare adapter use the same `nrg-api` router and domain logic.

## Docker

Build and run the native server and complete static frontend:

```bash
docker build -t nrg .
docker run --rm --read-only -p 8080:8080 nrg
```

The multi-stage image compiles browser WASM and the release server separately,
contains the complete frontend (including Swagger assets), and runs as the
unprivileged numeric user `65534:65534`. Docker commands require a running
Docker-compatible daemon; the Nix shell supplies only the client.

## Cloudflare Worker

The second deployment target is a regular Rust/WASM Cloudflare Worker, not a
Cloudflare Container. The build produces:

1. `id-core` browser WASM in `frontend/pkg`;
2. `cloudflare-worker`, including the shared Axum API, in
   `crates/cloudflare-worker/build`.

Run the local Worker or validate the complete upload without publishing:

```bash
pnpm dev
pnpm deploy:dry-run
```

Deploy the Worker and static assets to the configured custom domain:

```bash
pnpm deploy
```

The configured production URL is `https://nrg.miladiir.de`. Wrangler watches
the Rust workspace, build files, and embedded `data/` directory, so a reference
snapshot change rebuilds the Worker during development.

The Worker adds a second, per-Cloudflare-location limit of 120 LEI lookup
requests per minute through the `LEI_LOOKUP_RATE_LIMITER` binding. Its constant
route-class key deliberately contains neither a LEI nor a client IP, so full
identifiers do not become logs, metrics labels, or rate-limit dimensions. The
checked-in namespace ID is a non-secret account-local identifier; if that ID is
already used by another rate-limit binding in the target Cloudflare account,
assign a different positive integer before deployment. Cloudflare's limiter is
permissive and eventually consistent, so the app-side miss budget remains the
portable upstream safeguard for the native server and Worker alike.
See Cloudflare's [Workers Rate Limiting API](https://developers.cloudflare.com/workers/runtime-apis/bindings/rate-limit/)
for the binding's locality and consistency semantics.

## Fuzzing

The separate `fuzz/` workspace feeds arbitrary bytes, valid UTF-8, and
projected Unicode into every public parser and validator. CI compiles the
harness and checks its formatting; longer campaigns remain an explicit local
or scheduled job:

```bash
cargo install cargo-fuzz
nix develop -c cargo fuzz run validators -- -max_total_time=60
```

The invariant is panic-free handling of arbitrary input, not acceptance of the
input. See `fuzz/README.md` for details.

## Validation and publishing

Run the same substantive checks used by CI:

```bash
nix develop -c cargo xtask check-reference-data
nix develop -c cargo test --workspace --exclude cloudflare-worker
nix develop -c cargo fmt --all -- --check
nix develop -c cargo fmt --manifest-path fuzz/Cargo.toml -- --check
nix develop -c cargo clippy --workspace --exclude cloudflare-worker --all-targets -- -D warnings
nix develop -c cargo check --manifest-path fuzz/Cargo.toml --bin validators --locked
nix develop -c cargo build --workspace --exclude cloudflare-worker
nix develop -c make wasm-release
nix develop -c cargo check -p cloudflare-worker --target wasm32-unknown-unknown
nix develop -c make worker
nix develop -c pnpm exec wrangler deploy --dry-run
```

The validation workflow runs these native, reference-data, browser-WASM,
Worker-WASM, OpenAPI/catalog, and deployment-bundle checks for pull requests and
pushes to `main`. The Docker publication workflow repeats the complete quality
gate and can publish to GHCR only after it succeeds.
