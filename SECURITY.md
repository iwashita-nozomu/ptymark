<!--
@dependency-start
contract policy
responsibility Defines supported source release lines, private vulnerability reporting, and distribution trust boundaries.
upstream design documents/release.md source-only release and recovery contract
upstream design documents/ptymark-design.md terminal safety and process boundary design
downstream implementation .github/workflows/ptymark-release.yml notes-only source publication
@dependency-end
-->

# Security Policy

## Supported versions

| Version | Support |
| --- | --- |
| `0.1.0-alpha.2` | Best-effort security fixes during the alpha period |
| Older prereleases | Unsupported except for coordinated disclosure or replacement guidance |
| Unreleased development branches | Not a supported distribution channel |

The newest published prerelease is the only supported alpha line.

## Source-only distribution policy

Ptymark does not publish project-built native executables, installer archives, renderer bundles, or executable-bearing GitHub Actions artifacts for end users. GitHub Releases retain immutable tags, release notes, and GitHub-generated source snapshots only.

The executable assets originally uploaded for `v0.1.0-alpha.1` and `v0.1.0-alpha.2` have been withdrawn. Their tags and source history remain unchanged.

This policy avoids presenting unsigned and unnotarized downloads as an operating-system-trusted channel. Checksums and provenance can establish artifact identity, but they do not replace platform signing, notarization, reputation, revocation, or package-manager trust.

Source availability is not a security guarantee. Users and downstream builders must evaluate the source, lockfile, toolchain, dependencies, build environment, and locally generated executable. The project does not endorse third-party binary packages unless a future policy names that channel explicitly.

## Reporting a vulnerability

Use the repository's private **Security advisories** reporting flow. Do not open a public issue for a vulnerability that could expose terminal contents, command arguments, local paths, renderer input, credentials, build secrets, or managed-bundle integrity details.

Include only the minimum information needed to reproduce the issue:

- affected ptymark version or commit and operating system;
- invocation mode (`preview`, `run`, native PTY/ConPTY, doctor, or installer);
- whether built-in, external, or managed renderers are selected;
- a redacted reproducer containing no secrets or private terminal source;
- the observed security-boundary violation and expected safe behavior.

The project will validate impact, coordinate a fix and disclosure, and publish a corrected source release when required. Exact response times are not guaranteed during the alpha period.

## Security boundaries

Ptymark treats terminal-control bytes, keyboard input, signals, child argument boundaries, semantic source, diagnostic redaction, and renderer process ownership as protected interfaces. Configuration values are data and are never evaluated as shell source. Normal rendering and default doctor perform no dependency installation or network access.

CI compiles and tests native executables across supported platforms, but executable outputs remain ephemeral and are not distributed. A future binary channel must complete the signing, notarization, lifecycle, and revocation work defined in `documents/release.md` before it can be described as supported.
