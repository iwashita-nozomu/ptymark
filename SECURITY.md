<!--
@dependency-start
contract policy
responsibility Defines alpha support, private vulnerability reporting, and source-only distribution trust boundaries.
upstream design documents/release.md source-only prerelease and recovery contract
upstream design documents/ptymark-design.md terminal safety and process boundary design
downstream implementation .github/workflows/ptymark-release.yml notes-only source prerelease publication
downstream implementation scripts/check-release-metadata.py prerelease status validation
@dependency-end
-->

# Security Policy

## Release status

Ptymark is still an **alpha prerelease**. The command-line interface, configuration schema, installer behavior, renderer ownership model, and lifecycle commands may change between alpha releases. Alpha releases receive best-effort security fixes, but they do not carry stable-API, long-term-support, upgrade, rollback, or binary-distribution guarantees.

A development branch or version declared in `Cargo.toml` is not a supported release until the matching immutable tag and GitHub prerelease have been published. The current publication workflow accepts only version strings of the form `alpha.N`, `beta.N`, or `rc.N`; publishing a stable version requires a separate reviewed policy change.

## Supported versions

| Version | Support |
| --- | --- |
| Newest published `0.1.0-alpha.*` prerelease | Best-effort security fixes during the alpha period |
| Older alpha prereleases | Unsupported except for coordinated disclosure or replacement guidance |
| Unreleased development branches | Not a supported distribution channel |

Only the newest published alpha prerelease is supported.

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

The project will validate impact, coordinate a fix and disclosure, and publish a corrected source prerelease when required. Exact response times are not guaranteed during the alpha period.

## Security boundaries

Ptymark treats terminal-control bytes, keyboard input, signals, child argument boundaries, semantic source, diagnostic redaction, and renderer process ownership as protected interfaces. Configuration values are data and are never evaluated as shell source. Normal rendering and default doctor perform no dependency installation or network access.

CI compiles and tests native executables across supported platforms, but executable outputs remain ephemeral and are not distributed. A future binary channel must complete the signing, notarization, lifecycle, and revocation work defined in `documents/release.md` before it can be described as supported.
