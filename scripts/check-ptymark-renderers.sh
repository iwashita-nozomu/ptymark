#!/usr/bin/env bash
set -euo pipefail

work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT

cat >"$work_dir/diagram.mmd" <<'EOF'
flowchart LR
    PTY --> Detector
    Detector --> Renderer
    Renderer --> Display
EOF

mmdc \
  --input "$work_dir/diagram.mmd" \
  --output "$work_dir/diagram.svg" \
  --backgroundColor transparent

test -s "$work_dir/diagram.svg"
grep -F '<svg' "$work_dir/diagram.svg" >/dev/null

printf '%s\n' '\frac{-b \pm \sqrt{b^2 - 4ac}}{2a}' \
  | katex --display-mode --format mathml \
  >"$work_dir/equation.mathml"

test -s "$work_dir/equation.mathml"
grep -F '<math' "$work_dir/equation.mathml" >/dev/null

cat >"$work_dir/equation.typ" <<'EOF'
#set page(width: auto, height: auto, margin: 4pt)
$ E = m c^2 $
EOF

typst compile "$work_dir/equation.typ" "$work_dir/equation.svg"
test -s "$work_dir/equation.svg"
grep -F '<svg' "$work_dir/equation.svg" >/dev/null

printf 'ptymark existing-engine smoke: ok\n'
