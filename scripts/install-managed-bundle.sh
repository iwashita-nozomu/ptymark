#!/usr/bin/env bash

# @dependency-start
# contract implementation
# responsibility Installs the pinned private renderer bundle on Unix.
# upstream design ../documents/ptymark-installer.md bundle isolation
# upstream environment ../renderers/managed-bundle.env runtime pins
# upstream environment ../renderers/package-lock.json package lock
# downstream implementation ./installer.sh role selection
# downstream implementation ../tests/managed_renderer_smoke.sh bundle validation
# @dependency-end

# Install the pinned Node renderer bundle under a versioned user-data directory.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
# shellcheck disable=SC1091
source "$repo_root/renderers/managed-bundle.env"

managed_root=""
launcher=""
browser_path=""
skip_browser_download=0
offline=0
force=0

usage() {
  cat <<'EOF'
Usage: bash scripts/install-managed-bundle.sh [OPTIONS]

Install the pinned Mermaid/MathJax/ANSI renderer bundle without modifying the
user's PATH or global npm installation. The renderer commands are native copies
of the ptymark binary, so user content is never forwarded through a shell.

Options:
  --root DIR                install into DIR
  --launcher PATH           native ptymark binary used for mmdc/tex2svg/chafa aliases
  --browser PATH            use an existing Chromium-compatible browser
  --skip-browser-download   require an existing browser; do not download one
  --offline                 use an existing complete bundle; perform no downloads or npm install
  --force                   reinstall the app bundle and native aliases
  -h, --help                show this help
EOF
}

need_value() {
  [[ -n "${2:-}" ]] || { printf 'missing value after %s\n' "$1" >&2; exit 2; }
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --root) need_value "$1" "${2:-}"; managed_root="$2"; shift 2 ;;
    --launcher) need_value "$1" "${2:-}"; launcher="$2"; shift 2 ;;
    --browser) need_value "$1" "${2:-}"; browser_path="$2"; shift 2 ;;
    --skip-browser-download) skip_browser_download=1; shift ;;
    --offline) offline=1; shift ;;
    --force) force=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) printf 'unknown managed-bundle option: %s\n' "$1" >&2; usage >&2; exit 2 ;;
  esac
done

[[ -n "$launcher" ]] || { echo '--launcher is required' >&2; exit 2; }
[[ -x "$launcher" ]] || { printf 'launcher is not executable: %s\n' "$launcher" >&2; exit 1; }
launcher="$(cd "$(dirname "$launcher")" && pwd -P)/$(basename "$launcher")"

host_os="$(uname -s)"
case "$host_os" in
  Darwin)
    platform=darwin
    default_data_root="${HOME:?HOME is required}/Library/Application Support/ptymark"
    ;;
  Linux)
    platform=linux
    default_data_root="${XDG_DATA_HOME:-${HOME:?HOME is required}/.local/share}/ptymark"
    ;;
  *)
    printf 'managed renderer installation is unsupported on %s; use scripts/installer.ps1 on Windows\n' "$host_os" >&2
    exit 1
    ;;
esac

case "$(uname -m)" in
  x86_64|amd64) architecture=x64 ;;
  arm64|aarch64) architecture=arm64 ;;
  *) printf 'unsupported managed renderer architecture: %s\n' "$(uname -m)" >&2; exit 1 ;;
esac

bundle_id="v${PTYMARK_MANAGED_BUNDLE_VERSION}-node${PTYMARK_MANAGED_NODE_VERSION}-mermaid${PTYMARK_MANAGED_MERMAID_VERSION}-mathjax${PTYMARK_MANAGED_MATHJAX_VERSION}"
managed_root="${managed_root:-$default_data_root/renderer-bundles/$bundle_id}"
managed_root="$(mkdir -p "$managed_root" && cd "$managed_root" && pwd -P)"
runtime_root="$managed_root/runtime/node-v${PTYMARK_MANAGED_NODE_VERSION}-${platform}-${architecture}"
app_root="$managed_root/app"
bin_root="$managed_root/bin"
cache_root="$managed_root/cache/puppeteer"
stamp_path="$managed_root/bundle.stamp"
manifest_path="$managed_root/bundle.toml"
puppeteer_config_path="$managed_root/puppeteer-config.json"
mkdir -p "$bin_root" "$cache_root"

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

