# Contributing

Thanks for taking a look — contributions are welcome.

## Setup

- Install Rust (stable) via rustup.
- Clone the repo.

## Build / Run

```bash
cargo build
cargo run -- --help
```

## Tests

```bash
cargo test --all-targets
cargo test --release --all-targets
```

## Lint / Format

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
```

## Pull requests

- Keep changes focused (one feature/fix per PR).
- Prefer adding/adjusting tests when behavior changes.
- Run the commands in **Tests** + **Lint / Format** before opening a PR.
- Include a short description and screenshots/terminal output for UX changes.

## Reporting issues

Please include:

- OS (Windows/macOS/Linux)
- How you installed the binary (Release asset vs `cargo install`)
- Exact command you ran and the output
- Your `config.toml` (redact personal paths if needed)
