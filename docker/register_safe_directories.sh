#!/usr/bin/env bash
# @dependency-start
# contract environment
# responsibility Registers the workspace and direct vendor children as Git safe directories.
# upstream environment Dockerfile installs this helper into the canonical image.
# downstream environment ../.devcontainer/devcontainer.json runs this helper after workspace mount.
# downstream environment packs/default.toml smoke-checks the registered safe directories.
# @dependency-end

set -euo pipefail

workspace="${1:-/workspace}"
scope="${GIT_SAFE_DIRECTORY_SCOPE:-global}"

case "$scope" in
  global|system)
    ;;
  *)
    printf 'unsupported GIT_SAFE_DIRECTORY_SCOPE=%s\n' "$scope" >&2
    exit 2
    ;;
esac

list_registered_safe_dirs() {
  git config "--${scope}" --get-all safe.directory 2>/dev/null || true
}

safe_dir_is_registered() {
  local target="$1"
  list_registered_safe_dirs | grep -Fx -- "$target" >/dev/null
}

register_safe_dir() {
  local target="$1"

  [ -n "$target" ] || return 0
  if ! safe_dir_is_registered "$target"; then
    git config "--${scope}" --add safe.directory "$target"
  fi
  printf 'safe_directory=%s\n' "$target"
}

register_safe_dir "$workspace"

vendor_root="${workspace%/}/vendor"
if [ -d "$vendor_root" ]; then
  while IFS= read -r -d '' vendor_dir; do
    register_safe_dir "$vendor_dir"
  done < <(find "$vendor_root" -mindepth 1 -maxdepth 1 -type d -print0 | sort -z)
fi
