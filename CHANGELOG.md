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

### Changed

- Adopted a source-only distribution policy: GitHub Releases contain immutable tags, release notes, and GitHub-generated source snapshots, but no project-uploaded executables or installer archives.
- Cross-platform executable and package smoke tests remain required in CI, but their outputs are discarded instead of uploaded.
- Withdrew the project-uploaded executable assets, binary checksums, binary manifests, and binary attestations from `v0.1.0-alpha.1` and `v0.1.0-alpha.2`; tags and release notes remain immutable.

### Security

- Clarified that checksums and provenance do not replace operating-system code signing, notarization, reputation, revocation, or an approved package-manager trust path.
- Local source builds and third-party packages are not automatically trusted or endorsed by the project.

## [0.1.0-alpha.2] - 2026-07-14

### Added

- `ptymark doctor`, `ptymark doctor --json`, and atomic redacted support-report files using the versioned `ptymark.doctor.v1` schema.
- Stable diagnostic finding codes and ready/degraded/unusable exit categories (`0`, `10`, and `20`).
- Public-safe support forms and packaged troubleshooting documentation.

### Changed

- External render and presentation attempts now share a ten-second monotonic hard deadline.
- Later terminal output held behind one unresolved semantic block is bounded to one MiB; overload restores exact source and resumes in order.
- Renderer stdout, artifacts, presentation bytes, and diagnostic stderr remain bounded; source-bearing stderr is not copied into public findings.

### Safety

- Timeout, output-limit, process-exit, invalid-artifact, and presentation failures never enter the cache.
- Timed-out renderer/presenter process trees are cleaned up without terminating the user's PTY/ConPTY child.
- Default doctor performs no install, download, network access, renderer/browser execution, child launch, or mutation.
- Default reports exclude semantic source, child environment, credentials, sensitive path prefixes, raw renderer stderr, and terminal-control bytes.

### Known limitations

- Guided setup, CJK/grapheme/accessibility completion, lifecycle commands, signed channels, persistent workers/cache, and image protocols remain later work.
- Project-uploaded executable assets for this release were withdrawn on 2026-07-15; the tag and release notes remain available under the source-only policy.

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

- Project-uploaded executable assets for this release were withdrawn on 2026-07-15; the tag and release notes remain available under the source-only policy.
- The originally published archives were not signed with Apple Developer ID or Windows Authenticode certificates.
- Pixel image placement for WezTerm, Kitty, iTerm2, and Sixel is not included; the initial presenter emits terminal-safe text and ANSI/Unicode output.
- Renderer workers and disk cache are process-local or absent, so cold renderer startup can be noticeable.
- Upgrade, automatic rollback, and uninstall orchestration remain follow-up lifecycle work; this release is always recoverable by reinstalling a previously downloaded versioned archive.
- This is an alpha release. Configuration and presentation details may change before the first stable release.
