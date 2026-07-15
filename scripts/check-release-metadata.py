#!/usr/bin/env python3

# @dependency-start
# contract implementation
# responsibility Validates version, documentation, packaging, and workflow release metadata.
# upstream environment ../Cargo.toml package version
# upstream design ../documents/release.md release contract
# downstream implementation ../.github/workflows/ptymark-release.yml publication gate
# downstream implementation ../tests/tools/test_release_metadata.py metadata tests
# @dependency-end

"""Validate ptymark release metadata without changing the repository."""

from __future__ import annotations

import argparse
import re
import sys
import tomllib
from pathlib import Path

TAG_PATTERN = re.compile(r"^v[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?$")
FULL_COMMIT_SHA_PATTERN = r"[0-9a-f]{40}"


def package_version(root: Path) -> str:
    """Return the root Cargo package version."""
    with (root / "Cargo.toml").open("rb") as handle:
        data = tomllib.load(handle)
    value = data.get("package", {}).get("version")
    if not isinstance(value, str) or not value:
        raise ValueError("Cargo.toml does not declare package.version")
    return value


def validate(root: Path, tag: str | None = None) -> tuple[str, list[str]]:
    """Return the resolved version and deterministic validation failures."""
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

    lock_path = root / "Cargo.lock"
    try:
        with lock_path.open("rb") as handle:
            lock = tomllib.load(handle)
        root_packages = [
            package
            for package in lock.get("package", [])
            if package.get("name") == "ptymark"
        ]
        if len(root_packages) != 1:
            failures.append("Cargo.lock must contain exactly one ptymark package entry")
        elif root_packages[0].get("version") != version:
            failures.append("Cargo.lock ptymark version does not match Cargo.toml")
    except (OSError, tomllib.TOMLDecodeError) as error:
        failures.append(f"cannot read Cargo.lock: {error}")

    required_files = (
        "CHANGELOG.md",
        "SECURITY.md",
        "documents/release.md",
        "documents/troubleshooting.md",
        "scripts/build-release-manifest.py",
        ".github/workflows/ptymark-release.yml",
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
    if notes and "ptymark.doctor.v1" not in notes:
        failures.append(f"release notes do not document ptymark.doctor.v1: {notes_path}")

    release_workflow = _read_text(
        root / ".github/workflows/ptymark-release.yml", failures
    )
    for marker in (
        "scripts/check-release-metadata.py",
        "scripts/package-release.sh",
        "scripts/package-release.ps1",
        "gh release create",
    ):
        if release_workflow and marker not in release_workflow:
            failures.append(f"release workflow does not contain required marker: {marker}")

    for action, release_major in (
        ("actions/download-artifact", 8),
        ("actions/attest", 4),
    ):
        if release_workflow and not _has_full_sha_action_reference(
            release_workflow, action, release_major
        ):
            failures.append(
                "release workflow does not contain a full-SHA-pinned "
                f"{action} reference annotated with # v{release_major}"
            )

    for package_script in ("scripts/package-release.sh", "scripts/package-release.ps1"):
        content = _read_text(root / package_script, failures)
        for packaged_document in (
            "CHANGELOG.md",
            "SECURITY.md",
            "documents/release.md",
            "documents/troubleshooting.md",
            "release-notes",
        ):
            markers = (packaged_document, packaged_document.replace("/", "\\"))
            if content and not any(marker in content for marker in markers):
                failures.append(
                    f"{package_script} does not package {packaged_document}"
                )

    temporary_workflows = sorted((root / ".github/workflows").glob("*-once.yml"))
    for path in temporary_workflows:
        failures.append(
            f"temporary workflow is forbidden in a release tree: {path.relative_to(root)}"
        )
    if (root / ".release-candidate-placeholder").exists():
        failures.append("release-candidate placeholder is forbidden in a release tree")

    return version, failures


def _has_full_sha_action_reference(workflow: str, action: str, release_major: int) -> bool:
    """Return whether an Action is pinned to a full SHA with a major-version note."""
    pattern = re.compile(
        rf"^\s*uses:\s*{re.escape(action)}@{FULL_COMMIT_SHA_PATTERN}"
        rf"\s+#\s+v{release_major}(?:\.[0-9]+(?:\.[0-9]+)?)?\s*$",
        flags=re.MULTILINE,
    )
    return pattern.search(workflow) is not None


def _read_text(path: Path, failures: list[str]) -> str:
    """Read UTF-8 text and record a validation failure instead of raising."""
    try:
        return path.read_text(encoding="utf-8")
    except OSError as error:
        failures.append(f"cannot read {path}: {error}")
        return ""


def main(argv: list[str] | None = None) -> int:
    """Run the command-line validator."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--tag")
    arguments = parser.parse_args(argv)

    version, failures = validate(arguments.root.resolve(), arguments.tag)
    if failures:
        for failure in failures:
            print(f"release metadata error: {failure}", file=sys.stderr)
        return 1
    print(f"release metadata ok: version={version} tag={arguments.tag or f'v{version}'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
