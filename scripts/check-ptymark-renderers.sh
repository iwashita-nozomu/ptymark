#!/usr/bin/env bash
set -euo pipefail

renderer_root="${PTYMARK_RENDERER_ROOT:-/opt/ptymark-renderers}"
work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT

node "$renderer_root/check.mjs"

cat >"$work_dir/diagram.mmd" <<'MMD'
flowchart LR
  Output --> SafetyGate --> Detector --> Renderer --> Display
MMD

mmdc \
  --input "$work_dir/diagram.mmd" \
  --output "$work_dir/diagram.svg" \
  --backgroundColor transparent

test -s "$work_dir/diagram.svg"
grep -F '<svg' "$work_dir/diagram.svg" >/dev/null

printf 'ptymark Mermaid smoke: ok\n'
