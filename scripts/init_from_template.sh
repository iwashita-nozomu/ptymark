#!/usr/bin/env bash
# @dependency-start
# contract tool
# responsibility Initializes a project repository from the template snapshot.
# upstream design README.md template overview
# downstream implementation start_repository.sh wraps template initialization
# @dependency-end
set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  bash scripts/init_from_template.sh --project-slug <slug> [options]

Options:
  --project-slug <slug>      Required. Kebab-case project slug.
  --display-name <name>      Optional. Human-facing display name.
  --python-package <name>    Optional. Defaults to slug with '-' replaced by '_'.
  --bare-repo <name>.git     Optional. Defaults to <slug>.git.
  --force                    Allow running with a dirty worktree.
  --dry-run                  Print the planned updates without writing files.
EOF
}

PROJECT_SLUG=""
DISPLAY_NAME=""
PYTHON_PACKAGE=""
BARE_REPO=""
BARE_GIT_ROOT="${TEMPLATE_BARE_GIT_ROOT:-/mnt/git}"
FORCE=0
DRY_RUN=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --project-slug)
      PROJECT_SLUG="${2:-}"
      shift 2
      ;;
    --display-name)
      DISPLAY_NAME="${2:-}"
      shift 2
      ;;
    --python-package)
      PYTHON_PACKAGE="${2:-}"
      shift 2
      ;;
    --bare-repo)
      BARE_REPO="${2:-}"
      shift 2
      ;;
    --force)
      FORCE=1
      shift
      ;;
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ -z "${PROJECT_SLUG}" ]]; then
  echo "--project-slug is required" >&2
  usage >&2
  exit 2
fi

if [[ ! "${PROJECT_SLUG}" =~ ^[a-z0-9][a-z0-9._-]*$ ]]; then
  echo "project slug must be lowercase and shell-safe: ${PROJECT_SLUG}" >&2
  exit 2
fi

if [[ -z "${DISPLAY_NAME}" ]]; then
  DISPLAY_NAME="${PROJECT_SLUG}"
fi

if [[ -z "${PYTHON_PACKAGE}" ]]; then
  PYTHON_PACKAGE="${PROJECT_SLUG//-/_}"
fi

if [[ -z "${BARE_REPO}" ]]; then
  BARE_REPO="${PROJECT_SLUG}.git"
fi

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

if [[ "${DRY_RUN}" != "1" ]] && [[ "${FORCE}" != "1" ]] && [[ -n "$(git status --short)" ]]; then
  echo "worktree is dirty; commit, stash, or rerun with --force" >&2
  exit 1
fi

export TEMPLATE_PROJECT_SLUG="${PROJECT_SLUG}"
export TEMPLATE_DISPLAY_NAME="${DISPLAY_NAME}"
export TEMPLATE_PYTHON_PACKAGE="${PYTHON_PACKAGE}"
export TEMPLATE_BARE_REPO="${BARE_REPO}"
export TEMPLATE_DRY_RUN="${DRY_RUN}"

python3 - <<'PY'
from __future__ import annotations

import os
from pathlib import Path

root = Path.cwd()
project_slug = os.environ["TEMPLATE_PROJECT_SLUG"]
display_name = os.environ["TEMPLATE_DISPLAY_NAME"]
python_package = os.environ["TEMPLATE_PYTHON_PACKAGE"]
bare_repo = os.environ["TEMPLATE_BARE_REPO"]
dry_run = os.environ["TEMPLATE_DRY_RUN"] == "1"

replacements: dict[str, list[tuple[str, str]]] = {
    "pyproject.toml": [
        ('name = "project-template"', f'name = "{project_slug}"'),
    ],
    "README.md": [
        ("# Project Template", f"# {display_name}"),
        ("docker build -t project-template -f docker/Dockerfile .", f"docker build -t {project_slug} -f docker/Dockerfile ."),
        ("  project-template bash", f"  {project_slug} bash"),
        ("/mnt/git/template.git", f"/mnt/git/{bare_repo}"),
    ],
    "QUICK_START.md": [
        ("docker build -t project-template -f docker/Dockerfile .", f"docker build -t {project_slug} -f docker/Dockerfile ."),
        ("  project-template bash", f"  {project_slug} bash"),
        ("/mnt/git/template.git", f"/mnt/git/{bare_repo}"),
    ],
    "docker/README.md": [
        ("python -m ipykernel install --user --name project-template", f"python -m ipykernel install --user --name {project_slug}"),
    ],
    ".devcontainer/devcontainer.json": [
        ('"name": "project-template"', f'"name": "{project_slug}"'),
    ],
    ".devcontainer/post-attach.sh": [
        ('echo "project-template devcontainer"', f'echo "{project_slug} devcontainer"'),
    ],
    "docker/Dockerfile": [
        ('"${TEMPLATE_BARE_GIT_ROOT}/template.git"', f'"${{TEMPLATE_BARE_GIT_ROOT}}/{bare_repo}"'),
    ],
    "docker/packs/default.toml": [
        ('image_tag = "project-template:default-runtime-pack"', f'image_tag = "{project_slug}:default-runtime-pack"'),
        ("/mnt/git/template.git", f"/mnt/git/{bare_repo}"),
    ],
    "docker/packs/default-host-docker.toml": [
        ('image_tag = "project-template:default-runtime-pack-host-docker"', f'image_tag = "{project_slug}:default-runtime-pack-host-docker"'),
    ],
    "documents/linux-wsl-host-requirements.md": [
        ("/mnt/git/template.git", f"/mnt/git/{bare_repo}"),
    ],
}

for relative_path, pairs in replacements.items():
    path = root / relative_path
    text = path.read_text(encoding="utf-8")
    original = text
    for before, after in pairs:
        text = text.replace(before, after)
    if text == original:
        continue
    if dry_run:
        print(f"would update {relative_path}")
    else:
        path.write_text(text, encoding="utf-8")
        print(f"updated {relative_path}")

print(f"project_slug={project_slug}")
print(f"display_name={display_name}")
print(f"python_package={python_package}")
print(f"bare_repo={bare_repo}")
print("agent_canon_source=github_submodule")
PY

report_agent_canon_source() {
  if [[ "${DRY_RUN}" == "1" ]]; then
    echo "would keep agent_canon_source=github_submodule"
    return
  fi

  echo "agent_canon_source=github_submodule"
  echo "agent_canon_next=commit_shared_canon_changes_on_vendor_branch_and_open_AgentCanon_PR"
}

report_agent_canon_source

if [[ "${DRY_RUN}" != "1" ]]; then
  echo "next:"
  echo "  1. Review git diff"
  echo "  2. Push the project branch to ${BARE_GIT_ROOT}/${BARE_REPO} or your chosen origin"
  echo "  3. Run make agent-canon-ensure-latest"
  echo "  4. Run make fresh-clone-check"
fi
