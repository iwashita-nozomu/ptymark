# @dependency-start
# contract test
# responsibility Verifies immutable Action refs, immutable PR-head CodeQL registration, stable PR gates, and prerelease-only publication.
# upstream implementation ../../scripts/check-workflow-policy.py workflow policy validator
# upstream environment ../../.github/workflows GitHub Actions definitions
# downstream environment ../../.github/workflows/ci.yml repository wiring gate
# @dependency-end

"""Workflow security policy regression tests."""

from __future__ import annotations

import importlib.util
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "check-workflow-policy.py"

_SPEC = importlib.util.spec_from_file_location("check_workflow_policy", SCRIPT)
if _SPEC is None or _SPEC.loader is None:  # pragma: no cover - import setup guard
    raise RuntimeError(f"cannot load workflow policy module from {SCRIPT}")
_POLICY = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(_POLICY)


class WorkflowPolicyTest(unittest.TestCase):
    """Verify workflow supply-chain, CodeQL identity, gate, and prerelease boundaries."""

    def test_repository_workflows_satisfy_policy(self) -> None:
        result = subprocess.run(
            [sys.executable, str(SCRIPT)],
            cwd=ROOT,
            check=True,
            capture_output=True,
            text=True,
        )
        self.assertIn("workflow security policy ok", result.stdout)

    def test_unpinned_remote_action_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            workflow_dir = root / ".github" / "workflows"
            workflow_dir.mkdir(parents=True)
            (workflow_dir / "example.yml").write_text(
                "name: example\njobs:\n  test:\n    steps:\n      - uses: actions/checkout@v7\n",
                encoding="utf-8",
            )
            failures = _POLICY.validate(root)
        self.assertTrue(
            any("full 40-character commit SHA" in failure for failure in failures),
            failures,
        )

    def test_codeql_branch_ref_registration_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            workflow_dir = root / ".github" / "workflows"
            workflow_dir.mkdir(parents=True)
            (workflow_dir / "codeql.yml").write_text(
                "pull_request:\n"
                "permissions:\n  security-events: write\n"
                "jobs:\n  analyze:\n    steps:\n"
                "      - name: Check out the immutable analyzed source without stored credentials\n"
                "        uses: actions/checkout@9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0\n"
                "        with:\n"
                "          repository: ${{ github.event_name == 'pull_request' && github.event.pull_request.head.repo.full_name || github.repository }}\n"
                "          ref: ${{ github.event_name == 'pull_request' && github.event.pull_request.head.sha || github.sha }}\n"
                "      - name: Verify checked-out source identity\n"
                "        env:\n"
                "          EXPECTED_SHA: ${{ github.event_name == 'pull_request' && github.event.pull_request.head.sha || github.sha }}\n"
                "      - name: Analyze and register the immutable pull-request head\n"
                "        uses: github/codeql-action/analyze@99df26d4f13ea111d4ec1a7dddef6063f76b97e9\n"
                "        with:\n"
                "          category: \"/language:${{ matrix.language }}\"\n"
                "          ref: ${{ github.event_name == 'pull_request' && format('refs/heads/{0}', github.event.pull_request.head.ref) || github.ref }}\n"
                "          sha: ${{ github.event_name == 'pull_request' && github.event.pull_request.head.sha || github.sha }}\n",
                encoding="utf-8",
            )
            failures = _POLICY.validate(root)
        self.assertTrue(
            any("branch or moving merge identity" in failure for failure in failures),
            failures,
        )

    def test_codeql_merge_ref_registration_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            workflow_dir = root / ".github" / "workflows"
            workflow_dir.mkdir(parents=True)
            (workflow_dir / "codeql.yml").write_text(
                "pull_request:\n"
                "permissions:\n  security-events: write\n"
                "jobs:\n  analyze:\n    steps:\n"
                "      - name: Check out the immutable analyzed source without stored credentials\n"
                "        uses: actions/checkout@9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0\n"
                "        with:\n"
                "          repository: ${{ github.event_name == 'pull_request' && github.event.pull_request.head.repo.full_name || github.repository }}\n"
                "          ref: ${{ github.event_name == 'pull_request' && github.event.pull_request.head.sha || github.sha }}\n"
                "      - name: Verify checked-out source identity\n"
                "        env:\n"
                "          EXPECTED_SHA: ${{ github.event_name == 'pull_request' && github.event.pull_request.head.sha || github.sha }}\n"
                "      - name: Analyze and register the immutable pull-request head\n"
                "        uses: github/codeql-action/analyze@99df26d4f13ea111d4ec1a7dddef6063f76b97e9\n"
                "        with:\n"
                "          category: \"/language:${{ matrix.language }}\"\n"
                "          ref: ${{ github.event_name == 'pull_request' && format('refs/pull/{0}/merge', github.event.pull_request.number) || github.ref }}\n"
                "          sha: ${{ github.event_name == 'pull_request' && github.event.pull_request.head.sha || github.sha }}\n",
                encoding="utf-8",
            )
            failures = _POLICY.validate(root)
        self.assertTrue(
            any("branch or moving merge identity" in failure for failure in failures),
            failures,
        )

    def test_product_gate_path_filter_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            workflow_dir = root / ".github" / "workflows"
            workflow_dir.mkdir(parents=True)
            (workflow_dir / "ptymark-ci.yml").write_text(
                "name: ptymark CI\n"
                "on:\n"
                "  pull_request:\n"
                "    paths:\n"
                "      - 'src/**'\n"
                "  push:\n"
                "    branches: [main]\n"
                "jobs:\n"
                "  gate:\n"
                "    name: Ptymark PR Gate\n"
                "    if: always()\n",
                encoding="utf-8",
            )
            failures = _POLICY.validate(root)
        self.assertTrue(
            any("stable PR gate must always be emitted" in failure for failure in failures),
            failures,
        )


if __name__ == "__main__":
    unittest.main()
