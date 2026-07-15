#!/usr/bin/env python3

from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def write(path: str, content: str) -> None:
    target = ROOT / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(content.rstrip() + "\n", encoding="utf-8")


def replace_once(path: str, old: str, new: str) -> None:
    target = ROOT / path
    text = target.read_text(encoding="utf-8")
    if text.count(old) != 1:
        raise SystemExit(f"expected exactly one match in {path}: {old[:100]!r}")
    target.write_text(text.replace(old, new), encoding="utf-8")


def regex_once(path: str, pattern: str, replacement: str, flags: int = 0) -> None:
    target = ROOT / path
    text = target.read_text(encoding="utf-8")
    updated, count = re.subn(pattern, replacement, text, count=1, flags=flags)
    if count != 1:
        raise SystemExit(f"expected one regex match in {path}: {pattern!r}")
    target.write_text(updated, encoding="utf-8")


write(
    ".github/workflows/ptymark-release.yml",
    r'''# @dependency-start
# contract workflow
# responsibility Creates immutable version tags and notes-only source releases without publishing project-built executables.
# upstream design ../../documents/release.md source-only release and recovery contract
# upstream implementation ../../scripts/check-release-metadata.py source-only publication validator
# downstream design ../../SECURITY.md distribution and vulnerability-reporting policy
# @dependency-end

name: ptymark Source Release

on:
  push:
    branches:
      - 'release/v*'

concurrency:
  group: ptymark-source-release-${{ github.ref }}
  cancel-in-progress: false

permissions:
  contents: write

jobs:
  publish:
    name: Validate, tag, and publish source-only release
    runs-on: ubuntu-24.04
    timeout-minutes: 15
    environment: production-release
    steps:
      - name: Check out reviewed source commit
        uses: actions/checkout@9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0 # v7
        with:
          fetch-depth: 0
          submodules: false
          persist-credentials: false

      - name: Check out AgentCanon submodule
        env:
          AGENT_CANON_REPO_TOKEN: ''
          AGENT_CANON_REPO_SSH_KEY: ''
        run: bash .github/scripts/checkout_agent_canon_submodule.sh

      - name: Resolve source-only release identity
        id: release
        shell: bash
        run: |
          set -euo pipefail
          version=$(python3 -c 'import tomllib; print(tomllib.load(open("Cargo.toml", "rb"))["package"]["version"])')
          expected_branch="release/v${version}"
          expected_tag="v${version}"
          test "$GITHUB_REF_NAME" = "$expected_branch" || {
            echo "release request branch $GITHUB_REF_NAME does not match Cargo version $version" >&2
            exit 1
          }
          git fetch origin main --depth=1
          source_sha=$(git rev-parse HEAD)
          test "$source_sha" = "$(git rev-parse origin/main)" || {
            echo 'source-only release request branch must point exactly at current main' >&2
            exit 1
          }
          python3 scripts/check-release-metadata.py --tag "$expected_tag"
          echo "version=$version" >> "$GITHUB_OUTPUT"
          echo "tag=$expected_tag" >> "$GITHUB_OUTPUT"
          echo "source_sha=$source_sha" >> "$GITHUB_OUTPUT"

      - name: Create immutable annotated tag
        env:
          GH_TOKEN: ${{ github.token }}
          RELEASE_TAG: ${{ steps.release.outputs.tag }}
          SOURCE_SHA: ${{ steps.release.outputs.source_sha }}
        shell: bash
        run: |
          set -euo pipefail
          if git ls-remote --exit-code --tags origin "refs/tags/$RELEASE_TAG" >/dev/null 2>&1; then
            git fetch origin "refs/tags/$RELEASE_TAG:refs/tags/$RELEASE_TAG"
            test "$(git rev-list -n 1 "$RELEASE_TAG")" = "$SOURCE_SHA" || {
              echo "existing tag $RELEASE_TAG points at a different commit" >&2
              exit 1
            }
          else
            git config user.name github-actions[bot]
            git config user.email 41898282+github-actions[bot]@users.noreply.github.com
            git tag -a "$RELEASE_TAG" "$SOURCE_SHA" -m "ptymark $RELEASE_TAG"
            git push \
              "https://x-access-token:${GH_TOKEN}@github.com/${GITHUB_REPOSITORY}.git" \
              "refs/tags/$RELEASE_TAG"
          fi

      - name: Publish notes-only GitHub prerelease
        env:
          GH_TOKEN: ${{ github.token }}
          RELEASE_TAG: ${{ steps.release.outputs.tag }}
          VERSION: ${{ steps.release.outputs.version }}
        shell: bash
        run: |
          set -euo pipefail
          if gh release view "$RELEASE_TAG" >/dev/null 2>&1; then
            echo "release $RELEASE_TAG already exists; refusing to overwrite it" >&2
            exit 1
          fi
          gh release create "$RELEASE_TAG" \
            --verify-tag \
            --prerelease \
            --title "ptymark $RELEASE_TAG" \
            --notes-file "release-notes/${VERSION}.md"
          test "$(gh release view "$RELEASE_TAG" --json assets --jq '.assets | length')" = '0'

      - name: Remove completed release request branch
        env:
          GH_TOKEN: ${{ github.token }}
          REPOSITORY: ${{ github.repository }}
          BRANCH: ${{ github.ref_name }}
        shell: bash
        run: |
          set -euo pipefail
          encoded=${BRANCH//\//%2F}
          gh api --method DELETE "repos/${REPOSITORY}/git/refs/heads/${encoded}"
''',
)

