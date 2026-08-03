# Ptymark contributor instructions

Ptymark is a Rust terminal pre-display renderer. Keep changes product-focused and preserve these contracts:

- terminal controls, progress redraws, alternate-screen traffic, and child argv stay byte-exact;
- user-authored TOML contains portable intent, while resolved paths and hard safety limits remain internal state;
- renderer failure restores exact source unless strict mode was explicitly selected;
- external commands are invoked as typed argv and never through shell interpolation;
- GitHub releases are source-only and must not upload project-built executables.

Before opening or updating a pull request, run:

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

Use `make ptymark-check` for the canonical Docker acceptance path when installer, renderer, shell, or container behavior changes.
