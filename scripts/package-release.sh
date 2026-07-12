#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
binary="${1:-$repo_root/target/release/ptymark}"
output_dir="${2:-$repo_root/dist}"

[[ -x "$binary" ]] || {
  printf 'release binary is not executable: %s\n' "$binary" >&2
  exit 1
}

version_output="$($binary --version)"
version="${version_output#ptymark }"
[[ -n "$version" && "$version" != "$version_output" ]] || {
  printf 'unexpected version output: %s\n' "$version_output" >&2
  exit 1
}

case "$(uname -s)" in
  Linux) platform=linux ;;
  Darwin) platform=macos ;;
  *) printf 'unsupported release packaging host: %s\n' "$(uname -s)" >&2; exit 1 ;;
esac

case "$(uname -m)" in
  x86_64|amd64) architecture=x86_64 ;;
  arm64|aarch64) architecture=aarch64 ;;
  *) printf 'unsupported release packaging architecture: %s\n' "$(uname -m)" >&2; exit 1 ;;
esac

hash_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

package_name="ptymark-${version}-${platform}-${architecture}"
package_root="$output_dir/$package_name"
archive="$output_dir/$package_name.tar.gz"
checksum="$archive.sha256"

rm -rf "$package_root"
mkdir -p \
  "$package_root/bin" \
  "$package_root/scripts" \
  "$package_root/renderers/managed" \
  "$package_root/plugin" \
  "$package_root/examples" \
  "$package_root/documents" \
  "$package_root/compat/shell-integrations"

install -m 755 "$binary" "$package_root/bin/ptymark"
install -m 755 "$repo_root/distribution/install.sh" "$package_root/install.sh"
install -m 755 "$repo_root/scripts/installer.sh" "$package_root/scripts/installer.sh"
install -m 755 "$repo_root/scripts/install-managed-bundle.sh" "$package_root/scripts/install-managed-bundle.sh"
install -m 644 "$repo_root/scripts/installer.ps1" "$package_root/scripts/installer.ps1"
install -m 644 "$repo_root/scripts/install-managed-bundle.ps1" "$package_root/scripts/install-managed-bundle.ps1"
install -m 644 "$repo_root/distribution/install.ps1" "$package_root/install.ps1"
install -m 644 "$repo_root/distribution/install.cmd" "$package_root/install.cmd"
install -m 644 "$repo_root/renderers/package.json" "$package_root/renderers/package.json"
install -m 644 "$repo_root/renderers/package-lock.json" "$package_root/renderers/package-lock.json"
install -m 644 "$repo_root/renderers/managed-bundle.env" "$package_root/renderers/managed-bundle.env"
install -m 644 "$repo_root/renderers/managed/mathjax-cli.mjs" "$package_root/renderers/managed/mathjax-cli.mjs"
install -m 644 "$repo_root/renderers/managed/ansi-presenter.mjs" "$package_root/renderers/managed/ansi-presenter.mjs"
install -m 644 "$repo_root/plugin/init.lua" "$package_root/plugin/init.lua"
install -m 644 "$repo_root/examples/ptymark.toml" "$package_root/examples/ptymark.toml"
install -m 644 "$repo_root/examples/wezterm.lua" "$package_root/examples/wezterm.lua"
install -m 644 "$repo_root/README.md" "$package_root/README.md"
install -m 644 "$repo_root/LICENSE" "$package_root/LICENSE"
install -m 644 "$repo_root/documents/ptymark-design.md" "$package_root/documents/ptymark-design.md"
install -m 644 "$repo_root/documents/ptymark-installer.md" "$package_root/documents/ptymark-installer.md"
install -m 644 "$repo_root/documents/shell-plugin-compatibility.md" "$package_root/documents/shell-plugin-compatibility.md"
for inventory in bash zsh fish powershell nushell; do
  install -m 644 \
    "$repo_root/compat/shell-integrations/$inventory.tsv" \
    "$package_root/compat/shell-integrations/$inventory.tsv"
done

cat >"$package_root/PACKAGE-MANIFEST.txt" <<EOF_MANIFEST
name=$package_name
version=$version
platform=$platform
architecture=$architecture
binary=bin/ptymark
installer=install.sh
EOF_MANIFEST

smoke_root="$(mktemp -d)"
trap 'rm -rf "$smoke_root"' EXIT
home="$smoke_root/home"
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

profile_files=(
  "$home/.bashrc"
  "$home/.zshrc"
  "$home/.config/fish/config.fish"
  "$home/.config/nushell/config.nu"
  "$home/.config/powershell/Microsoft.PowerShell_profile.ps1"
)
profile_snapshot() {
  local path
  for path in "${profile_files[@]}"; do
    printf '%s  %s\n' "$(hash_file "$path")" "$path"
  done
}
profile_snapshot >"$smoke_root/profiles.before"

HOME="$home" \
XDG_CONFIG_HOME="$smoke_root/xdg-config" \
XDG_STATE_HOME="$smoke_root/xdg-state" \
XDG_DATA_HOME="$smoke_root/xdg-data" \
bash "$package_root/install.sh" \
  --managed never \
  --mermaid preview \
  --math preview \
  --config "$smoke_root/config.toml" \
  --state "$smoke_root/state.toml"

profile_snapshot >"$smoke_root/profiles.after"
cmp "$smoke_root/profiles.before" "$smoke_root/profiles.after"
"$package_root/bin/ptymark" --version >/dev/null
"$package_root/bin/ptymark" --config "$smoke_root/config.toml" config check >/dev/null
printf '%s\n' '$$' 'E = mc^2' '$$' \
  | "$package_root/bin/ptymark" --config "$smoke_root/config.toml" preview - \
  >"$smoke_root/preview.out"
grep -F 'ptymark math' "$smoke_root/preview.out" >/dev/null

mkdir -p "$output_dir"
rm -f "$archive" "$checksum"
tar -czf "$archive" -C "$output_dir" "$package_name"
printf '%s  %s\n' "$(hash_file "$archive")" "$(basename "$archive")" >"$checksum"

printf 'package\t%s\n' "$package_root"
printf 'archive\t%s\n' "$archive"
printf 'checksum\t%s\n' "$checksum"
