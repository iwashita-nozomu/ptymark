#!/usr/bin/env bash

# @dependency-start
# contract test
# responsibility Proves isolated Mermaid, math, presenter, and managed runtime diagnosis.
# upstream implementation ../scripts/install-managed-bundle.sh bundle installation
# upstream implementation ../src/managed_launcher.rs role execution and bounded runtime probe
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

normal_doctor="$root/doctor-normal.json"
tmux_doctor="$root/doctor-tmux.json"
env -u TMUX TERM=xterm-256color \
  "$binary" --config "$config" doctor --json >"$normal_doctor"
TMUX="$root/tmux-socket,1,0" TERM=tmux-256color \
  "$binary" --config "$config" doctor --json >"$tmux_doctor"
for report in "$normal_doctor" "$tmux_doctor"; do
  test "$(grep -c '"runtime_state": "ready"' "$report")" -eq 3
  grep -F '"browser_state": "present"' "$report" >/dev/null
  if grep -F 'browser.runtime_' "$report" >/dev/null; then
    echo 'healthy managed bundle produced a browser runtime finding' >&2
    exit 1
  fi
done

cat >"$root/diagram.mmd" <<'EOF_MERMAID_BODY'
flowchart LR
  Install --> Resolve --> Render
EOF_MERMAID_BODY
"$bundle/bin/mmdc" \
  --input "$root/diagram.mmd" \
  --output "$root/direct-mermaid.svg"
test -s "$root/direct-mermaid.svg"
grep -F '<svg' "$root/direct-mermaid.svg" >/dev/null

"$bundle/bin/tex2svg" 'E = mc^2' >"$root/direct-math.svg"
test -s "$root/direct-math.svg"
grep -F '<svg' "$root/direct-math.svg" >/dev/null

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
$$
E = mc^2
$$
EOF_MATH
"$binary" --config "$config" preview --strict --columns 48 "$root/math.md" \
  >"$root/math.out"
test -s "$root/math.out"
if grep -F 'E = mc^2' "$root/math.out" >/dev/null; then
  echo 'strict MathJax preview left the source expression unchanged' >&2
  exit 1
fi

interactive_script=$(cat <<'EOF_INTERACTIVE_SCRIPT'
printf 'before\n```mermaid\nflowchart LR\n  Interactive --> PTY --> Renderer\n```\n$$\nE = mc^2\n$$\nafter\n'
EOF_INTERACTIVE_SCRIPT
)
"$binary" --config "$config" shell --strict -- /bin/sh -c "$interactive_script" \
  >"$root/interactive-managed.out"
test -s "$root/interactive-managed.out"
if grep -F '```mermaid' "$root/interactive-managed.out" >/dev/null \
  || grep -F '$$' "$root/interactive-managed.out" >/dev/null; then
  echo 'interactive PTY path fell back to semantic source' >&2
  exit 1
fi

printf 'ptymark managed renderer and real-PTY smoke: ok\n'
