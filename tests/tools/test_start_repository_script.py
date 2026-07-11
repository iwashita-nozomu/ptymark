# @dependency-start
# contract test
# responsibility Tests test start repository script behavior.
# upstream implementation ../../scripts/start_repository.sh repository start wrapper
# upstream design ../../documents/template-bootstrap.md bootstrap contract
# @dependency-end
"""Tests for the start repository wrapper script."""

from __future__ import annotations

import os
import subprocess
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]


def run(
    args: list[str],
    cwd: Path,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    """Run a command and capture text output."""
    return subprocess.run(
        args,
        cwd=cwd,
        env=env,
        check=True,
        capture_output=True,
        text=True,
    )


def test_start_repository_wrapper_keeps_agent_canon_github_submodule(tmp_path: Path) -> None:
    """The wrapper preserves dry-run safety and does not seed local AgentCanon remotes."""
    clone_dir = tmp_path / "clone"
    git_root = tmp_path / "git"
    missing_git_exec = tmp_path / "missing-git-exec"
    git_root.mkdir()
    missing_git_exec.mkdir()

    run(["git", "clone", "--no-local", str(REPO_ROOT), str(clone_dir)], cwd=tmp_path)
    run(
        ["rsync", "-a", "--delete", "--exclude", ".git", f"{REPO_ROOT}/", str(clone_dir)],
        cwd=tmp_path,
    )

    env = os.environ.copy()
    env["TEMPLATE_BARE_GIT_ROOT"] = str(git_root)
    env["GIT_EXEC_PATH"] = str(missing_git_exec)

    dry_run = run(
        [
            "bash",
            "scripts/start_repository.sh",
            "--project-slug",
            "seeded-project",
            "--display-name",
            "Seeded Project",
            "--dry-run",
        ],
        cwd=clone_dir,
        env=env,
    )

    assert "would keep agent_canon_source=github_submodule" in dry_run.stdout
    assert "start_repository_mode=dry_run_only" in dry_run.stdout

    result = run(
        [
            "bash",
            "scripts/start_repository.sh",
            "--project-slug",
            "seeded-project",
            "--display-name",
            "Seeded Project",
            "--skip-preflight-dry-run",
            "--force",
        ],
        cwd=clone_dir,
        env=env,
    )

    assert "agent_canon_source=github_submodule" in result.stdout
    assert "agent_canon_preflight=blocked_init_force" in result.stdout
    assert (
        "agent_canon_preflight_reason="
        "wrapper_skips_make_agent-canon-ensure-latest_when_init_force_is_requested" in result.stdout
    )
    assert "start_repository_init=pass" in result.stdout
    assert not (git_root / "seeded-project-agent-canon.git").exists()
