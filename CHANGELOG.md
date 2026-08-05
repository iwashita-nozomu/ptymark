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

No user-visible changes are currently queued after `0.1.0-alpha.5`.


## [0.1.0-alpha.5] - 2026-08-05

### Added

- A session-local interactive rendering toggle that pauses semantic replacement without changing user configuration, installation state, shell profiles, child argv, or the child process.
- Append-only WezTerm `render_toggle_key` integration, defaulting to `CTRL|SHIFT|ALT+R`, with custom-key and disabled forms.
- Real Unix PTY and Windows ConPTY evidence that one block can render before the toggle while the next complete block is restored as exact source.

### Safety

- The native input filter reserves only UTF-8 `U+10FFFD`, recognizes every read-boundary split, forwards partial or mismatching prefixes byte-for-byte, and never delays ordinary Escape.
- Pausing restores detector-retained partial source before passthrough; resuming in the middle of a logical line waits through the next newline so it cannot manufacture a false semantic opener.
- ANSI/CSI, OSC, DCS/APC/PM, carriage-return redraw, parser-fail-closed state, and alternate-screen classification remain active while semantic rendering is paused.
- The toggle can suppress the selected baseline policy but cannot turn `--source` or `--safe` into a render-capable mode or re-enable cache under `--private`.

### Changed

- The broader guided-adoption, state-discovery, CJK/grapheme, and text-first usability plan moves to `v0.1.0-alpha.6` so this bounded interactive control can be reviewed and released independently.
- The prerelease remains source-only, with zero project-uploaded executable assets.

### Known limitations

- A renderer already executing when the key is pressed may finish before the new state applies.
- The default WezTerm binding targets the active pane; use it inside a Ptymark-hosted session or customize/disable it.
- The toggle does not persist state or inject a visual status line.

## [0.1.0-alpha.4] - 2026-08-03

### Added

- Typed TOML configuration schema v2 with named profiles, portable user intent, deterministic schema-v1 migration, and `--profile` selection.
- Canonical `ptymark shell -- COMMAND` interactive execution while retaining the Alpha.3 `ptymark -- COMMAND` compatibility form.
- Structural SVG validation, bounded maintained XML parsing, typed managed-bundle inspection, and standard platform-directory and executable resolution.

### Changed

- Resolved executable paths, managed-bundle ownership, and installation provenance now live in machine-local install state instead of being copied into portable user configuration.
- Hard process, parser, terminal-control, artifact, and pending-output limits are centralized as internal policy and cannot be weakened through user TOML.
- Hand-written CLI parsing, doctor JSON serialization, XML tokenization, temporary-directory allocation, and LRU recency bookkeeping were replaced with maintained crates where the product contract could be preserved.
- CI now validates product-owned Rust, installer, renderer, Docker, and release surfaces directly; inherited repository-template and generic automation suites were removed from the product gate.

### Fixed

- Config and install-state publication is recoverable as one ownership-scoped transaction, preventing mixed new/old installation metadata after a failed commit.
- Unterminated or oversized terminal control sequences are bounded and remain on the raw byte-exact path.
- Renderer output containing an incidental `<svg` substring is no longer accepted as a valid SVG artifact.
- Cache recency updates are O(1), source-bearing keys are not duplicated for ordering, and disabled caches no longer report policy misses.

### Compatibility

- Schema-v1 TOML remains readable and can be normalized with `ptymark config migrate`; schema-v2 is the canonical emitted form.
- Existing `ptymark -- COMMAND`, source, safe, private, strict, PTY/ConPTY, exact-source fallback, and source-only distribution contracts remain supported.

### Known limitations

- This remains an alpha release. Guided first-run setup, complete CJK/grapheme/accessibility behavior, persistent renderer workers/cache, signed package channels, and terminal image protocols remain follow-up work.

## [0.1.0-alpha.3] - 2026-08-02

### Added

- Explicit OpenMath XML fences with bounded local XML-to-TeX conversion, generic custom Content Dictionary presentation, exact-source fallback, and task-oriented documentation routes.
- The stable `PTYMARK_ACTIVE=1` child-environment marker for interactive Unix PTY and Windows ConPTY sessions.
- `--allow-nested` as an explicit development/debug escape hatch; accidental nested Ptymark sessions are rejected before configuration loading or child launch.
- Opt-in `[ptymark]` prompt examples without automatic shell-profile modification.

### Fixed

- Managed renderer installation now runs npm with the selected private Node runtime on `PATH` for that child process, so a clean Linux/WSL host does not require a global `node` command.
- Managed-bundle failures now state that the core binary may exist while configuration and installation state remain uncommitted.
- Successful renderer output in interactive sessions converts lone LF bytes to CRLF while preserving existing CRLF, child passthrough, terminal controls, and exact-source fallback. This keeps each generated row at terminal column zero.

### Changed

- Adopted a source-only distribution policy: GitHub Releases contain immutable tags, release notes, and GitHub-generated source snapshots, but no project-uploaded executables or installer archives.
- Cross-platform executable and package smoke tests remain required in CI, but their outputs are discarded instead of uploaded.
- Withdrew the project-uploaded executable assets, binary checksums, binary manifests, and binary attestations from `v0.1.0-alpha.1` and `v0.1.0-alpha.2`; tags and release notes remain immutable.

### Security

- Clarified that checksums and provenance do not replace operating-system code signing, notarization, reputation, revocation, or an approved package-manager trust path.
- Local source builds and third-party packages are not automatically trusted or endorsed by the project.
- Session visibility exposes only the stable active marker; nesting depth, parent PID, and other unstable process metadata are not public API.

### Known limitations

- Interactive semantic blocks must begin on a clean logical line. Prompt or shell-integration control bytes on that line are preserved as raw terminal output; emit a leading newline before a block when required.
- Guided setup, CJK/grapheme/accessibility completion, lifecycle commands, signed channels, persistent workers/cache, and image protocols remain later work.

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
