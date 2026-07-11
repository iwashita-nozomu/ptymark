#!/usr/bin/env bash
# @dependency-start
# contract environment
# responsibility Installs repo Python dependencies after the workspace is mounted.
# upstream environment requirements.txt canonical Python dependency list
# upstream environment ../.devcontainer/post-create.sh devcontainer post-create entrypoint
# downstream environment ../.github/workflows/ci.yml installs CI Python dependencies through this script
# downstream environment packs/default.toml smoke-runs this installer before Python-dependent checks
# @dependency-end

set -euo pipefail

workspace="${1:-/workspace}"
requirements="${workspace%/}/docker/requirements.txt"
marker_dir="${PYTHON_DEPENDENCY_MARKER_DIR:-/usr/local/share/project-template}"
marker="${marker_dir%/}/python-requirements.sha256"

if [ ! -f "$requirements" ]; then
  printf 'missing requirements file: %s\n' "$requirements" >&2
  exit 2
fi

requirements_hash="$(sha256sum "$requirements" | awk '{print $1}')"

if [ -f "$marker" ] && [ "$(cat "$marker")" = "$requirements_hash" ]; then
  if python3 -m pip check >/dev/null; then
    printf 'python_dependencies=up-to-date hash=%s\n' "$requirements_hash"
    exit 0
  fi
fi

python3 -m pip install --upgrade pip
python3 -m pip install --no-cache-dir -r "$requirements"
python3 -m pip check

mkdir -p "$marker_dir"
printf '%s' "$requirements_hash" > "$marker"
printf 'python_dependencies=installed hash=%s\n' "$requirements_hash"
