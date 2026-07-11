#!/usr/bin/env bash
# @dependency-start
# contract test
# responsibility Verifies selected existing renderer engines and artifact formats.
# upstream environment ../renderers/package-lock.json pins JavaScript engines.
# upstream implementation ../renderers/check.mjs checks MathJax, Mermaid, and KaTeX.
# downstream workflow ../.github/workflows/ptymark-ci.yml runs correctness before benchmarks.
# @dependency-end
set -euo pipefail

renderer_root="${PTYMARK_RENDERER_ROOT:-/opt/ptymark-renderers}"
work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT

node "$renderer_root/check.mjs"

cat >"$work_dir/diagram.mmd" <<'MMD'
flowchart LR
    PTY --> Gate
    Gate --> Detector
    Detector --> Coordinator
    Coordinator --> Presenter
    Presenter --> Terminal
MMD
mmdc \
  --input "$work_dir/diagram.mmd" \
  --output "$work_dir/diagram.svg" \
  --backgroundColor transparent

test -s "$work_dir/diagram.svg"
grep -F '<svg' "$work_dir/diagram.svg" >/dev/null

cat >"$work_dir/equation.typ" <<'TYP'
#set page(width: auto, height: auto, margin: 4pt)
$ E = m c^2 $
TYP
typst compile "$work_dir/equation.typ" "$work_dir/equation.svg"
test -s "$work_dir/equation.svg"
grep -F '<svg' "$work_dir/equation.svg" >/dev/null

printf 'ptymark selected renderer engines: ok\n'