write(
    "documents/release.md",
    r'''<!--
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
''',
)

write(
    "SECURITY.md",
    r'''<!--
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
''',
)

write(
    "scripts/check-release-metadata.py",
    r'''#!/usr/bin/env python3

# @dependency-start
# contract implementation
# responsibility Validates version metadata and enforces source-only GitHub Release publication.
# upstream environment ../Cargo.toml package version
# upstream design ../documents/release.md source-only release contract
# downstream implementation ../.github/workflows/ptymark-release.yml notes-only publication
# downstream implementation ../tests/tools/test_release_metadata.py metadata tests
# @dependency-end

"""Validate ptymark source-only release metadata without changing the repository."""

from __future__ import annotations

import argparse
import re
import sys
import tomllib
from pathlib import Path

TAG_PATTERN = re.compile(r"^v[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?$")


def package_version(root: Path) -> str:
    with (root / "Cargo.toml").open("rb") as handle:
        data = tomllib.load(handle)
    value = data.get("package", {}).get("version")
    if not isinstance(value, str) or not value:
        raise ValueError("Cargo.toml does not declare package.version")
    return value


def _read_text(path: Path, failures: list[str]) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except OSError as error:
        failures.append(f"cannot read {path}: {error}")
        return ""


def validate(root: Path, tag: str | None = None) -> tuple[str, list[str]]:
    failures: list[str] = []
    try:
        version = package_version(root)
    except (OSError, tomllib.TOMLDecodeError, ValueError) as error:
        return "unknown", [str(error)]

    expected_tag = f"v{version}"
    if tag is not None:
        if not TAG_PATTERN.fullmatch(tag):
            failures.append(f"release tag has an unsupported format: {tag}")
        if tag != expected_tag:
            failures.append(f"release tag {tag} does not match Cargo version {version}")

    try:
        with (root / "Cargo.lock").open("rb") as handle:
            lock = tomllib.load(handle)
        packages = [p for p in lock.get("package", []) if p.get("name") == "ptymark"]
        if len(packages) != 1:
            failures.append("Cargo.lock must contain exactly one ptymark package entry")
        elif packages[0].get("version") != version:
            failures.append("Cargo.lock ptymark version does not match Cargo.toml")
    except (OSError, tomllib.TOMLDecodeError) as error:
        failures.append(f"cannot read Cargo.lock: {error}")

    required_files = (
        "CHANGELOG.md",
        "SECURITY.md",
        "README.md",
        "documents/release.md",
        "documents/troubleshooting.md",
        ".github/workflows/ptymark-release.yml",
        ".github/workflows/ptymark-ci.yml",
    )
    for relative in required_files:
        if not (root / relative).is_file():
            failures.append(f"required release file is missing: {relative}")

    changelog = _read_text(root / "CHANGELOG.md", failures)
    if changelog and not re.search(
        rf"^## \[{re.escape(version)}\] - \d{{4}}-\d{{2}}-\d{{2}}$",
        changelog,
        flags=re.MULTILINE,
    ):
        failures.append(f"CHANGELOG.md has no dated section for {version}")

    notes_path = root / "release-notes" / f"{version}.md"
    notes = _read_text(notes_path, failures)
    if notes and not notes.startswith(f"# ptymark v{version}\n"):
        failures.append(f"release notes heading does not identify v{version}: {notes_path}")
    if notes and "source-only" not in notes.lower():
        failures.append(f"release notes do not document source-only distribution: {notes_path}")

    for relative in ("README.md", "SECURITY.md", "documents/release.md"):
        content = _read_text(root / relative, failures)
        if content and "source-only" not in content.lower():
            failures.append(f"{relative} does not document the source-only policy")

    workflow = _read_text(root / ".github/workflows/ptymark-release.yml", failures)
    required_markers = (
        "release/v*",
        "gh release create",
        "--notes-file",
        "assets --jq '.assets | length'",
        "source-only",
    )
    for marker in required_markers:
        if workflow and marker not in workflow:
            failures.append(f"source release workflow is missing required marker: {marker}")

    forbidden_markers = (
        "cargo build",
        "scripts/package-release",
        "actions/upload-artifact",
        "actions/download-artifact",
        "actions/attest",
        "release-manifest.json",
        "SHA256SUMS",
        "dist/*",
    )
    for marker in forbidden_markers:
        if workflow and marker in workflow:
            failures.append(f"source release workflow contains forbidden binary marker: {marker}")

    product_ci = _read_text(root / ".github/workflows/ptymark-ci.yml", failures)
    if product_ci and "Cross-platform local package smoke" not in product_ci:
        failures.append("product CI does not identify local package smoke as non-distribution evidence")
    for marker in ("dist/*.tar.gz", "dist/*.zip", "Upload executable package"):
        if product_ci and marker in product_ci:
            failures.append(f"product CI exposes a downloadable executable package marker: {marker}")

    for package_script in ("scripts/package-release.sh", "scripts/package-release.ps1"):
        content = _read_text(root / package_script, failures)
        if content and "developer/ci verification only" not in content.lower():
            failures.append(f"{package_script} is not marked as developer/CI verification only")

    temporary_workflows = sorted((root / ".github/workflows").glob("*-once.yml"))
    for path in temporary_workflows:
        failures.append(
            f"temporary workflow is forbidden in a release tree: {path.relative_to(root)}"
        )
    if (root / ".release-candidate-placeholder").exists():
        failures.append("release-candidate placeholder is forbidden in a release tree")

    return version, failures


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--tag")
    arguments = parser.parse_args(argv)
    version, failures = validate(arguments.root.resolve(), arguments.tag)
    if failures:
        for failure in failures:
            print(f"release metadata error: {failure}", file=sys.stderr)
        return 1
    print(f"source-only release metadata ok: version={version} tag={arguments.tag or f'v{version}'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
''',
)

