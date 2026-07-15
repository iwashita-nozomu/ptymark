# @dependency-start
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
