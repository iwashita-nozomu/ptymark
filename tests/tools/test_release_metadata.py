# @dependency-start
# contract test
# responsibility Verifies prerelease version consistency and the source-only publication policy.
# upstream implementation ../../scripts/check-release-metadata.py source-only validator
# upstream environment ../../Cargo.toml current package version
# upstream design ../../documents/release.md source-only prerelease contract
# downstream environment ../../.github/workflows/ptymark-release.yml notes-only publication
# @dependency-end

"""Source-only prerelease metadata contract tests."""

from __future__ import annotations

import re
import subprocess
import sys
import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PRERELEASE_VERSION_PATTERN = re.compile(
    r"^[0-9]+\.[0-9]+\.[0-9]+-(?:alpha|beta|rc)\.[0-9]+$"
)


class ReleaseMetadataTest(unittest.TestCase):
    """Verify source-only prerelease metadata and publication constraints."""

    def test_release_tree_metadata_is_consistent(self) -> None:
        """Require the current prerelease tree to satisfy the metadata validator."""
        cargo = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
        version = cargo["package"]["version"]
        self.assertRegex(version, PRERELEASE_VERSION_PATTERN)
        tag = f"v{version}"
        result = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts/check-release-metadata.py"),
                "--tag",
                tag,
            ],
            cwd=ROOT,
            check=True,
            capture_output=True,
            text=True,
        )
        self.assertIn("source-only release metadata ok", result.stdout)
        self.assertIn("prerelease", result.stdout)

    def test_release_workflow_publishes_prerelease_notes_without_project_assets(self) -> None:
        """Require release automation to publish prerelease notes without binary assets."""
        workflow = (ROOT / ".github/workflows/ptymark-release.yml").read_text()
        self.assertIn("gh release create", workflow)
        self.assertIn("--notes-file", workflow)
        self.assertIn("--prerelease", workflow)
        self.assertIn("isPrerelease", workflow)
        self.assertIn(
            "source prerelease workflow accepts only alpha, beta, or rc versions",
            workflow,
        )
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
            "--latest",
            "make_latest=true",
        ):
            self.assertNotIn(forbidden, workflow)

    def test_product_ci_keeps_package_smoke_ephemeral(self) -> None:
        """Require product package smoke outputs to remain ephemeral."""
        workflow = (ROOT / ".github/workflows/ptymark-ci.yml").read_text()
        self.assertIn("Cross-platform local package smoke", workflow)
        self.assertIn("Discard local package output", workflow)
        self.assertNotIn("Upload executable package", workflow)
        self.assertNotIn("dist/*.tar.gz", workflow)
        self.assertNotIn("dist/*.zip", workflow)

    def test_local_packagers_are_not_a_distribution_channel(self) -> None:
        """Require local packaging scripts to disclaim distribution status."""
        for relative in ("scripts/package-release.sh", "scripts/package-release.ps1"):
            content = (ROOT / relative).read_text(encoding="utf-8").lower()
            self.assertIn("developer/ci verification only", content)


if __name__ == "__main__":
    unittest.main()
