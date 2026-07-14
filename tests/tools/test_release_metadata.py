# @dependency-start
# contract test
# responsibility Verifies version consistency and machine-readable release assets.
# upstream implementation ../../scripts/check-release-metadata.py release tree validator
# upstream implementation ../../scripts/build-release-manifest.py release manifest generator
# upstream design ../../documents/release.md immutable release and recovery contract
# @dependency-end
"""Release metadata and manifest contract tests."""

from __future__ import annotations

import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from typing import cast

ROOT = Path(__file__).resolve().parents[2]


class ReleaseMetadataTest(unittest.TestCase):
    """Exercise the release validators through their public command-line surface."""

    def test_release_tree_metadata_is_consistent(self) -> None:
        """The checked-in tree must be ready for its declared version tag."""
        result = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts/check-release-metadata.py"),
                "--tag",
                "v0.1.0-alpha.1",
            ],
            cwd=ROOT,
            check=True,
            capture_output=True,
            text=True,
        )
        self.assertIn("release metadata ok", result.stdout)

    def test_manifest_records_all_platform_archives(self) -> None:
        """Manifest generation verifies sidecars and records every supported OS."""
        with tempfile.TemporaryDirectory() as temporary_directory:
            dist = Path(temporary_directory)
            for platform, extension in (
                ("linux", "tar.gz"),
                ("macos", "tar.gz"),
                ("windows", "zip"),
            ):
                archive = (
                    dist
                    / f"ptymark-0.1.0-alpha.1-{platform}-x86_64.{extension}"
                )
                archive.write_bytes(platform.encode("utf-8"))
                digest = hashlib.sha256(archive.read_bytes()).hexdigest()
                archive.with_name(f"{archive.name}.sha256").write_text(
                    f"{digest}  {archive.name}\n", encoding="utf-8"
                )

            manifest_path = dist / "release-manifest.json"
            checksums_path = dist / "SHA256SUMS"
            notes_path = dist / "release-notes.md"
            subprocess.run(
                [
                    sys.executable,
                    str(ROOT / "scripts/build-release-manifest.py"),
                    "--root",
                    str(ROOT),
                    "--dist",
                    str(dist),
                    "--tag",
                    "v0.1.0-alpha.1",
                    "--commit",
                    "0" * 40,
                    "--output",
                    str(manifest_path),
                    "--checksums-output",
                    str(checksums_path),
                    "--notes-output",
                    str(notes_path),
                ],
                check=True,
                capture_output=True,
                text=True,
            )

            manifest = cast(
                dict[str, object],
                json.loads(manifest_path.read_text(encoding="utf-8")),
            )
            source = cast(dict[str, object], manifest["source"])
            assets = cast(list[dict[str, object]], manifest["assets"])
            self.assertEqual(manifest["version"], "0.1.0-alpha.1")
            self.assertEqual(source["commit"], "0" * 40)
            self.assertEqual(
                {asset["platform"] for asset in assets},
                {"linux", "macos", "windows"},
            )
            self.assertEqual(
                len(checksums_path.read_text(encoding="utf-8").splitlines()),
                4,
            )
            self.assertIn(
                "Native Unix PTY", notes_path.read_text(encoding="utf-8")
            )


if __name__ == "__main__":
    unittest.main()
