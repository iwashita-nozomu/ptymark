#!/usr/bin/env python3

# Developer/CI verification only: binary manifests are not published by ptymark.

# @dependency-start
# contract implementation
# responsibility Verifies archives and generates checksums, notes, and machine-readable metadata.
# upstream environment ../Cargo.toml package version
# upstream environment ../renderers/managed-bundle.env managed compatibility versions
# upstream design ../documents/release.md asset contract
# downstream implementation ../.github/workflows/ptymark-release.yml publication orchestration
# downstream implementation ../tests/tools/test_release_metadata.py manifest tests
# @dependency-end

"""Verify release archives and write machine-readable release metadata."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import tomllib
from pathlib import Path

ARCHIVE_PATTERN = re.compile(
    r"^ptymark-(?P<version>.+)-(?P<platform>linux|macos|windows)-"
    r"(?P<architecture>x86_64|aarch64)\.(?P<extension>tar\.gz|zip)$"
)


def sha256(path: Path) -> str:
    """Return a lowercase SHA-256 digest for a file."""
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def cargo_version(root: Path) -> str:
    """Read package.version from Cargo.toml."""
    with (root / "Cargo.toml").open("rb") as handle:
        return str(tomllib.load(handle)["package"]["version"])


def rust_constant(path: Path, name: str) -> int:
    """Read one public u32 constant from a Rust source file."""
    text = path.read_text(encoding="utf-8")
    match = re.search(rf"pub const {re.escape(name)}: u32 = (\d+);", text)
    if match is None:
        raise ValueError(f"cannot find {name} in {path}")
    return int(match.group(1))


def managed_versions(path: Path) -> dict[str, str | int]:
    """Parse the committed managed-bundle environment contract."""
    result: dict[str, str | int] = {}
    for raw_line in path.read_text(encoding="utf-8").splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        key, value = line.split("=", 1)
        public_key = key.removeprefix("PTYMARK_MANAGED_").lower()
        result[public_key] = int(value) if value.isdigit() else value
    return result


def changelog_notes(path: Path, version: str) -> str:
    """Extract one release section, excluding the heading itself."""
    text = path.read_text(encoding="utf-8")
    pattern = re.compile(
        rf"^## \[{re.escape(version)}\] - \d{{4}}-\d{{2}}-\d{{2}}\n"
        rf"(?P<body>.*?)(?=^## \[|\Z)",
        flags=re.MULTILINE | re.DOTALL,
    )
    match = pattern.search(text)
    if match is None:
        raise ValueError(f"CHANGELOG.md has no section for {version}")
    return match.group("body").strip() + "\n"


def build_manifest(
    root: Path, dist: Path, tag: str, commit: str
) -> tuple[dict[str, object], list[tuple[str, str]]]:
    """Verify all archives and return manifest data plus aggregate checksums."""
    version = cargo_version(root)
    if tag != f"v{version}":
        raise ValueError(f"tag {tag} does not match Cargo version {version}")

    assets: list[dict[str, object]] = []
    platforms: set[str] = set()
    aggregate: list[tuple[str, str]] = []
    for archive in sorted(dist.iterdir()):
        match = ARCHIVE_PATTERN.fullmatch(archive.name)
        if match is None:
            continue
        if match.group("version") != version:
            raise ValueError(f"archive version does not match release: {archive.name}")
        digest = sha256(archive)
        sidecar = archive.with_name(f"{archive.name}.sha256")
        expected_line = f"{digest}  {archive.name}"
        actual_line = sidecar.read_text(encoding="utf-8").strip()
        if actual_line != expected_line:
            raise ValueError(f"checksum sidecar does not match archive: {sidecar.name}")
        platform = match.group("platform")
        if platform in platforms:
            raise ValueError(f"multiple archives found for platform: {platform}")
        platforms.add(platform)
        assets.append(
            {
                "name": archive.name,
                "platform": platform,
                "architecture": match.group("architecture"),
                "bytes": archive.stat().st_size,
                "sha256": digest,
                "checksum_asset": sidecar.name,
            }
        )
        aggregate.append((digest, archive.name))

    expected_platforms = {"linux", "macos", "windows"}
    if platforms != expected_platforms:
        missing = ", ".join(sorted(expected_platforms - platforms)) or "none"
        extra = ", ".join(sorted(platforms - expected_platforms)) or "none"
        raise ValueError(f"release platform set mismatch; missing={missing} extra={extra}")

    manifest: dict[str, object] = {
        "schema_version": 1,
        "project": "ptymark",
        "version": version,
        "tag": tag,
        "prerelease": "-" in version,
        "source": {
            "repository": "iwashita-nozomu/ptymark",
            "commit": commit,
        },
        "compatibility": {
            "config_schema": rust_constant(
                root / "src/config.rs", "CONFIG_SCHEMA_VERSION"
            ),
            "install_state_schema": rust_constant(
                root / "src/install.rs", "INSTALL_STATE_SCHEMA_VERSION"
            ),
            "managed_bundle_schema": rust_constant(
                root / "src/managed_launcher.rs", "MANAGED_BUNDLE_SCHEMA_VERSION"
            ),
        },
        "managed_bundle": managed_versions(root / "renderers/managed-bundle.env"),
        "assets": assets,
    }
    return manifest, aggregate


def main() -> int:
    """Run release manifest generation."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--dist", type=Path, required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--checksums-output", type=Path, required=True)
    parser.add_argument("--notes-output", type=Path, required=True)
    arguments = parser.parse_args()

    root = arguments.root.resolve()
    dist = arguments.dist.resolve()
    manifest, aggregate = build_manifest(root, dist, arguments.tag, arguments.commit)
    arguments.output.write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    aggregate.append((sha256(arguments.output), arguments.output.name))
    arguments.checksums_output.write_text(
        "".join(f"{digest}  {name}\n" for digest, name in sorted(aggregate)),
        encoding="utf-8",
    )
    release_notes_path = root / "release-notes" / f"{manifest['version']}.md"
    release_notes = release_notes_path.read_text(encoding="utf-8")
    if f"v{manifest['version']}" not in release_notes.splitlines()[0]:
        raise ValueError(
            f"release notes heading does not identify v{manifest['version']}: {release_notes_path}"
        )
    arguments.notes_output.write_text(release_notes.rstrip() + "\n", encoding="utf-8")
    print(f"release manifest ok: {arguments.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
