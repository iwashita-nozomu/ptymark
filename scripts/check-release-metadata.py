#!/usr/bin/env python3

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
