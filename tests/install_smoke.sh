#!/usr/bin/env bash

# @dependency-start
# contract test
# responsibility Exercises source and package-local installation.
# upstream implementation ../scripts/installer.sh source installation
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

grep -F 'backend = "mermaid-cli"' "$config" >/dev/null
grep -F 'backend = "mathjax-cli"' "$config" >/dev/null
grep -F "path = \"$fake_bin/mmdc\"" "$config" >/dev/null
grep -F "path = \"$fake_bin/tex2svg\"" "$config" >/dev/null
grep -F "path = \"$fake_bin/chafa\"" "$config" >/dev/null

{
  printf '%s\n' '```mermaid' 'A --> B' '```'
} | env PTYMARK_CONFIG="$config" "$binary" preview \
  | grep -F 'installed-engine-output' >/dev/null

# The old name remains a compatibility wrapper and must preserve one-slot updates.
bash scripts/install.sh \
  --skip-core \
  --binary "$binary" \
  --config "$config" \
  --state "$state" \
  --mermaid source

grep -F 'backend = "source"' "$config" >/dev/null
grep -F 'backend = "mathjax-cli"' "$config" >/dev/null
"$binary" install status --state "$state" | grep -F $'math\tmathjax-cli\tready' >/dev/null

printf 'ptymark installer smoke: ok\n'
