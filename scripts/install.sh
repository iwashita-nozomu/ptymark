#!/usr/bin/env bash

# @dependency-start
# contract implementation
# responsibility Preserves the former installer command as a compatibility wrapper.
# upstream design ../README.md documented compatibility command
# upstream implementation ./installer.sh canonical installer
# downstream implementation ../tests/install_smoke.sh wrapper validation
# @dependency-end

# Compatibility entrypoint. New documentation uses scripts/installer.sh.
set -euo pipefail
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
exec bash "$script_dir/installer.sh" "$@"