write(
    "tests/tools/test_release_metadata.py",
    r'''# @dependency-start
# contract test
# responsibility Verifies version consistency and the source-only release/publication policy.
# upstream implementation ../../scripts/check-release-metadata.py source-only validator
# upstream design ../../documents/release.md source-only release contract
# downstream environment ../../.github/workflows/ptymark-release.yml notes-only publication
# @dependency-end

"""Source-only release metadata contract tests."""

from __future__ import annotations

import subprocess
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


class ReleaseMetadataTest(unittest.TestCase):
    def test_release_tree_metadata_is_consistent(self) -> None:
        result = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts/check-release-metadata.py"),
                "--tag",
                "v0.1.0-alpha.2",
            ],
            cwd=ROOT,
            check=True,
            capture_output=True,
            text=True,
        )
        self.assertIn("source-only release metadata ok", result.stdout)

    def test_release_workflow_publishes_notes_without_project_assets(self) -> None:
        workflow = (ROOT / ".github/workflows/ptymark-release.yml").read_text()
        self.assertIn("gh release create", workflow)
        self.assertIn("--notes-file", workflow)
        self.assertIn(".assets | length", workflow)
        for forbidden in (
            "cargo build",
            "scripts/package-release",
            "actions/upload-artifact",
            "actions/download-artifact",
            "actions/attest",
            "release-manifest.json",
            "SHA256SUMS",
            "dist/*",
        ):
            self.assertNotIn(forbidden, workflow)

    def test_product_ci_keeps_package_smoke_ephemeral(self) -> None:
        workflow = (ROOT / ".github/workflows/ptymark-ci.yml").read_text()
        self.assertIn("Cross-platform local package smoke", workflow)
        self.assertIn("Discard local package output", workflow)
        self.assertNotIn("Upload executable package", workflow)
        self.assertNotIn("dist/*.tar.gz", workflow)
        self.assertNotIn("dist/*.zip", workflow)

    def test_local_packagers_are_not_a_distribution_channel(self) -> None:
        for relative in ("scripts/package-release.sh", "scripts/package-release.ps1"):
            content = (ROOT / relative).read_text(encoding="utf-8").lower()
            self.assertIn("developer/ci verification only", content)


if __name__ == "__main__":
    unittest.main()
''',
)

