#!/usr/bin/env bash
# @dependency-start
# contract distribution
# responsibility Builds a native ptymark archive with the binary, launcher, validated configuration examples, user/design documents, and checksum.
# upstream implementation ../Cargo.toml supplies the package version.
# upstream implementation ../plugin/init.lua supplies the WezTerm launcher.
# upstream configuration ../examples/ptymark.example.toml supplies the canonical complete configuration.
# upstream configuration ../examples/config/*.toml supplies copy-paste session profiles.
# downstream workflow ../.github/workflows/ptymark-release.yml publishes archives created by this script.
# @dependency-end
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

binary_path="${1:-target/release/ptymark}"
out_dir="${2:-dist}"

[[ -x "$binary_path" ]] || {
  printf 'release binary is missing or not executable: %s\n' "$binary_path" >&2
  exit 1
}

version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)"
host="$(rustc -vV | sed -n 's/^host: //p')"
[[ -n "$version" && -n "$host" ]]

archive_root="ptymark-v${version}-${host}"
work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT
mkdir -p \
  "$work_dir/$archive_root/plugin" \
  "$work_dir/$archive_root/examples/config" \
  "$work_dir/$archive_root/documents" \
  "$out_dir"

cp "$binary_path" "$work_dir/$archive_root/ptymark"
cp LICENSE README.md "$work_dir/$archive_root/"
cp plugin/init.lua "$work_dir/$archive_root/plugin/init.lua"
cp examples/ptymark.example.toml "$work_dir/$archive_root/examples/ptymark.example.toml"
cp examples/config/*.toml "$work_dir/$archive_root/examples/config/"

for document in \
  README.md \
  system-design.md \
  design-review.md \
  architecture.md \
  renderer-architecture.md \
  configuration.md \
  ui-design.md \
  extension-guide.md \
  usage.md \
  dependencies.md \
  development-environment.md \
  distribution.md \
  licensing-policy.md; do
  cp "documents/$document" "$work_dir/$archive_root/documents/$document"
done

tar -C "$work_dir" -czf "$out_dir/$archive_root.tar.gz" "$archive_root"

if command -v sha256sum >/dev/null 2>&1; then
  (cd "$out_dir" && sha256sum "$archive_root.tar.gz" >"$archive_root.tar.gz.sha256")
else
  (cd "$out_dir" && shasum -a 256 "$archive_root.tar.gz" >"$archive_root.tar.gz.sha256")
fi

printf '%s\n' "$out_dir/$archive_root.tar.gz"
