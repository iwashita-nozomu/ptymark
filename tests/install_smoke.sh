#!/usr/bin/env bash

# @dependency-start
# contract test
# responsibility Exercises source and package-local installation.
# upstream implementation ../scripts/installer.sh source installation
# upstream implementation ../scripts/install-managed-bundle.sh private runtime installation
# upstream implementation ../distribution/install.sh package installation
# downstream environment ../.github/workflows/ptymark-ci.yml test execution
# @dependency-end

set -euo pipefail

binary="${1:-${CARGO_TARGET_DIR:-target}/debug/ptymark}"
if [[ ! -x "$binary" ]]; then
  printf 'ptymark test binary is not executable: %s\n' "$binary" >&2
  exit 1
fi
binary="$(cd "$(dirname "$binary")" && pwd -P)/$(basename "$binary")"

root="$(mktemp -d)"
trap 'rm -rf "$root"' EXIT
fake_bin="$root/bin"
config="$root/config/ptymark.toml"
state="$root/state/install.toml"
mkdir -p "$fake_bin"

cat >"$fake_bin/mmdc" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
output=""
while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --output)
      output="$2"
      shift 2
      ;;
    *)
      shift
      ;;
  esac
done
cat >/dev/null
printf '<svg xmlns="http://www.w3.org/2000/svg"></svg>\n' >"$output"
EOF

cat >"$fake_bin/tex2svg" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
printf '<svg xmlns="http://www.w3.org/2000/svg"><text>%s</text></svg>\n' "${1:-}"
EOF

cat >"$fake_bin/chafa" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
printf 'installed-engine-output\n'
EOF
chmod +x "$fake_bin/mmdc" "$fake_bin/tex2svg" "$fake_bin/chafa"

bash scripts/installer.sh \
  --skip-core \
  --binary "$binary" \
  --config "$config" \
  --state "$state" \
  --mermaid "$fake_bin/mmdc" \
  --math "$fake_bin/tex2svg" \
  --presenter "$fake_bin/chafa"

grep -F 'schema_version = 2' "$config" >/dev/null
test "$(grep -F -c 'provider = "external"' "$config")" -eq 3
grep -F "program = \"$fake_bin/mmdc\"" "$config" >/dev/null
grep -F "program = \"$fake_bin/tex2svg\"" "$config" >/dev/null
grep -F "program = \"$fake_bin/chafa\"" "$config" >/dev/null
grep -F 'backend = "mermaid-cli"' "$state" >/dev/null
grep -F 'backend = "mathjax-cli"' "$state" >/dev/null

{
  printf '%s\n' '```mermaid' 'A --> B' '```'
} | env PTYMARK_CONFIG="$config" PTYMARK_INSTALL_STATE="$state" \
  "$binary" preview | grep -F 'installed-engine-output' >/dev/null

# The old name remains a compatibility wrapper and must preserve one-slot updates.
bash scripts/install.sh \
  --skip-core \
  --binary "$binary" \
  --config "$config" \
  --state "$state" \
  --mermaid source

grep -F 'provider = "source"' "$config" >/dev/null
grep -F "program = \"$fake_bin/tex2svg\"" "$config" >/dev/null
"$binary" install status --state "$state" | grep -F $'math\tmathjax-cli\tready' >/dev/null

# A private Node runtime must execute npm even when no global node command is
# discoverable. The fixture models npm's `#!/usr/bin/env node` entrypoint and
# fails on the historical implementation because the private runtime was not
# exposed to that child process.
# shellcheck disable=SC1091
source renderers/managed-bundle.env
case "$(uname -s)" in
  Darwin) platform=darwin ;;
  Linux) platform=linux ;;
  *) platform=unsupported ;;
esac
case "$(uname -m)" in
  x86_64|amd64) architecture=x64 ;;
  arm64|aarch64) architecture=arm64 ;;
  *) architecture=unsupported ;;
esac

if [[ "$platform" != unsupported && "$architecture" != unsupported ]]; then
  private_root="$root/private-node-bundle"
  runtime_bin="$private_root/runtime/node-v${PTYMARK_MANAGED_NODE_VERSION}-${platform}-${architecture}/bin"
  no_node_path="$root/no-global-node-path"
  mkdir -p "$runtime_bin" "$no_node_path"

  cat >"$runtime_bin/node" <<'EOF_PRIVATE_NODE'
#!/bin/sh
set -eu
case "${1:-}" in
  -)
    output="${2:?missing Puppeteer config path}"
    printf '{"headless":true}\n' >"$output"
    ;;
  */npm)
    shift
    prefix=""
    while [ "$#" -gt 0 ]; do
      case "$1" in
        --prefix)
          prefix="${2:?missing npm prefix}"
          shift 2
          ;;
        *) shift ;;
      esac
    done
    mkdir -p "${prefix:?missing npm prefix}/node_modules"
    ;;
  *)
    printf 'unexpected fake node entrypoint: %s\n' "${1:-}" >&2
    exit 1
    ;;
esac
EOF_PRIVATE_NODE
  cat >"$runtime_bin/npm" <<'EOF_PRIVATE_NPM'
#!/usr/bin/env node
EOF_PRIVATE_NPM
  chmod +x "$runtime_bin/node" "$runtime_bin/npm"

  for command_name in awk basename cat cp dirname install mkdir rm sha256sum uname; do
    command_path="$(command -v "$command_name")"
    ln -s "$command_path" "$no_node_path/$command_name"
  done

  PATH="$no_node_path" /bin/bash scripts/install-managed-bundle.sh \
    --root "$private_root" \
    --launcher "$binary" \
    --browser /bin/true \
    --skip-browser-download

  test -s "$private_root/bundle.toml"
  test -s "$private_root/bundle.stamp"
  test -x "$private_root/bin/mmdc"
  test -x "$private_root/bin/tex2svg"
  test -x "$private_root/bin/chafa"
fi

# A managed-bundle failure must occur before the Rust resolver can atomically
# commit configuration or install state, and the message must identify the
# partial core-only outcome.
failed_config="$root/failed/config.toml"
failed_state="$root/failed/install.toml"
failed_log="$root/failed-install.log"
if bash scripts/installer.sh \
  --skip-core \
  --binary "$binary" \
  --managed always \
  --managed-root "$root/offline-incomplete-bundle" \
  --offline \
  --config "$failed_config" \
  --state "$failed_state" \
  >"$failed_log" 2>&1; then
  echo 'incomplete offline managed bundle unexpectedly succeeded' >&2
  exit 1
fi
test ! -e "$failed_config"
test ! -e "$failed_state"
grep -F 'configuration/install state was not committed' "$failed_log" >/dev/null

printf 'ptymark installer smoke: ok\n'
