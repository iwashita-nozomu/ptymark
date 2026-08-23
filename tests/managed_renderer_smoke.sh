#!/usr/bin/env bash

# @dependency-start
# contract test
# responsibility Proves isolated Mermaid, math, and presenter execution.
# upstream implementation ../scripts/install-managed-bundle.sh bundle installation
# upstream implementation ../src/managed_launcher.rs role execution
# upstream implementation ../renderers/managed/ansi-presenter.mjs terminal-cell presentation
# downstream environment ../.github/workflows/ptymark-ci.yml evidence recording
# @dependency-end

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
binary="${1:-${CARGO_TARGET_DIR:-target}/debug/ptymark}"
browser="${2:-${PTYMARK_TEST_BROWSER:-}}"

[[ -x "$binary" ]] || {
  printf 'ptymark binary is not executable: %s\n' "$binary" >&2
  exit 1
}
binary="$(cd "$(dirname "$binary")" && pwd -P)/$(basename "$binary")"

root="$(mktemp -d)"
trap 'rm -rf "$root"' EXIT
bundle="$root/bundle"
config="$root/config.toml"
state="$root/state.toml"

installer_args=(
  --skip-core
  --binary "$binary"
  --managed always
  --managed-root "$bundle"
  --config "$config"
  --state "$state"
)
if [[ -n "$browser" ]]; then
  installer_args+=(--browser "$browser" --skip-browser-download)
fi

PTYMARK_BROWSER_NO_SANDBOX="${PTYMARK_BROWSER_NO_SANDBOX:-1}" \
  bash "$repo_root/scripts/installer.sh" "${installer_args[@]}"

export PTYMARK_CONFIG="$config"
export PTYMARK_INSTALL_STATE="$state"

"$binary" --config "$config" config check
"$binary" --config "$config" engine check
"$binary" install status --state "$state"

cat >"$root/diagram.mmd" <<'EOF_MERMAID_BODY'
flowchart LR
  Install --> Resolve --> Render
EOF_MERMAID_BODY
"$bundle/bin/mmdc" \
  --input "$root/diagram.mmd" \
  --output "$root/direct-mermaid.svg"
test -s "$root/direct-mermaid.svg"
grep -F '<svg' "$root/direct-mermaid.svg" >/dev/null

formula_names=(einstein fraction integral)
formula_sources=(
  'E = mc^2'
  '\frac{a+b}{c+d}'
  '\int_0^1 x^2\,dx'
)

for index in "${!formula_names[@]}"; do
  name="${formula_names[$index]}"
  source="${formula_sources[$index]}"
  "$bundle/bin/tex2svg" "$source" >"$root/$name.svg"
  test -s "$root/$name.svg"
  grep -F '<svg' "$root/$name.svg" >/dev/null

  for appearance in dark light; do
    for term in xterm-256color tmux-256color; do
      TERM="$term" \
      PTYMARK_APPEARANCE="$appearance" \
        "$bundle/bin/chafa" \
          --format symbols \
          --probe off \
          --polite on \
          --relative off \
          --animate off \
          --colors full \
          --size 80x \
          "$root/$name.svg" \
          >"$root/$name-$appearance-$term.out"
      test -s "$root/$name-$appearance-$term.out"
    done
  done

  TERM=xterm-256color \
  PTYMARK_APPEARANCE=dark \
    "$bundle/bin/chafa" \
      --format symbols \
      --probe off \
      --polite on \
      --relative off \
      --animate off \
      --colors full \
      --size 40x \
      "$root/$name.svg" \
      >"$root/$name-40-dark-xterm-256color.out"
  test -s "$root/$name-40-dark-xterm-256color.out"
done

node - "$root" "${formula_names[@]}" <<'NODE'
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';

const [root, ...names] = process.argv.slice(2);
const requirements = {
  einstein: {
    minOccupied: 18,
    minColumns: 8,
    minHalfRows: 3,
    minHorizontalRun: 2,
    minVerticalRun: 2,
  },
  fraction: {
    minOccupied: 18,
    minColumns: 6,
    minHalfRows: 5,
    minHorizontalRun: 5,
    minVerticalRun: 2,
  },
  integral: {
    minOccupied: 18,
    minColumns: 6,
    minHalfRows: 5,
    minHorizontalRun: 2,
    minVerticalRun: 4,
  },
};

