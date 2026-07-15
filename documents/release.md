<!--
@dependency-start
contract design
responsibility Defines source-only release publication, version identity, and recovery behavior.
upstream environment ../Cargo.toml package version
downstream implementation ../.github/workflows/ptymark-release.yml source-only publication orchestration
downstream implementation ../scripts/check-release-metadata.py release-policy validator
@dependency-end
-->

# Source-only release and recovery contract

Ptymark uses a **source-only distribution policy**. The project does not publish project-built native executables, installer archives, renderer bundles, or executable-bearing workflow artifacts for end-user installation.

This policy responds to operating-system and enterprise controls that flag unsigned or unnotarized native downloads. SHA-256 records and build provenance establish identity, but they do not provide Apple Developer ID, Windows Authenticode, notarization, reputation, revocation, or an approved package-manager trust path.

Source-only does not mean risk-free. Users and downstream distributors remain responsible for reviewing the source, lockfile, toolchain, dependencies, build environment, and any locally produced executable.

## Public release surface

A ptymark GitHub Release contains only:

1. an immutable annotated version tag;
2. versioned release notes;
3. GitHub-generated source-code archives for that tag.

GitHub-generated `Source code (zip)` and `Source code (tar.gz)` links are repository snapshots. They are **not prebuilt executables** and are not produced by ptymark packaging scripts.

The project must not upload any of the following to a GitHub Release:

- native executables or executable archives;
- package-local installer archives;
- managed renderer bundles;
- adjacent checksums for executable archives;
- aggregate binary `SHA256SUMS` files;
- binary release manifests;
- build-provenance attestations whose subject is a distributed executable package.

## Release invariants

A source-only release is publishable only when:

1. `Cargo.toml`, the root `Cargo.lock` package entry, the changelog heading, release notes, request branch, and tag agree on the version;
2. the release source commit is the current reviewed `main` commit;
3. repository, Rust, PTY/ConPTY, renderer, installer, shell-coexistence, local-package, CodeQL, and dependency-review checks pass for that commit;
4. no temporary `*-once.yml` workflow or release-candidate placeholder remains;
5. the release workflow contains no binary build, package upload, binary manifest, or binary attestation step;
6. the created GitHub Release has zero project-uploaded assets.

The release workflow accepts only `release/v<version>` request branches. A request branch must point exactly at current `main`. The workflow creates the matching annotated tag, publishes notes only, verifies that the release has zero uploaded assets, and removes the request branch after success.

## Build and package verification

CI continues to compile and exercise native executables on Ubuntu, macOS, Windows, and the canonical Docker environment. It may assemble local packages to validate:

- release-mode compilation;
- package layout;
- local installer behavior;
- configuration and install-state generation;
- `--version`, doctor, source, safe, PTY/ConPTY, and preview smoke tests.

These outputs exist only in the runner's temporary workspace and are deleted before the job ends. Executable packages must not be uploaded as GitHub Actions artifacts or attached to Releases.

`scripts/package-release.sh`, `scripts/package-release.ps1`, and `scripts/build-release-manifest.py` are developer/CI verification utilities. Their output is not an official distribution channel.

## Build from source

Use a reviewed tag or commit and a locally controlled Rust toolchain:

```bash
git clone --recurse-submodules https://github.com/iwashita-nozomu/ptymark.git
cd ptymark
git checkout v<version>
cargo install --locked --path .
```

Platform installer frontends may also be run from the source checkout. They build or select a local ptymark executable and configure optional renderer dependencies; they do not convert the checkout into an official project-signed binary distribution.

## Existing alpha releases

The immutable `v0.1.0-alpha.1` and `v0.1.0-alpha.2` tags and release notes are retained. Project-uploaded executable archives, executable-package checksums, binary manifests, and related binary attestations are withdrawn. The GitHub-generated source snapshots remain available.

Removing uploaded assets does not rewrite or move the tags. Historical changelog entries continue to describe what was originally published and now carry a withdrawal notice.

## Support and diagnosis

The public support path remains:

```text
ptymark doctor
ptymark doctor --json
ptymark doctor --support-report PATH
```

The versioned schema is `ptymark.doctor.v1`. Default reports exclude semantic source, child environment, credentials, raw source-bearing renderer stderr, sensitive path prefixes, and terminal-control bytes. Security vulnerabilities and accidental credential exposure belong in a private Security Advisory.

## Recovery

Published source tags are immutable. A defective release is corrected by a higher version rather than by moving or rebuilding the tag. Users can return to an earlier reviewed source tag and rebuild locally.

Reintroducing official project-built binaries requires a separate reviewed decision covering, at minimum:

- platform code signing and key custody;
- macOS notarization and Windows signing/reputation;
- signed package-manager metadata;
- reproducible build and provenance policy;
- vulnerability response, revocation, and replacement procedures;
- supported upgrade, rollback, uninstall, and purge behavior.
