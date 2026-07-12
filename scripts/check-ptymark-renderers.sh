#!/usr/bin/env bash
set -euo pipefail

renderer_root="${PTYMARK_RENDERER_ROOT:-/opt/ptymark-renderers}"
work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT

node "$renderer_root/check.mjs"

cat >"$work_dir/diagram.mmd" <<'MMD'
flowchart LR
  Output --> SafetyGate --> Detector --> Renderer --> Presenter --> Display
MMD

mmdc \
  --input "$work_dir/diagram.mmd" \
  --output "$work_dir/diagram.svg" \
  --backgroundColor transparent

test -s "$work_dir/diagram.svg"
grep -F '<svg' "$work_dir/diagram.svg" >/dev/null

chafa \
  --format symbols \
  --probe off \
  --polite on \
  --relative off \
  --animate off \
  --colors none \
  --size 60x \
  "$work_dir/diagram.svg" \
  >"$work_dir/diagram.txt"

test -s "$work_dir/diagram.txt"

cargo run --quiet --locked -- \
  --config examples/external-engines.toml \
  engine check \
  >"$work_dir/engine-check.txt"
grep -F $'mermaid\tmermaid-cli' "$work_dir/engine-check.txt" >/dev/null
grep -F $'presenter\tchafa-symbols' "$work_dir/engine-check.txt" >/dev/null

cat <<'MARKDOWN' | cargo run --quiet --locked -- \
  --config examples/external-engines.toml \
  preview \
  >"$work_dir/ptymark-display.txt"
```mermaid
flowchart LR
  Installed --> Selected --> Rendered
```
MARKDOWN

test -s "$work_dir/ptymark-display.txt"
if grep -F '```mermaid' "$work_dir/ptymark-display.txt" >/dev/null; then
  echo "external Mermaid block was not replaced" >&2
  exit 1
fi

printf 'ptymark Mermaid CLI + Chafa integration: ok\n'