#!/usr/bin/env bash
set -euo pipefail

package_root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
source_binary="$package_root/bin/ptymark"

[[ -x "$source_binary" ]] || {
  printf 'packaged ptymark binary is not executable: %s\n' "$source_binary" >&2
  exit 1
}

binary_destination="${PTYMARK_BINARY_DEST:-${CARGO_HOME:-${HOME:?HOME is required}/.cargo}/bin/ptymark}"
forward=()
need_value() {
  [[ -n "${2:-}" ]] || {
    printf 'missing value after %s\n' "$1" >&2
    exit 2
  }
}
while [[ $# -gt 0 ]]; do
  case "$1" in
    --binary-destination)
      need_value "$1" "${2:-}"
      binary_destination="$2"
      shift 2
      ;;
    --binary|--skip-core|--root)
      printf '%s is owned by the packaged installer; use --binary-destination instead\n' "$1" >&2
      exit 2
      ;;
    *)
      forward+=("$1")
      shift
      ;;
  esac
done

mkdir -p "$(dirname "$binary_destination")"
temporary_binary="${binary_destination}.tmp.$$"
trap 'rm -f "${temporary_binary:-}"' EXIT
install -m 755 "$source_binary" "$temporary_binary"
mv -f "$temporary_binary" "$binary_destination"
trap - EXIT
binary_destination="$(cd "$(dirname "$binary_destination")" && pwd -P)/$(basename "$binary_destination")"

exec bash "$package_root/scripts/installer.sh" \
  --skip-core \
  --binary "$binary_destination" \
  "${forward[@]}"
