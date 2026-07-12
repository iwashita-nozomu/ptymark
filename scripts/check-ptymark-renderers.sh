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
printf 'ptymark Mermaid + Chafa smoke: ok\n'