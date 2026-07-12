#!/usr/bin/env bash
# @dependency-start
# contract test
# responsibility Measures renderer workers and coordinator cache latency and enforces CI budgets.
# upstream implementation ../renderers/benchmark.mjs measures selected existing engines.
# upstream implementation ../examples/benchmark_core.rs measures Rust coordinator cache hits.
# downstream workflow ../.github/workflows/ptymark-ci.yml uploads benchmark JSON evidence.
# @dependency-end
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

out_dir="${1:-reports/benchmarks}"
renderer_root="${PTYMARK_RENDERER_ROOT:-/opt/ptymark-renderers}"
mkdir -p "$out_dir"

node "$renderer_root/benchmark.mjs" >"$out_dir/renderer.json"
cargo run --quiet --locked --release --example benchmark_core >"$out_dir/core.json"
python3 scripts/check-ptymark-benchmarks.py \
  "$out_dir/renderer.json" \
  "$out_dir/core.json" \
  | tee "$out_dir/budget.txt"

printf 'ptymark benchmark artifacts: %s\n' "$out_dir"
