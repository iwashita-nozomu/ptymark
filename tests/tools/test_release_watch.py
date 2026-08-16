# @dependency-start
# contract test
# responsibility Verifies release-watch schema-v2 classification against false-positive and genuine future-v1 cases.
# upstream implementation ../../scripts/check-release-watch.py release-watch classifier
# upstream issue https://github.com/iwashita-nozomu/ptymark/issues/160 false-positive regression
# downstream environment ../../.github/workflows/ci.yml repository wiring gate
# @dependency-end

"""Release-watch classification regression tests."""

from __future__ import annotations

import importlib.util
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "check-release-watch.py"

_SPEC = importlib.util.spec_from_file_location("check_release_watch", SCRIPT)
if _SPEC is None or _SPEC.loader is None:  # pragma: no cover - import setup guard
    raise RuntimeError(f"cannot load release-watch classifier from {SCRIPT}")
_WATCH = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(_WATCH)

SCHEMA_V2_DESIGN = """\
# Alpha.4

## Schema v2

```toml
schema_version = 2
```
"""


class ReleaseWatchSchemaV2Test(unittest.TestCase):
    """Distinguish actionable roadmap work from historical or negative prose."""

    def test_current_schema_v2_baseline_is_aligned(self) -> None:
        issue = """\
## Scope and release baseline

This issue starts from the **shipped schema-v2 contract**. It does not propose
schema v1 as a future configuration format.

- [x] `schema_version = 2` is the canonical normalized user format.
- [x] Schema-v1 input is normalized deterministically into schema-v2.

The completed v1-to-v2 implementation is baseline evidence, not unfinished work.

## Acceptance criteria

- No roadmap or acceptance text in this issue presents `schema_version = 1` as future work.
"""
        self.assertFalse(_WATCH.schema_v2_roadmap_mismatch(issue, SCHEMA_V2_DESIGN))

    def test_unchecked_future_v1_task_is_mismatch(self) -> None:
        issue = "- [ ] Freeze the future public format as `schema_version = 1`.\n"
        self.assertTrue(_WATCH.schema_v2_roadmap_mismatch(issue, SCHEMA_V2_DESIGN))

    def test_multiline_future_v1_task_is_mismatch(self) -> None:
        issue = """\
- [ ] Define the future stable configuration:

  ```toml
  schema_version = 1
  ```

- [ ] Add editor completion.
"""
        self.assertTrue(_WATCH.schema_v2_roadmap_mismatch(issue, SCHEMA_V2_DESIGN))

    def test_completed_v1_history_is_not_mismatch(self) -> None:
        issue = "- [x] Alpha.1 shipped with `schema_version = 1`.\n"
        self.assertFalse(_WATCH.schema_v2_roadmap_mismatch(issue, SCHEMA_V2_DESIGN))

    def test_ordinary_negative_prose_is_not_mismatch(self) -> None:
        issue = "Do not restore `schema_version = 1` as future work.\n"
        self.assertFalse(_WATCH.schema_v2_roadmap_mismatch(issue, SCHEMA_V2_DESIGN))

    def test_without_schema_v2_design_there_is_no_cross_contract_mismatch(self) -> None:
        issue = "- [ ] Define `schema_version = 1`.\n"
        self.assertFalse(_WATCH.schema_v2_roadmap_mismatch(issue, "# No schema baseline\n"))

    def test_cli_reports_aligned_and_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            issue = root / "issue.md"
            design = root / "design.md"
            design.write_text(SCHEMA_V2_DESIGN, encoding="utf-8")

            issue.write_text(
                "No future `schema_version = 1` contract remains.\n", encoding="utf-8"
            )
            aligned = subprocess.run(
                [sys.executable, str(SCRIPT), "schema-v2-status", str(issue), str(design)],
                check=True,
                capture_output=True,
                text=True,
            )
            self.assertEqual(aligned.stdout.strip(), "aligned")

            issue.write_text(
                "- [ ] Keep `schema_version = 1` as the future format.\n",
                encoding="utf-8",
            )
            mismatch = subprocess.run(
                [sys.executable, str(SCRIPT), "schema-v2-status", str(issue), str(design)],
                check=True,
                capture_output=True,
                text=True,
            )
            self.assertEqual(mismatch.stdout.strip(), "mismatch")


if __name__ == "__main__":
    unittest.main()
