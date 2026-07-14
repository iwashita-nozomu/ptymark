<!--
@dependency-start
contract policy
responsibility Defines supported release lines, private vulnerability reporting, and protected security boundaries.
upstream design documents/release.md release support and replacement contract
upstream design documents/ptymark-design.md terminal safety and process boundary design
downstream implementation .github/workflows/ptymark-release.yml replacement release publication
@dependency-end
-->

# Security Policy

## Supported versions

| Version | Support |
| --- | --- |
| `0.1.0-alpha.1` | Best-effort security fixes during the alpha period |
| Unreleased development branches | Not a supported distribution channel |

The newest published prerelease is the only supported alpha line. A superseded prerelease may receive a replacement release when a high-impact vulnerability prevents safe upgrade.

## Reporting a vulnerability

Use the repository's private **Security advisories** reporting flow. Do not open a public issue for a vulnerability that could expose terminal contents, command arguments, local paths, renderer input, credentials, or managed-bundle integrity details.

Include only the minimum information needed to reproduce the issue:

- affected ptymark version and operating system;
- invocation mode (`preview`, `run`, or native PTY/ConPTY);
- whether built-in, external, or managed renderers are selected;
- a redacted reproducer that contains no secrets or private terminal source;
- the observed security boundary violation and expected safe behavior.

The project will acknowledge the report, validate impact, coordinate a fix and disclosure, and publish a replacement release when required. Exact response times are not guaranteed during the alpha period.

## Security boundaries

ptymark treats terminal control bytes, keyboard input, signals, and child argument boundaries as protected interfaces. Configuration values are data and are never evaluated as shell source. Normal rendering must not download or install dependencies. Release assets are accompanied by SHA-256 checksums and GitHub build-provenance attestations; operating-system code signing is tracked separately before stable release.
