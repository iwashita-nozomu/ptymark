# @dependency-start
# contract test
# responsibility Tests worktree note bootstrap and current-checkout work-log behavior.
# upstream implementation ../../tools/agent_tools/bootstrap_worktree_notes.py bootstrap helper
# upstream implementation ../../tools/agent_tools/work_log.py run-local work-log helper
# upstream design ../../vendor/agent-canon/documents/worktree-lifecycle.md worktree lifecycle contract
# @dependency-end
"""Tests for worktree note bootstrap and append helpers."""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[2]
BOOTSTRAP_SCRIPT = PROJECT_ROOT / "tools" / "agent_tools" / "bootstrap_worktree_notes.py"
WORK_LOG_SCRIPT = PROJECT_ROOT / "tools" / "agent_tools" / "work_log.py"


def test_bootstrap_worktree_notes_and_append_run_log(tmp_path: Path) -> None:
    """The helpers fill legacy note paths and append current run-bundle entries."""
    repo_root = tmp_path / "repo"
    workspace_root = repo_root / ".worktrees" / "work-demo"
    notes_worktrees = repo_root / "notes" / "worktrees"
    notes_branches = repo_root / "notes" / "branches"
    run_dir = repo_root / "reports" / "agents" / "run-demo"

    workspace_root.mkdir(parents=True)
    notes_worktrees.mkdir(parents=True)
    notes_branches.mkdir(parents=True)
    run_dir.mkdir(parents=True)

    (workspace_root / "WORKTREE_SCOPE.md").write_text(
        "\n".join(
            [
                "# WORKTREE_SCOPE Template",
                "",
                "## Worktree Summary",
                "- Branch:",
                "- Worktree path:",
                "- Purpose:",
                "- Owner or agent:",
                "",
                "## Kickoff Status",
                "- Scope refreshed at:",
                "- Action log path:",
                "- Branch summary path:",
                "- User request contract path:",
                "- Kickoff checks completed:",
                "- Next step after kickoff:",
                "",
                "## Editable Directories",
                "- `python`",
                "",
                "## Runtime Output Directories",
                "- `reports/`",
                "",
                "## Read-Only Or Avoid Directories",
                "- `vendor/`",
                "",
                "## Required References Before Editing",
                "- [documents/worktree-lifecycle.md](documents/worktree-lifecycle.md)",
                "- [documents/BRANCH_SCOPE.md](documents/BRANCH_SCOPE.md)",
                "- [notes/worktrees/README.md](notes/worktrees/README.md)",
                "",
                "## Main Carry-Over Targets",
                "- `notes/worktrees/worktree_<topic>_YYYY-MM-DD.md`",
                "- `notes/branches/<branch_topic>.md`",
                "",
                "## Working Notes During Execution",
                "- Action log path: `notes/worktrees/worktree_<topic>_YYYY-MM-DD.md`",
                "- Branch summary path: `notes/branches/<branch_topic>.md`",
                "- User request contract path: `reports/agents/<run-id>/user_request_contract.md`",
                "",
                "## Required Checks Before Commit",
                "- `make ci-quick`",
                "",
            ]
        )
        + "\n",
        encoding="utf-8",
    )

    bootstrap = subprocess.run(
        [
            sys.executable,
            str(BOOTSTRAP_SCRIPT),
            "--repo-root",
            str(repo_root),
            "--workspace-root",
            str(workspace_root),
            "--branch",
            "work/demo-20260408",
        ],
        cwd=PROJECT_ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    assert bootstrap.returncode == 0, bootstrap.stderr
    match = re.search(
        r"ACTION_LOG=(notes/worktrees/worktree_work-demo_\d{4}-\d{2}-\d{2}\.md)", bootstrap.stdout
    )
    assert match is not None, bootstrap.stdout

    action_log_rel = match.group(1)
    action_log = repo_root / action_log_rel
    branch_summary = repo_root / "notes" / "branches" / "work-demo.md"
    scope_text = (workspace_root / "WORKTREE_SCOPE.md").read_text(encoding="utf-8")

    assert action_log.is_file()
    assert branch_summary.is_file()
    assert "`work/demo-20260408`" in scope_text
    assert action_log_rel in scope_text
    assert "notes/branches/work-demo.md" in scope_text

    work_log = subprocess.run(
        [
            sys.executable,
            str(WORK_LOG_SCRIPT),
            "--workspace-root",
            str(workspace_root),
            "--report-dir",
            str(run_dir),
            "--kind",
            "test",
            "--message",
            "ran smoke checks",
            "--request-clause-id",
            "R1",
            "--next",
            "prepare closeout",
            "--ref",
            "reports/demo",
        ],
        cwd=PROJECT_ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    assert work_log.returncode == 0, work_log.stderr

    run_log_text = (run_dir / "work_log.md").read_text(encoding="utf-8")
    assert "ran smoke checks" in run_log_text
    assert "request_clause_ids: R1" in run_log_text
    assert "refs: reports/demo" in run_log_text
    assert "next: prepare closeout" in run_log_text