download() {
  local url="$1"
  local destination="$2"
  if command -v curl >/dev/null 2>&1; then
    curl --proto '=https' --tlsv1.2 --fail --location --silent --show-error "$url" --output "$destination"
  elif command -v wget >/dev/null 2>&1; then
    wget --https-only --quiet "$url" --output-document "$destination"
  else
    echo 'curl or wget is required to install the private Node runtime' >&2
    return 1
  fi
}

node_cmd=""
npm_cmd=""
if command -v node >/dev/null 2>&1 && command -v npm >/dev/null 2>&1 \
  && [[ "$(node -p 'process.versions.node')" == "$PTYMARK_MANAGED_NODE_VERSION" ]]; then
  node_cmd="$(command -v node)"
  npm_cmd="$(command -v npm)"
elif [[ -x "$runtime_root/bin/node" && -x "$runtime_root/bin/npm" ]]; then
  node_cmd="$runtime_root/bin/node"
  npm_cmd="$runtime_root/bin/npm"
elif [[ "$offline" -eq 1 ]]; then
  echo "offline mode: private Node runtime is missing at $runtime_root" >&2
  exit 1
else
  temporary="$(mktemp -d)"
  trap 'rm -rf "${temporary:-}"' EXIT
  archive="node-v${PTYMARK_MANAGED_NODE_VERSION}-${platform}-${architecture}.tar.xz"
  base_url="https://nodejs.org/dist/v${PTYMARK_MANAGED_NODE_VERSION}"
  download "$base_url/$archive" "$temporary/$archive"
  download "$base_url/SHASUMS256.txt" "$temporary/SHASUMS256.txt"
  expected="$(awk -v name="$archive" '$2 == name { print $1 }' "$temporary/SHASUMS256.txt")"
  [[ -n "$expected" ]] || { echo "Node checksum entry is missing for $archive" >&2; exit 1; }
  actual="$(sha256_file "$temporary/$archive")"
  [[ "$actual" == "$expected" ]] || { echo "Node archive checksum mismatch for $archive" >&2; exit 1; }
  rm -rf "$runtime_root"
  mkdir -p "$runtime_root"
  tar -xJf "$temporary/$archive" --strip-components=1 -C "$runtime_root"
  node_cmd="$runtime_root/bin/node"
  npm_cmd="$runtime_root/bin/npm"
fi
node_cmd="$(cd "$(dirname "$node_cmd")" && pwd -P)/$(basename "$node_cmd")"

if [[ -n "$browser_path" ]]; then
  browser_path="$(cd "$(dirname "$browser_path")" && pwd -P)/$(basename "$browser_path")"
  [[ -x "$browser_path" ]] || { printf 'browser is not executable: %s\n' "$browser_path" >&2; exit 1; }
elif [[ "$skip_browser_download" -eq 1 ]]; then
  for candidate in chromium chromium-browser google-chrome google-chrome-stable microsoft-edge microsoft-edge-stable; do
    if command -v "$candidate" >/dev/null 2>&1; then
      browser_path="$(command -v "$candidate")"
      break
    fi
  done
  if [[ -z "$browser_path" && "$platform" == darwin ]]; then
    for candidate in \
      '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome' \
      '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge' \
      '/Applications/Chromium.app/Contents/MacOS/Chromium'; do
      if [[ -x "$candidate" ]]; then browser_path="$candidate"; break; fi
    done
  fi
  [[ -n "$browser_path" ]] || {
    echo 'no Chromium-compatible browser found while --skip-browser-download is active' >&2
    exit 1
  }
fi

browser_no_sandbox=0
browser_no_sandbox_toml=false
if [[ "${PTYMARK_BROWSER_NO_SANDBOX:-0}" == 1 || -f /.dockerenv ]]; then
  browser_no_sandbox=1
  browser_no_sandbox_toml=true
