# Validator fuzzing

The `validators` target feeds arbitrary byte sequences and valid projected
Unicode into every public identifier parser and validator in `id-core`. A crash
or panic is always a bug; accepting or rejecting an individual value is left to
the corresponding identifier rules.

The package has its own workspace so it does not become a member of the root
application workspace.

Compile the harness without running a campaign:

```sh
nix develop -c cargo check --manifest-path fuzz/Cargo.toml --bin validators --locked
```

Run a bounded local campaign after installing `cargo-fuzz`:

```sh
cargo install cargo-fuzz
nix develop -c cargo fuzz run validators -- -max_total_time=60
```
