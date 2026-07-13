#!/usr/bin/env bash
set -euo pipefail

binary="${1:-${CARGO_TARGET_DIR:-target}/debug/ptymark}"
[[ -x "$binary" ]] || {
  printf 'ptymark binary is not executable: %s\n' "$binary" >&2
  exit 1
}
binary="$(cd "$(dirname "$binary")" && pwd -P)/$(basename "$binary")"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
root="$(mktemp -d)"
trap 'rm -rf "$root"' EXIT
home="$root/home"
config="$root/config.toml"
state="$root/state.toml"
mkdir -p \
  "$home/.config/fish" \
  "$home/.config/nushell" \
  "$home/.config/powershell"

cat >"$home/.bashrc" <<'EOF_BASH_PROFILE'
source "$HOME/.bash_plugins"
EOF_BASH_PROFILE
cat >"$home/.zshrc" <<'EOF_ZSH_PROFILE'
source "$ZDOTDIR/plugins.zsh"
EOF_ZSH_PROFILE
cat >"$home/.config/fish/config.fish" <<'EOF_FISH_PROFILE'
source ~/.config/fish/plugins.fish
EOF_FISH_PROFILE
cat >"$home/.config/nushell/config.nu" <<'EOF_NUSHELL_PROFILE'
source ~/.config/nushell/plugins.nu
EOF_NUSHELL_PROFILE
cat >"$home/.config/powershell/Microsoft.PowerShell_profile.ps1" <<'EOF_POWERSHELL_PROFILE'
Import-Module PSReadLine
EOF_POWERSHELL_PROFILE

snapshot() {
  find "$home" -type f -print0 \
    | sort -z \
    | xargs -0 sha256sum
}

snapshot >"$root/before.sha256"
HOME="$home" \
XDG_CONFIG_HOME="$root/xdg-config" \
XDG_STATE_HOME="$root/xdg-state" \
XDG_DATA_HOME="$root/xdg-data" \
bash "$repo_root/scripts/installer.sh" \
  --skip-core \
  --binary "$binary" \
  --managed never \
  --mermaid preview \
  --math preview \
  --config "$config" \
  --state "$state"
snapshot >"$root/after.sha256"
cmp "$root/before.sha256" "$root/after.sha256"

child_output="$root/child.out"
normalized_output="$root/child.normalized.out"
# The command string is intentionally expanded by the child shell, not here.
# shellcheck disable=SC2016
child_command='printf "%s|%s|%s|%s|%s\n" "$STARSHIP_SHELL" "$ATUIN_SESSION" "$ZDOTDIR" "$FISH_CONFIG_DIR" "$NU_LIB_DIRS"'
STARSHIP_SHELL=bash \
ATUIN_SESSION=ptymark-compat \
ZDOTDIR="$home" \
FISH_CONFIG_DIR="$home/.config/fish" \
NU_LIB_DIRS="$home/.config/nushell" \
"$binary" --config "$config" -- sh -c "$child_command" \
  >"$child_output"
tr -d '\r' <"$child_output" >"$normalized_output"
expected="bash|ptymark-compat|$home|$home/.config/fish|$home/.config/nushell"
grep -Fx "$expected" "$normalized_output" >/dev/null

printf 'ptymark real-PTY shell profile coexistence: ok\n'
