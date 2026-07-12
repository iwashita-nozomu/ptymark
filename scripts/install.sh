#!/usr/bin/env bash
# Compatibility entrypoint. New documentation uses scripts/installer.sh.
set -euo pipefail
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
exec bash "$script_dir/installer.sh" "$@"
