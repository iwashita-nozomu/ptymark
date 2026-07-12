#!/usr/bin/env bash
# @dependency-start
# contract installer
# responsibility Installs the ptymark binary, resolves optional renderer executables once, and writes the user-owned configuration snapshot.
# upstream implementation ../src/install.rs owns engine-resolution and replacement policy.
# downstream test ../tests/install_smoke.sh validates idempotent installation and engine replacement.
# @dependency-end

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
install_root=""
binary=""
skip_core=0
dry_run=0
state_path=""
resolve_args=()

usage() {
  cat <<'EOF'
Usage: bash scripts/install.sh [OPTIONS]

Install the ptymark binary, then resolve already-installed rendering engines
into an absolute-path user configuration.

Options:
  --root DIR          pass DIR to `cargo install --root`
  --binary PATH       use PATH as the installed ptymark binary
  --skip-core         do not run `cargo install` (requires --binary)
  --config PATH       write/update this ptymark configuration
  --state PATH        write the installation snapshot here
  --mermaid VALUE     keep | auto | preview | source | EXECUTABLE
  --math VALUE        keep | auto | preview | source | EXECUTABLE
  --presenter VALUE   keep | auto | EXECUTABLE
  --reprobe           search standard engine names again in the current PATH
  --reset             discard existing configuration before resolving
  --dry-run           install the core, but only print the engine-resolution plan
  -h, --help          show this help

The installer never invokes npm, Homebrew, apt, or another package manager.
Install optional tools first, then rerun with --reprobe or an explicit path.
EOF
}

need_value() {
  local option="$1"
  local value="${2:-}"
  if [[ -z "$value" ]]; then
    printf 'missing value after %s\n' "$option" >&2
    exit 2
  fi
}

while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --root)
      need_value "$1" "${2:-}"
      install_root="$2"
      shift 2
      ;;
    --binary)
      need_value "$1" "${2:-}"
      binary="$2"
      shift 2
      ;;
    --skip-core)
      skip_core=1
      shift
      ;;
    --config)
      need_value "$1" "${2:-}"
      resolve_args+=(--config "$2")
      shift 2
      ;;
    --state)
      need_value "$1" "${2:-}"
      state_path="$2"
      resolve_args+=(--state "$2")
      shift 2
      ;;
    --mermaid|--math|--presenter)
      need_value "$1" "${2:-}"
      resolve_args+=("$1" "$2")
      shift 2
      ;;
    --reprobe)
      resolve_args+=(--mermaid auto --math auto --presenter auto)
      shift
      ;;
    --reset)
      resolve_args+=(--reset)
      shift
      ;;
    --dry-run)
      dry_run=1
      resolve_args+=(--dry-run)
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      printf 'unknown installer option: %s\n' "$1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ "$skip_core" -eq 0 ]]; then
  command -v cargo >/dev/null 2>&1 || {
    echo 'cargo is required to install the ptymark core binary' >&2
    exit 1
  }
  cargo_args=(install --locked --force --path "$repo_root")
  if [[ -n "$install_root" ]]; then
    cargo_args+=(--root "$install_root")
  fi
  cargo "${cargo_args[@]}"
fi

if [[ -z "$binary" ]]; then
  if [[ -n "$install_root" ]]; then
    binary="$install_root/bin/ptymark"
  elif [[ -n "${CARGO_INSTALL_ROOT:-}" ]]; then
    binary="$CARGO_INSTALL_ROOT/bin/ptymark"
  elif [[ -n "${CARGO_HOME:-}" ]]; then
    binary="$CARGO_HOME/bin/ptymark"
  else
    binary="${HOME:?HOME is required when --binary and --root are omitted}/.cargo/bin/ptymark"
  fi
fi

if [[ "$skip_core" -eq 1 && ! -x "$binary" ]]; then
  printf 'the --skip-core binary is not executable: %s\n' "$binary" >&2
  exit 1
fi
if [[ ! -x "$binary" ]]; then
  printf 'installed ptymark binary was not found or is not executable: %s\n' "$binary" >&2
  exit 1
fi

"$binary" install resolve "${resolve_args[@]}"

if [[ "$dry_run" -eq 0 ]]; then
  status_args=()
  if [[ -n "$state_path" ]]; then
    status_args+=(--state "$state_path")
  fi
  "$binary" install status "${status_args[@]}"
fi