fi

lock_sha="$(sha256_file "$repo_root/renderers/package-lock.json")"
launcher_sha="$(sha256_file "$launcher")"
expected_stamp="bundle=$bundle_id
lock_sha=$lock_sha
launcher_sha=$launcher_sha
node=$node_cmd
browser=${browser_path:-puppeteer-managed}
browser_no_sandbox=$browser_no_sandbox"
installed=0
if [[ "$force" -eq 0 && -f "$stamp_path" ]] \
  && [[ "$(cat "$stamp_path")" == "$expected_stamp" ]] \
  && [[ -d "$app_root/node_modules" ]]; then
  installed=1
fi

if [[ "$installed" -eq 0 ]]; then
  [[ "$offline" -eq 0 ]] || { echo 'offline mode: managed renderer app is incomplete' >&2; exit 1; }
  rm -rf "$app_root"
  mkdir -p "$app_root/managed"
  cp "$repo_root/renderers/package.json" "$app_root/package.json"
  cp "$repo_root/renderers/package-lock.json" "$app_root/package-lock.json"
  cp "$repo_root/renderers/managed/mathjax-cli.mjs" "$app_root/managed/mathjax-cli.mjs"
  cp "$repo_root/renderers/managed/ansi-presenter.mjs" "$app_root/managed/ansi-presenter.mjs"
  export PUPPETEER_CACHE_DIR="$cache_root"
  export npm_config_cache="$managed_root/cache/npm"
  if [[ "$skip_browser_download" -eq 1 || -n "$browser_path" ]]; then
    export PUPPETEER_SKIP_DOWNLOAD=true
  else
    unset PUPPETEER_SKIP_DOWNLOAD || true
  fi
  "$npm_cmd" ci --prefix "$app_root" --omit=dev --no-audit --no-fund
  if [[ -z "$browser_path" && "$skip_browser_download" -eq 0 ]]; then
    "$node_cmd" "$app_root/node_modules/puppeteer/install.mjs"
  fi
fi

"$node_cmd" - "$puppeteer_config_path" "$browser_path" "$browser_no_sandbox" <<'NODE'
import fs from 'node:fs';
const [output, executablePath, noSandbox] = process.argv.slice(2);
const config = { headless: true };
if (executablePath) config.executablePath = executablePath;
if (noSandbox === '1') {
  config.args = ['--no-sandbox', '--disable-setuid-sandbox'];
}
fs.writeFileSync(output, `${JSON.stringify(config)}\n`, 'utf8');
NODE

install -m 755 "$launcher" "$bin_root/mmdc"
install -m 755 "$launcher" "$bin_root/tex2svg"
install -m 755 "$launcher" "$bin_root/chafa"

toml_quote() {
  local value="$1"
  value="${value//\\/\\\\}"
  value="${value//\"/\\\"}"
  printf '"%s"' "$value"
}
{
  echo 'schema_version = 1'
  printf 'node_path = '; toml_quote "$node_cmd"; echo
  printf 'app_root = '; toml_quote "$app_root"; echo
  printf 'cache_root = '; toml_quote "$cache_root"; echo
  printf 'puppeteer_config_path = '; toml_quote "$puppeteer_config_path"; echo
  if [[ -n "$browser_path" ]]; then
    printf 'browser_path = '; toml_quote "$browser_path"; echo
  fi
  printf 'browser_no_sandbox = %s\n' "$browser_no_sandbox_toml"
} >"$manifest_path"
printf '%s' "$expected_stamp" >"$stamp_path"

printf 'root\t%s\n' "$managed_root"
printf 'node\t%s\n' "$node_cmd"
printf 'mermaid\t%s\n' "$bin_root/mmdc"
printf 'math\t%s\n' "$bin_root/tex2svg"
printf 'presenter\t%s\n' "$bin_root/chafa"
printf 'browser\t%s\n' "${browser_path:-puppeteer-managed}"
