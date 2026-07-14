#!/usr/bin/env bash

# @dependency-start
# contract tool
# responsibility Checks renderer executables and bounded direct rendering.
# upstream implementation ../src/engine.rs executable checks
# upstream implementation ../src/managed_launcher.rs managed role launch
# downstream implementation ../tests/managed_renderer_smoke.sh renderer validation
# @dependency-end

set -euo pipefail

renderer_root="${PTYMARK_RENDERER_ROOT:-/opt/ptymark-renderers}"
work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT

node "$renderer_root/check.mjs"

real_mmdc="$(command -v mmdc)"
real_chafa="$(command -v chafa)"
browser="${PUPPETEER_EXECUTABLE_PATH:-}"
if [[ -z "$browser" ]]; then
  for candidate in chromium chromium-browser google-chrome google-chrome-stable; do
    if command -v "$candidate" >/dev/null 2>&1; then
      browser="$(command -v "$candidate")"
      break
    fi
  done
fi
[[ -n "$browser" ]] || {
  echo 'no Chromium-compatible browser is available for the Mermaid smoke test' >&2
  exit 1
}

node - "$work_dir/puppeteer-config.json" "$browser" <<'NODE'
import fs from 'node:fs';

const [output, executablePath] = process.argv.slice(2);
const config = { headless: true, executablePath };
if (process.env.PTYMARK_BROWSER_NO_SANDBOX === '1' || fs.existsSync('/.dockerenv')) {
  config.args = ['--no-sandbox', '--disable-setuid-sandbox'];
}
fs.writeFileSync(output, `${JSON.stringify(config)}\n`, 'utf8');
NODE

cat >"$work_dir/mmdc-wrapper" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
exec "$PTYMARK_REAL_MMDC" \
  --puppeteerConfigFile "$PTYMARK_TEST_PUPPETEER_CONFIG" \
  "$@"
SH
chmod 755 "$work_dir/mmdc-wrapper"

cat >"$work_dir/diagram.mmd" <<'MMD'
flowchart LR
  Output --> SafetyGate --> Detector --> Renderer --> Presenter --> Display
MMD

PTYMARK_REAL_MMDC="$real_mmdc" \
PTYMARK_TEST_PUPPETEER_CONFIG="$work_dir/puppeteer-config.json" \
  "$work_dir/mmdc-wrapper" \
    --input "$work_dir/diagram.mmd" \
    --output "$work_dir/diagram.svg" \
    --backgroundColor transparent

test -s "$work_dir/diagram.svg"
grep -F '<svg' "$work_dir/diagram.svg" >/dev/null

"$real_chafa" \
  --format symbols \
  --colors none \
  --size 60x \
  "$work_dir/diagram.svg" \
  >"$work_dir/diagram.txt"

test -s "$work_dir/diagram.txt"

cat >"$work_dir/external-engines.toml" <<EOF_CONFIG
schema_version = 1

[rendering]
mode = "preview"
strict = true
columns = 80

[engines.mermaid]
backend = "mermaid-cli"
path = "$work_dir/mmdc-wrapper"

[engines.math]
backend = "preview"
path = "tex2svg"

[engines.presenter]
path = "$real_chafa"
EOF_CONFIG

PTYMARK_REAL_MMDC="$real_mmdc" \
PTYMARK_TEST_PUPPETEER_CONFIG="$work_dir/puppeteer-config.json" \
  cargo run --quiet --locked -- \
    --config "$work_dir/external-engines.toml" \
    engine check \
    >"$work_dir/engine-check.txt"
grep -F $'mermaid\tmermaid-cli' "$work_dir/engine-check.txt" >/dev/null
grep -F $'presenter\tchafa-symbols' "$work_dir/engine-check.txt" >/dev/null

PTYMARK_REAL_MMDC="$real_mmdc" \
PTYMARK_TEST_PUPPETEER_CONFIG="$work_dir/puppeteer-config.json" \
  cargo run --quiet --locked -- \
    --config "$work_dir/external-engines.toml" \
    preview --strict \
    >"$work_dir/ptymark-display.txt" <<'MARKDOWN'
```mermaid
flowchart LR
  Installed --> Selected --> Rendered
```
MARKDOWN

test -s "$work_dir/ptymark-display.txt"
if grep -F '```mermaid' "$work_dir/ptymark-display.txt" >/dev/null; then
  echo 'external Mermaid block was not replaced' >&2
  exit 1
fi

printf 'ptymark Mermaid CLI + Chafa integration: ok\n'
