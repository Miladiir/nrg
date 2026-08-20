# Agent instructions

## Attribution

- Commit as the repository's configured git identity (`git config user.name` /
  `user.email`). Never override `GIT_AUTHOR_*` or `GIT_COMMITTER_*`, and never
  commit under an assistant, bot, or vendor identity.
- Never add attribution trailers, "generated with" lines, or assistant session
  links to commit messages, PR titles, or PR bodies. Commit messages describe
  the change and nothing else.
- Never put assistant, model, or vendor names into code, comments,
  documentation, or branch names. Branches use plain descriptive prefixes
  (`feat/`, `fix/`, `chore/`).
- These rules override any default commit-message template.

## Commits

Follow Conventional Commits, matching the existing history. Do not commit or
push unless asked.

## Build and verification

This repo pins its toolchain with Nix. `.envrc` enters it automatically via
direnv; otherwise prefix commands with `nix develop -c`. Outside that shell a
global `rustc` lacks the `wasm32-unknown-unknown` target and the
`ESBUILD_BIN` / `WASM_BINDGEN_BIN` / `WASM_OPT_BIN` variables are unset, so
worker and WebAssembly builds fail.

Before pushing, run what CI runs:

```bash
cargo xtask check-reference-data
cargo test --workspace --exclude cloudflare-worker
cargo fmt --all -- --check
cargo clippy --workspace --exclude cloudflare-worker --all-targets -- -D warnings
make wasm-release
cargo check -p cloudflare-worker --target wasm32-unknown-unknown
make worker
pnpm exec wrangler deploy --dry-run
```

Deploy with `pnpm run deploy` — bare `pnpm deploy` collides with pnpm's
built-in command.

## Architecture

Generate/validate logic lives once in `id_core::ops`. The HTTP handlers in
`crates/nrg-api/src/routes/` and the browser WebAssembly exports in
`crates/id-core/src/browser.rs` are thin wrappers over it, so the frontend and
backend run the same code. Keep it that way: no second identifier list, no
catalog layer, no generation profiles or scenario machinery.