readme_policy = r'''## Distribution policy: source only

Ptymark does **not** publish project-built native executables, installer archives, renderer bundles, or executable-bearing GitHub Actions artifacts for end-user installation. GitHub Releases retain an immutable tag, release notes, and GitHub-generated source-code archives only.

The GitHub `Source code (zip)` and `Source code (tar.gz)` links are source snapshots, not prebuilt executables. Build locally from a reviewed tag or commit. This avoids presenting unsigned and unnotarized native downloads as an operating-system-trusted channel; it does not make a local build automatically safe.

The executable assets originally uploaded for `v0.1.0-alpha.1` and `v0.1.0-alpha.2` have been withdrawn. Their tags, release notes, and source history remain.

The complete policy and requirements for any future signed binary channel are documented in [`documents/release.md`](documents/release.md).

## Build and install from a source checkout'''
regex_once(
    "README.md",
    r"## Install from a versioned GitHub Release\n.*?## Install from a source checkout",
    readme_policy,
    flags=re.DOTALL,
)
replace_once(
    "README.md",
    "- release executable packages for Linux, macOS, and Windows in GitHub Actions;",
    "- cross-platform local executable/package smoke tests whose outputs are discarded rather than distributed;",
)
replace_once(
    "README.md",
    "- supported upgrade, rollback, uninstall, purge, signing, and package-manager channels.",
    "- supported upgrade, rollback, uninstall, purge, and any approved signed binary/package-manager channel.",
)
replace_once(
    "README.md",
    "Build a release package locally after a release build:",
    "Build a local package for developer/CI verification after a release build. These outputs are not an official distribution channel:",
)
replace_once(
    "README.md",
    "- Linux, macOS, and Windows release-package creation, package-local installation, configuration\n  validation, preview smoke, checksums, and artifact upload;",
    "- Linux, macOS, and Windows local package assembly, package-local installation, configuration\n  validation, and preview smoke; executable outputs are discarded and never uploaded;",
)

