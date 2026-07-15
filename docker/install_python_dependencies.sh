#!/usr/bin/env bash
# @dependency-start
# contract environment
# responsibility Installs the selected repo Python dependency layer after the workspace is mounted.
# upstream environment requirements-runtime.txt canonical workload runtime dependencies
# upstream environment requirements-dev.txt canonical verification-only dependencies
# upstream environment requirements.txt generated full development aggregate
# upstream environment ../.devcontainer/post-create.sh devcontainer post-create entrypoint
# downstream environment ../.github/workflows/ci.yml selects the verification profile for CI
# downstream environment packs/default.toml smoke-runs the default verification profile
# @dependency-end

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: docker/install_python_dependencies.sh [WORKSPACE] [--profile runtime|verification]

Profiles:
  runtime       Install only packages required by the canonical workload runtime.
  verification  Install runtime plus test, lint, documentation, and audit packages.

The default profile is verification for backward-compatible devcontainer behavior.
EOF
}

workspace="/workspace"
profile="verification"

if [ "$#" -gt 0 ] && [[ "$1" != --* ]]; then
  workspace="$1"
  shift
fi

while [ "$#" -gt 0 ]; do
  case "$1" in
    --profile)
      [ "$#" -ge 2 ] || {
        printf 'missing value for --profile\n' >&2
        exit 2
      }
      profile="$2"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      printf 'unknown argument: %s\n' "$1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

requirements_runtime="${workspace%/}/docker/requirements-runtime.txt"
requirements_dev="${workspace%/}/docker/requirements-dev.txt"
requirements_all="${workspace%/}/docker/requirements.txt"

case "$profile" in
  runtime)
    requirements="$requirements_runtime"
    hash_inputs=("$requirements_runtime")
    ;;
  verification)
    requirements="$requirements_all"
    hash_inputs=("$requirements_runtime" "$requirements_dev" "$requirements_all")
    ;;
  *)
    printf 'unsupported Python dependency profile: %s\n' "$profile" >&2
    usage >&2
    exit 2
    ;;
esac

for dependency_file in "${hash_inputs[@]}"; do
  if [ ! -f "$dependency_file" ]; then
    printf 'missing requirements file: %s\n' "$dependency_file" >&2
    exit 2
  fi
done

marker_dir="${PYTHON_DEPENDENCY_MARKER_DIR:-/usr/local/share/project-template}"
marker="${marker_dir%/}/python-requirements-${profile}.sha256"
requirements_hash="$({
  for dependency_file in "${hash_inputs[@]}"; do
    printf '%s\n' "${dependency_file#${workspace%/}/}"
    sha256sum "$dependency_file"
  done
} | sha256sum | awk '{print $1}')"

if [ -f "$marker" ] && [ "$(cat "$marker")" = "$requirements_hash" ]; then
  if python3 -m pip check >/dev/null; then
    printf 'python_dependencies=up-to-date profile=%s hash=%s\n' "$profile" "$requirements_hash"
    exit 0
  fi
fi

python3 -m pip install --upgrade pip
python3 -m pip install --no-cache-dir -r "$requirements"
python3 -m pip check

mkdir -p "$marker_dir"
printf '%s' "$requirements_hash" > "$marker"
printf 'python_dependencies=installed profile=%s requirements=%s hash=%s\n' \
  "$profile" "${requirements#${workspace%/}/}" "$requirements_hash"
