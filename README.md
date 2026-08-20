# nrg

NRG generates and validates test identifiers for German energy-market,
metering, business, and payment workflows. Generated values are synthetic test
data; a valid syntax or checksum never proves real-world allocation, account
existence, or production usability.

## API

Every identifier lives behind its own route:

| Area | Identifier | Routes |
| --- | --- | --- |
| Energie | MaLo, MeLo, NeLo, NeBe | `/api/v1/{malo,melo,nelo,nebe}/{generate,validate}` |
| Energie | BDEW/DVGW MP-ID | `/api/v1/mp-id/{generate,validate}` (optional `sector`: `electricity` or `gas`) |
| Energie | CR-ID, SG-ID, SR-ID, TR-ID, Paket-ID | `/api/v1/{cr-id,sg-id,sr-id,tr-id,package-id}/generate` |
| Energie | MaStR-Nummer | `/api/v1/mastr/{generate,validate}` (optional `sector`, `prefix`, `role_suffix`) |
| Energie | EIC | `/api/v1/eic/{validate,lookup}`; lookup checks the embedded ENTSO-E snapshot |
| Zahlungsverkehr | IBAN | `/api/v1/iban/{generate,validate}` (optional `country`, default `DE`; optional `format`) |
| Zahlungsverkehr | BIC | `/api/v1/bic/{generate,validate}` (optional `include_branch`) |
| Zahlungsverkehr | Gläubiger-ID, Mandatsreferenz, End-to-End-ID, RF-Referenz, UETR | `/api/v1/{creditor-id,mandate-reference,end-to-end-id,rf-reference,uetr}/{generate,validate}`; RF additionally takes `format` and `invoice_reference` |
| Messwesen | OBIS | `/api/v1/obis/{validate,lookup}`; lookup checks an embedded, non-exhaustive catalog |
| Messwesen | DIN-43849-Gerätekennung | `/api/v1/din-43849/validate` |
| Unternehmen | USt-IdNr. (DE) | `/api/v1/vat-id/validate` (syntax only) |
| Unternehmen | LEI | `/api/v1/lei/{validate,lookup}`; lookup queries the public GLEIF JSON:API |

All generator endpoints use `POST` and accept `count` (1–100, default 1) and an
optional `seed`. The same seed reproduces the same values; without a seed each
batch is random. IBAN and RF reference also accept `format`: `electronic`
(default) or `formatted` for the grouped-in-fours representation.

```bash
curl -sS http://localhost:8080/api/v1/iban/generate \
  -H 'content-type: application/json' \
  --data '{"count": 5, "seed": "integration-test-4711"}'
```

```json
{"values":["DE47030016382797632510", "..."]}
```

German IBANs use a bank code that is absent from the embedded Bundesbank BLZ
snapshot, so they cannot route to a real account; other registry countries get
country-format and MOD-97-valid values.

Validators accept `{"id": "..."}` and answer with `valid` plus an `error`
message for invalid values:

```bash
curl -sS http://localhost:8080/api/v1/iban/validate \
  -H 'content-type: application/json' \
  --data '{"id":"DE79000000001234567890"}'
```

The LEI lookup validates first and then queries the public GLEIF JSON:API with
a bounded timeout and response size. Positive results are cached for 15
minutes and negative results for five minutes in a 256-entry process/isolate
cache; only cache misses consume the app-side budget of 60 upstream requests
per minute, and `429` responses carry `Retry-After`. The EIC and OBIS lookups
answer locally from embedded snapshots.

- `http://localhost:8080` — frontend
- `http://localhost:8080/swagger-ui` — API documentation
- `http://localhost:8080/api-docs/openapi.json` — OpenAPI document

The browser frontend generates and validates locally through a WebAssembly
build of `id-core` (`frontend/pkg`), calling the same `id_core::ops` dispatch
as the HTTP handlers — frontend and backend run the exact same code. Only the
lookups call the API: LEI needs the server-side GLEIF cache and rate limit,
and EIC/OBIS use the same path for consistency.

## Embedded reference data

NRG performs format and directory checks against versioned, local data. It
does not download reference snapshots during an API request; only the explicit
LEI lookup contacts its upstream registry.

- `data/bundesbank_blz_2026-06-08_2026-09-06.csv` is a compact projection of
  the Bundesbank BLZ file; it drives the non-routable German IBAN generator.
- `data/iban_registry_release_102.json` is the reviewed canonical projection of
  SWIFT IBAN Registry release 102 (June 2026), covering 89 countries.
- `data/bdew_identifiers_v1.2.json` captures the implemented BDEW/DVGW
  formation rules.
- `data/mastr_prefixes_2019-05.json` contains the supported MaStR prefixes and
  role suffixes.
- `data/entso_e_eic_2026-08-13.tsv` is a privacy-minimized projection of the
  official ENTSO-E bulk export (EIC code and lifecycle status only).

The maintenance task validates every embedded snapshot offline:

```bash
nix develop -c cargo xtask check-reference-data
nix develop -c cargo xtask refresh-reference-data --dry-run
```

See `crates/xtask/README.md` for check mode, write mode, expiry warnings,
canonicalization, and import invariants.

## Development environment

[Nix](https://nixos.org/download/) with flakes enabled is the only host
prerequisite:

```bash
nix develop
pnpm install --frozen-lockfile
make run
```

With [direnv](https://direnv.net) installed, the checked-in `.envrc` enters
that same shell automatically; run `direnv allow` once and drop the
`nix develop` prefix from every command:

```bash
direnv allow
```

This matters for builds: outside the pinned shell, a globally installed
`rustc` typically lacks the `wasm32-unknown-unknown` target and the
`WASM_OPT_BIN`/`ESBUILD_BIN`/`WASM_BINDGEN_BIN` variables are unset, so
`make worker` and `wrangler deploy` fail.

`make run` builds the browser WebAssembly artifact (`make wasm`) and starts
the native Axum server. Both the native server and the Cloudflare adapter use
the same `nrg-api` router and domain logic from `id-core`.

## Docker

```bash
docker build -t nrg .
docker run --rm --read-only -p 8080:8080 nrg
```

The image contains the release server and the static frontend and runs as the
unprivileged numeric user `65534:65534`.

## Cloudflare Worker

The second deployment target is a Rust/WASM Cloudflare Worker:

```bash
pnpm dev
pnpm run deploy:dry-run
pnpm run deploy
```

Use `pnpm run deploy`, not `pnpm deploy`: the bare form collides with pnpm's
built-in `deploy` command and ends up invoking `wrangler deploy deploy`.

The configured production URL is `https://nrg.miladiir.de`. The Worker adds a
per-location limit of 120 LEI lookup requests per minute through the
`LEI_LOOKUP_RATE_LIMITER` binding; the app-side miss budget remains the
portable upstream safeguard for both deployment targets.

## Fuzzing

The separate `fuzz/` workspace feeds arbitrary bytes, valid UTF-8, and
projected Unicode into every public parser and validator:

```bash
cargo install cargo-fuzz
nix develop -c cargo fuzz run validators -- -max_total_time=60
```

The invariant is panic-free handling of arbitrary input. See `fuzz/README.md`.

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

The Docker publication workflow repeats the quality gate and can publish to
GHCR only after it succeeds.