changelog = (ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
unreleased = """## [Unreleased]\n\n### Changed\n\n- Adopted a source-only distribution policy: GitHub Releases contain immutable tags, release notes, and GitHub-generated source snapshots, but no project-uploaded executables or installer archives.\n- Cross-platform executable and package smoke tests remain required in CI, but their outputs are discarded instead of uploaded.\n- Withdrew the project-uploaded executable assets, binary checksums, binary manifests, and binary attestations from `v0.1.0-alpha.1` and `v0.1.0-alpha.2`; tags and release notes remain immutable.\n\n### Security\n\n- Clarified that checksums and provenance do not replace operating-system code signing, notarization, reputation, revocation, or an approved package-manager trust path.\n- Local source builds and third-party packages are not automatically trusted or endorsed by the project.\n"""
if changelog.count("## [Unreleased]\n") != 1:
    raise SystemExit("unexpected Unreleased heading count")
changelog = changelog.replace("## [Unreleased]\n", unreleased, 1)
changelog = changelog.replace(
    "- This remains an unsigned alpha prerelease.\n",
    "- Project-uploaded executable assets for this release were withdrawn on 2026-07-15; the tag and release notes remain available under the source-only policy.\n",
    1,
)
alpha1_marker = "### Known limitations\n\n- Release archives are not yet signed with Apple Developer ID or Windows Authenticode certificates."
alpha1_replacement = "### Known limitations\n\n- Project-uploaded executable assets for this release were withdrawn on 2026-07-15; the tag and release notes remain available under the source-only policy.\n- The originally published archives were not signed with Apple Developer ID or Windows Authenticode certificates."
if alpha1_marker not in changelog:
    raise SystemExit("alpha.1 known-limitations marker not found")
changelog = changelog.replace(alpha1_marker, alpha1_replacement, 1)
(ROOT / "CHANGELOG.md").write_text(changelog, encoding="utf-8")

for relative in ("release-notes/0.1.0-alpha.1.md", "release-notes/0.1.0-alpha.2.md"):
    path = ROOT / relative
    text = path.read_text(encoding="utf-8")
    first, rest = text.split("\n", 1)
    notice = """
> **Source-only distribution notice (2026-07-15):** project-uploaded native executables, installer archives, binary checksums, binary manifests, and binary attestations for this tag have been withdrawn. The immutable tag, release notes, and GitHub-generated source snapshots remain. Build locally from the reviewed tag; no project-built executable download is an official channel.
"""
    if "Source-only distribution notice" not in text:
        text = first + "\n" + notice + rest
    text = text.replace(
        "- This remains an unsigned alpha prerelease distributed as Linux, macOS, and Windows archives.",
        "- This tag is retained as a source-only prerelease; previously uploaded Linux, macOS, and Windows executable archives have been withdrawn.",
    )
    path.write_text(text, encoding="utf-8")

for relative in (".github/ISSUE_TEMPLATE/bug-report.yml", ".github/ISSUE_TEMPLATE/terminal-compatibility.yml"):
    path = ROOT / relative
    text = path.read_text(encoding="utf-8")
    text = text.replace(
        "        - GitHub prerelease archive\n        - source checkout\n        - development commit or pull request\n        - other",
        "        - GitHub tag or GitHub-generated source archive\n        - source checkout\n        - downstream or locally built package\n        - development commit or pull request\n        - other",
    )
    text = text.replace("        - packaged installer smoke", "        - local source-build/install smoke")
    path.write_text(text, encoding="utf-8")

issue_test = ROOT / "tests/issue_form_contract.rs"
text = issue_test.read_text(encoding="utf-8")
needle = '        assert!(form.contains("raw renderer stderr"), "form={name}");\n'
insert = needle + '        assert!(form.contains("GitHub-generated source archive"), "form={name}");\n        assert!(!form.contains("GitHub prerelease archive"), "form={name}");\n'
if text.count(needle) != 1:
    raise SystemExit("issue form assertion insertion point not found")
issue_test.write_text(text.replace(needle, insert), encoding="utf-8")

for relative in ("scripts/package-release.sh", "scripts/package-release.ps1"):
    path = ROOT / relative
    text = path.read_text(encoding="utf-8")
    marker = "Developer/CI verification only: ptymark does not publish the executable output of this script."
    if marker not in text:
        if text.startswith("#!/"):
            first, rest = text.split("\n", 1)
            text = first + "\n# " + marker + "\n" + rest
        else:
            text = "# " + marker + "\n" + text
    path.write_text(text, encoding="utf-8")

manifest_builder = ROOT / "scripts/build-release-manifest.py"
text = manifest_builder.read_text(encoding="utf-8")
marker = "# Developer/CI verification only: binary manifests are not published by ptymark.\n"
if marker not in text:
    text = text.replace("#!/usr/bin/env python3\n", "#!/usr/bin/env python3\n\n" + marker, 1)
manifest_builder.write_text(text, encoding="utf-8")

ptymark_ci = ROOT / ".github/workflows/ptymark-ci.yml"
text = ptymark_ci.read_text(encoding="utf-8")
text = text.replace("  release-packages:\n    name: Build and smoke-test release package (${{ matrix.label }})", "  local-package-smoke:\n    name: Cross-platform local package smoke (${{ matrix.label }})")
step_pattern = re.compile(
    r"\n      - name: Upload executable package\n        uses: actions/upload-artifact@v7\n        with:\n          name: ptymark-\$\{\{ matrix\.label \}\}-release\n          path: \|\n            dist/\*\.tar\.gz\n            dist/\*\.tar\.gz\.sha256\n            dist/\*\.zip\n            dist/\*\.zip\.sha256\n          if-no-files-found: error\n          retention-days: 30\n"
)
replacement = """
      - name: Discard local package output
        shell: bash
        run: |
          set -euo pipefail
          test -d dist
          rm -rf dist
          test ! -e dist
"""
text, count = step_pattern.subn("\n" + replacement, text, count=1)
if count != 1:
    raise SystemExit("release package upload step not found")
ptymark_ci.write_text(text, encoding="utf-8")

verification_readme = ROOT / "verification/README.md"
text = verification_readme.read_text(encoding="utf-8")
text = text.replace(
    "| `package` | Native release executable is built, archived, checksummed, extracted, installed, and smoke-tested. |",
    "| `package` | A native executable is built and packaged in an ephemeral CI workspace, locally installed and smoke-tested, then discarded without upload or distribution. |",
)
text = text.replace("### Scripts and release packages", "### Scripts and ephemeral local package smoke")
text = text.replace("- `release.linux`\n- `release.macos`\n- `release.windows`", "- `package-smoke.linux`\n- `package-smoke.macos`\n- `package-smoke.windows`")
text = text.replace(
    "- `ptymark-linux-release`\n- `ptymark-macos-release`\n- `ptymark-windows-release`",
    "Executable package outputs are intentionally not uploaded. Only bounded diagnostic logs or non-executable evidence may be retained as workflow artifacts.",
)
text = text.replace(
    "Artifacts support diagnosis and distribution, but the committed tests and manifest define the contract.",
    "Non-executable artifacts may support diagnosis, but the committed tests and manifest define the contract. Project-built executable artifacts are not a distribution channel.",
)
verification_readme.write_text(text, encoding="utf-8")

manifest = ROOT / "verification/manifest.toml"
text = manifest.read_text(encoding="utf-8")
text = text.replace('id = "release.linux"', 'id = "package-smoke.linux"')
text = text.replace('id = "release.macos"', 'id = "package-smoke.macos"')
text = text.replace('id = "release.windows"', 'id = "package-smoke.windows"')
text = text.replace('evidence = ["ptymark-linux-release"]', 'evidence = ["Cross-platform local package smoke (linux)"]')
text = text.replace('evidence = ["ptymark-macos-release"]', 'evidence = ["Cross-platform local package smoke (macos)"]')
text = text.replace('evidence = ["ptymark-windows-release"]', 'evidence = ["Cross-platform local package smoke (windows)"]')
manifest.write_text(text, encoding="utf-8")

contract = ROOT / "tests/verification_manifest_contract.rs"
text = contract.read_text(encoding="utf-8")
text = text.replace('    "release.linux",\n    "release.macos",\n    "release.windows",', '    "package-smoke.linux",\n    "package-smoke.macos",\n    "package-smoke.windows",')
contract.write_text(text, encoding="utf-8")

design = ROOT / "documents/ptymark-design.md"
text = design.read_text(encoding="utf-8")
new_section = r'''## 12. Source distribution and local package verification

GitHub Releases are source-only. The public release surface consists of an immutable tag, release notes, and GitHub-generated source snapshots. Ptymark does not upload project-built executables, installer archives, renderer bundles, binary checksums, binary manifests, or binary attestations.

CI still builds native executables on Ubuntu, macOS, and Windows. Local package jobs assemble the same package layout in a temporary workspace, run package-local installation, configuration, doctor, version, and preview smoke tests, then delete the output. No executable package is uploaded as a workflow artifact.

`scripts/package-release.sh` and `scripts/package-release.ps1` remain developer/CI verification utilities. They do not define a supported distribution channel. Platform signing, notarization, package-manager trust, lifecycle, and revocation requirements must be approved before any future official binary channel is introduced.

'''
text, count = re.subn(r"## 12\. Release package contract\n.*?(?=## 13\.)", new_section, text, count=1, flags=re.DOTALL)
if count != 1:
    raise SystemExit("design release package section not found")
text = text.replace(
    "release packages\n  Linux, macOS, and Windows release executable\n  package-local installer\n  config validation and preview smoke\n  archive and SHA-256 artifact",
    "ephemeral local package smoke\n  Linux, macOS, and Windows release-mode executable\n  package-local installer\n  config validation, doctor, and preview smoke\n  output deletion with no executable artifact upload",
)
design.write_text(text, encoding="utf-8")

print("source-only policy rewrite complete")
