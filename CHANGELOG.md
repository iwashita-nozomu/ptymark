<!--
@dependency-start
contract reference
responsibility Records versioned user-visible changes, safety notes, and known limitations.
upstream environment Cargo.toml package version
upstream design documents/release.md immutable publication and recovery contract
downstream implementation .github/workflows/ptymark-release.yml release publication workflow
@dependency-end
-->

# Changelog

All notable changes to ptymark are documented in this file. The project follows semantic versioning once a stable release line is established; prerelease versions may still change user-facing contracts.

## [Unreleased]

## [0.1.0-alpha.1] - 2026-07-14

### Added

- Native Unix PTY and Windows ConPTY hosting for `ptymark -- COMMAND`, including input forwarding, resize propagation, terminal-mode restoration, and child exit-status preservation.
- Pipe-oriented command filtering through `ptymark run -- COMMAND` and file or stream rendering through `ptymark preview`.
- Mermaid and block-math detection with exact-source fallback, bounded in-memory caching, external renderer selection, and an isolated managed renderer bundle.
- Package-local installers and smoke-tested executable archives for Linux, macOS, and Windows.
- A thin WezTerm launcher plugin, portable configuration examples, and shell-coexistence contracts.
- Versioned release manifests, SHA-256 checksums, and GitHub build-provenance attestations.

### Safety

- Terminal control regions, progress redraws, and alternate-screen applications bypass semantic rendering byte-for-byte.
- Child executable and argument boundaries are preserved without constructing a shell command string.
- Normal rendering does not install dependencies or perform network access.

### Known limitations

- Release archives are not yet signed with Apple Developer ID or Windows Authenticode certificates.
- Pixel image placement for WezTerm, Kitty, iTerm2, and Sixel is not included; the initial presenter emits terminal-safe text and ANSI/Unicode output.
- Renderer workers and disk cache are process-local or absent, so cold renderer startup can be noticeable.
- Upgrade, automatic rollback, and uninstall orchestration remain follow-up lifecycle work; this release is always recoverable by reinstalling a previously downloaded versioned archive.
- This is an alpha release. Configuration and presentation details may change before the first stable release.