function stripAnsi(value) {
  return value.replace(/\x1b\[[0-9;]*m/g, '');
}

function geometryAndMetrics(value) {
  const geometry = stripAnsi(value);
  const lines = geometry.replace(/\n$/, '').split('\n');
  const width = Math.max(...lines.map((line) => [...line].length));
  const height = lines.length * 2;
  const occupied = new Array(width * height).fill(false);

  for (let row = 0; row < lines.length; row += 1) {
    for (const [column, glyph] of [...lines[row]].entries()) {
      if (glyph === '█' || glyph === '▀') occupied[(row * 2) * width + column] = true;
      if (glyph === '█' || glyph === '▄') occupied[(row * 2 + 1) * width + column] = true;
      assert.ok(
        glyph === ' ' || glyph === '█' || glyph === '▀' || glyph === '▄',
        `unexpected presenter glyph ${JSON.stringify(glyph)}`,
      );
    }
  }

  const at = (x, y) => x >= 0 && x < width && y >= 0 && y < height
    && occupied[y * width + x];
  let occupiedSubcells = 0;
  let isolatedSubcells = 0;
  let occupiedColumns = 0;
  let occupiedHalfRows = 0;
  let maxHorizontalRun = 0;
  let maxVerticalRun = 0;

  for (let y = 0; y < height; y += 1) {
    let rowOccupied = false;
    let run = 0;
    for (let x = 0; x < width; x += 1) {
      if (!at(x, y)) {
        run = 0;
        continue;
      }
      occupiedSubcells += 1;
      rowOccupied = true;
      run += 1;
      maxHorizontalRun = Math.max(maxHorizontalRun, run);
      let hasNeighbor = false;
      for (let neighborY = y - 1; neighborY <= y + 1; neighborY += 1) {
        for (let neighborX = x - 1; neighborX <= x + 1; neighborX += 1) {
          if ((neighborX !== x || neighborY !== y) && at(neighborX, neighborY)) {
            hasNeighbor = true;
          }
        }
      }
      if (!hasNeighbor) isolatedSubcells += 1;
    }
    if (rowOccupied) occupiedHalfRows += 1;
  }

  for (let x = 0; x < width; x += 1) {
    let columnOccupied = false;
    let run = 0;
    for (let y = 0; y < height; y += 1) {
      if (!at(x, y)) {
        run = 0;
        continue;
      }
      columnOccupied = true;
      run += 1;
      maxVerticalRun = Math.max(maxVerticalRun, run);
    }
    if (columnOccupied) occupiedColumns += 1;
  }

  return {
    geometry,
    occupiedSubcells,
    isolatedRatio: isolatedSubcells / occupiedSubcells,
    occupiedColumns,
    occupiedHalfRows,
    maxHorizontalRun,
    maxVerticalRun,
  };
}

for (const name of names) {
  const variants = [];
  for (const appearance of ['dark', 'light']) {
    for (const term of ['xterm-256color', 'tmux-256color']) {
      const file = path.join(root, `${name}-${appearance}-${term}.out`);
      variants.push(geometryAndMetrics(fs.readFileSync(file, 'utf8')));
    }
  }
  assert.equal(
    new Set(variants.map(({ geometry }) => geometry)).size,
    1,
    `${name}: dark/light or tmux changed terminal-cell geometry`,
  );

  const metrics = variants[0];
  const expected = requirements[name];
  assert.ok(metrics.occupiedSubcells >= expected.minOccupied, `${name}: sparse output`);
  assert.ok(metrics.isolatedRatio <= 0.3, `${name}: fragmented output`);
  assert.ok(metrics.occupiedColumns >= expected.minColumns, `${name}: insufficient width`);
  assert.ok(metrics.occupiedHalfRows >= expected.minHalfRows, `${name}: insufficient height`);
  assert.ok(
    metrics.maxHorizontalRun >= expected.minHorizontalRun,
    `${name}: horizontal stroke coverage lost`,
  );
  assert.ok(
    metrics.maxVerticalRun >= expected.minVerticalRun,
    `${name}: vertical stroke coverage lost`,
  );

  const narrow = geometryAndMetrics(fs.readFileSync(
    path.join(root, `${name}-40-dark-xterm-256color.out`),
    'utf8',
  ));
  assert.ok(narrow.occupiedSubcells >= 8, `${name}: 40-column output is too sparse`);
  assert.ok(narrow.isolatedRatio <= 0.3, `${name}: 40-column output is fragmented`);
  assert.ok(narrow.occupiedColumns >= 4, `${name}: 40-column structure lost`);
  assert.ok(narrow.occupiedHalfRows >= 3, `${name}: 40-column height lost`);
}
NODE

PTYMARK_PRESENTER_DIAGNOSTICS=1 \
PTYMARK_APPEARANCE=dark \
  "$bundle/bin/chafa" \
    --format symbols \
    --colors full \
    --size 80x \
    "$root/einstein.svg" \
    >"$root/diagnostic.out" \
    2>"$root/diagnostic.err"
grep -F 'terminal=80x' "$root/diagnostic.err" >/dev/null
grep -F 'cell_aspect=0.5' "$root/diagnostic.err" >/dev/null
grep -F 'raster_scale=4' "$root/diagnostic.err" >/dev/null
grep -F 'isolated_ratio=' "$root/diagnostic.err" >/dev/null

"$bundle/bin/chafa" \
  --format symbols \
  --probe off \
  --polite on \
  --relative off \
  --animate off \
  --colors none \
  --size 48x \
  "$root/direct-mermaid.svg" \
  >"$root/direct-presenter.txt"
test -s "$root/direct-presenter.txt"

cat >"$root/mermaid.md" <<'EOF_MERMAID'
```mermaid
flowchart LR
  A --> B
```
EOF_MERMAID
"$binary" --config "$config" preview --strict --columns 48 "$root/mermaid.md" \
  >"$root/mermaid.out"
test -s "$root/mermaid.out"
if grep -F '```mermaid' "$root/mermaid.out" >/dev/null; then
  echo 'strict Mermaid preview left the source fence unchanged' >&2
  exit 1
fi

cat >"$root/math.md" <<'EOF_MATH'
計算前
$$
\frac{a+b}{c+d} + x^2
$$
計算後
EOF_MATH
"$binary" --config "$config" preview --strict --columns 80 "$root/math.md" \
  >"$root/math.out"
test -s "$root/math.out"
if grep -F '\frac{a+b}{c+d} + x^2' "$root/math.out" >/dev/null; then
  echo 'strict MathJax preview left the source expression unchanged' >&2
  exit 1
fi
grep -F '計算前' "$root/math.out" >/dev/null
grep -F '計算後' "$root/math.out" >/dev/null

interactive_script=$(cat <<'EOF_INTERACTIVE_SCRIPT'
printf 'before\n```mermaid\nflowchart LR\n  Interactive --> PTY --> Renderer\n```\n$$\nE = mc^2\n$$\nafter\n'
EOF_INTERACTIVE_SCRIPT
)
interactive_outputs=()
for appearance in dark light; do
  for term in xterm-256color tmux-256color; do
    output="$root/interactive-$appearance-$term.out"
    TERM="$term" \
    PTYMARK_APPEARANCE="$appearance" \
      "$binary" --config "$config" shell --strict -- /bin/sh -c "$interactive_script" \
        >"$output"
    test -s "$output"
    if grep -F '```mermaid' "$output" >/dev/null \
      || grep -F '$$' "$output" >/dev/null; then
      echo "interactive PTY path fell back to semantic source: $appearance/$term" >&2
      exit 1
    fi
    interactive_outputs+=("$output")
  done
done

for output in "${interactive_outputs[@]:1}"; do
  cmp "${interactive_outputs[0]}" "$output"
done

printf 'ptymark managed renderer and real-PTY smoke: ok\n'
