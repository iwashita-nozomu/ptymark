#!/usr/bin/env bash
set -euo pipefail

package_root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
binary="$package_root/bin/ptymark"

[[ -x "$binary" ]] || {
  printf 'packaged ptymark binary is not executable: %s\n' "$binary" >&2
  exit 1
}

exec bash "$package_root/scripts/installer.sh" \
  --skip-core \
  --binary "$binary" \
  "$@"
