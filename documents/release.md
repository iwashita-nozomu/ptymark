<!--
@dependency-start
contract design
responsibility Defines immutable release publication, asset verification, and recovery behavior.
upstream environment ../Cargo.toml package version
upstream implementation ../scripts/check-release-metadata.py tree validation
upstream implementation ../scripts/build-release-manifest.py release metadata generation
downstream implementation ../.github/workflows/ptymark-release.yml publication orchestration
@dependency-end
-->

# Release and recovery contract

This document defines the release process for versioned ptymark prereleases. It is deliberately separate from future automatic upgrade, rollback, and uninstall commands.

## Release invariants

A release is publishable only when all of the following are true:

1. the version in `Cargo.toml`, the root `Cargo.lock` package entry, the changelog heading, and the Git tag agree;
2. the release source commit is the current reviewed `main` commit;
3. normal repository, product, package, PTY/ConPTY, renderer, and shell-coexistence checks pass for that commit;
4. no temporary `*-once.yml` workflow or release-candidate placeholder remains in the tree;
5. Linux, macOS, and Windows archives are rebuilt from the release commit rather than copied from a pull-request run;
6. every archive has a sidecar SHA-256 file, appears in `SHA256SUMS`, and is recorded in `release-manifest.json`;
7. GitHub build-provenance attestations are created before the GitHub Release is published.

The release workflow accepts an immutable version tag or a `release/v<version>` request branch. A request branch must point exactly at current `main`; the workflow creates the matching annotated tag before building and refuses to overwrite an existing release.

## Asset contract

Archive names are stable and include version, operating system, and architecture:

```text
ptymark-<version>-linux-<architecture>.tar.gz
ptymark-<version>-macos-<architecture>.tar.gz
ptymark-<version>-windows-<architecture>.zip
```

Each archive contains the executable, package-local installers, renderer metadata and entrypoints, the WezTerm plugin and example, user documentation, license, changelog, and security policy.

## Verification

Verify the downloaded archive against `SHA256SUMS` or its adjacent `.sha256` file. GitHub CLI users can additionally verify build provenance:

```bash
gh attestation verify ptymark-*.tar.gz --repo iwashita-nozomu/ptymark
```

```powershell
gh attestation verify .\ptymark-*.zip --repo iwashita-nozomu/ptymark
```

## Support and diagnosis

Every alpha.2 package contains `documents/troubleshooting.md`. The public support path is:

```text
ptymark doctor
ptymark doctor --json
ptymark doctor --support-report PATH
```

The versioned schema is `ptymark.doctor.v1`. Default reports are designed for public attachment and exclude semantic source, child environment, credentials, raw source-bearing renderer stderr, sensitive path prefixes, and terminal-control bytes. Doctor performs no installation, download, network access, renderer/browser execution, child launch, or mutation by default.

A support report is supplemental evidence, not an upload mechanism. Users review and attach it explicitly. Security vulnerabilities and accidental credential exposure remain private Security Advisory reports.

## Recovery and rollback

Published assets are immutable. A defective release is not rebuilt under the same tag. Instead:

1. mark the affected release notes with the known problem;
2. publish a corrected higher version;
3. retain the previous known-good versioned assets;
4. reinstall the previous archive when immediate rollback is required;
5. preserve user-owned configuration unless a documented schema migration requires an explicit change.

The package-local installers never edit shell profiles. Automated upgrade, owned-file uninstall, purge, and previous-installation restoration remain tracked by the lifecycle issue; their absence is called out in alpha release notes rather than hidden behind an unstable command.
