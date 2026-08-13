#!/usr/bin/env bash

# @dependency-start
# contract test
# responsibility Proves the complete isolated Linux source-install and real-engine user journey.
# upstream implementation ../scripts/installer.sh canonical source installation
# upstream implementation ../scripts/install-managed-bundle.sh private managed runtime
# upstream implementation ../src/install.rs configuration and installation state
# upstream implementation ../src/interactive.rs native PTY execution
# downstream environment ../.github/workflows/ptymark-ci.yml required product acceptance
# @dependency-end

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
browser="${1:-${PTYMARK_TEST_BROWSER:-/usr/bin/chromium}}"

[[ "$(uname -s)" == Linux ]] || {
  echo 'fresh-source acceptance currently requires Linux' >&2
  exit 2
}
[[ -x "$browser" ]] || {
  printf 'Chromium-compatible browser is not executable: %s\n' "$browser" >&2
  exit 2
}
browser="$(cd "$(dirname "$browser")" && pwd -P)/$(basename "$browser")"

original_path="${PATH:-}"
original_home="${HOME:-}"
original_cargo_home="${CARGO_HOME:-}"
original_rustup_home="${RUSTUP_HOME:-}"
original_target_dir="${CARGO_TARGET_DIR:-}"
root="$(mktemp -d)"
raw="$root/raw"
fixtures="$root/fixtures"
artifacts="$root/artifacts"
external_evidence="${PTYMARK_FRESH_SOURCE_EVIDENCE_DIR:-}"
evidence="${external_evidence:-$root/evidence}"
current_stage=initialize
max_evidence_bytes=16384
mkdir -p "$raw" "$fixtures" "$artifacts" "$evidence"
chmod 700 "$root" "$raw" "$fixtures" "$artifacts" "$evidence"

sed_escape() {
  printf '%s' "$1" | sed 's/[\\&|]/\\&/g'
}

sanitize_file() {
  local input="$1"
  local output="$2"
  local staged="$evidence/.sanitize-$$"
  local root_re repo_re cargo_re rustup_re target_re home_re
  root_re="$(sed_escape "$root")"
  repo_re="$(sed_escape "$repo_root")"
  cargo_re="$(sed_escape "$original_cargo_home")"
  rustup_re="$(sed_escape "$original_rustup_home")"
  target_re="$(sed_escape "$original_target_dir")"
  home_re="$(sed_escape "$original_home")"

  local -a sed_args
  sed_args=(-e "s|$root_re|<fresh-root>|g" -e "s|$repo_re|<repository>|g")
  [[ -z "$original_cargo_home" ]] || sed_args+=(-e "s|$cargo_re|<cargo-home>|g")
  [[ -z "$original_rustup_home" ]] || sed_args+=(-e "s|$rustup_re|<rustup-home>|g")
  [[ -z "$original_target_dir" ]] || sed_args+=(-e "s|$target_re|<cargo-target>|g")
  [[ -z "$original_home" ]] || sed_args+=(-e "s|$home_re|<host-home>|g")

  sed "${sed_args[@]}" "$input" >"$staged"
  local size
  size="$(wc -c <"$staged")"
  if (( size <= max_evidence_bytes )); then
    mv "$staged" "$output"
  else
    {
      head -c $((max_evidence_bytes / 2)) "$staged"
      printf '\n... bounded evidence: %s bytes omitted ...\n' "$((size - max_evidence_bytes))"
      tail -c $((max_evidence_bytes / 2)) "$staged"
    } >"$output"
    rm -f "$staged"
  fi
  chmod 600 "$output"
}

collect_evidence() {
  local status="$1"
  {
    printf 'stage\t%s\n' "$current_stage"
    printf 'exit_status\t%s\n' "$status"
    printf 'root\t<fresh-root>\n'
    printf 'browser\t%s\n' "$browser"
  } >"$evidence/summary.tsv"
  chmod 600 "$evidence/summary.tsv"

  local path name
  for path in "$raw"/*; do
    [[ -f "$path" ]] || continue
    name="$(basename "$path")"
    sanitize_file "$path" "$evidence/$name"
  done
  for path in \
    "${config_path:-}" \
    "${state_path:-}" \
    "${bundle_root:-}/bundle.toml" \
    "${bundle_root:-}/bundle.stamp"; do
    [[ -n "$path" && -f "$path" ]] || continue
    name="$(basename "$path")"
    sanitize_file "$path" "$evidence/$name"
  done
}

finish() {
  local status="$1"
  trap - EXIT
  set +e
  if (( status != 0 )); then
    collect_evidence "$status"
    printf 'fresh-source acceptance failed during %s (exit %s)\n' "$current_stage" "$status" >&2
    local path
    for path in "$evidence"/*; do
      [[ -f "$path" ]] || continue
      printf '\n===== %s =====\n' "$(basename "$path")" >&2
      cat "$path" >&2
    done
    if [[ -n "$external_evidence" ]]; then
      printf '\nbounded redacted evidence retained at %s\n' "$external_evidence" >&2
    fi
  fi
  rm -rf "$root"
  if (( status == 0 )) && [[ -n "$external_evidence" ]]; then
    rm -rf "$external_evidence"
  fi
  exit "$status"
}
trap 'finish "$?"' EXIT

# Build a PATH containing the existing build/system tools but no globally
# installed Node, renderer, presenter, or Ptymark command. The managed installer
# may expose its selected private Node runtime only to npm's child process.
safe_bin="$root/path"
mkdir -p "$safe_bin"
IFS=':' read -r -a path_dirs <<<"$original_path"
for directory in "${path_dirs[@]}"; do
  [[ -n "$directory" && -d "$directory" ]] || continue
  for candidate in "$directory"/*; do
    [[ -x "$candidate" && ! -d "$candidate" ]] || continue
    name="$(basename "$candidate")"
    case "$name" in
      node|nodejs|npm|npx|corepack|pnpm|yarn|yarnpkg|ptymark|mmdc|tex2svg|chafa)
        continue
        ;;
    esac
    [[ -e "$safe_bin/$name" ]] || ln -s "$candidate" "$safe_bin/$name"
  done
done
export PATH="$safe_bin"
hash -r

for forbidden in node nodejs npm npx corepack ptymark mmdc tex2svg chafa; do
  unset -f "$forbidden" 2>/dev/null || true
  if command -v "$forbidden" >/dev/null 2>&1; then
    printf 'fresh PATH unexpectedly exposes %s\n' "$forbidden" >&2
    exit 1
  fi
done
for required in awk basename bash cargo cat chmod cmp cp curl cut dirname env git grep head install ln mkdir mktemp mv od pwd rm rustc sed sha256sum tail tar tr wc xz; do
  command -v "$required" >/dev/null 2>&1 || {
    printf 'required acceptance tool is unavailable: %s\n' "$required" >&2
    exit 1
  }
done

export HOME="$root/home"
export XDG_CONFIG_HOME="$root/xdg/config"
export XDG_DATA_HOME="$root/xdg/data"
export XDG_STATE_HOME="$root/xdg/state"
export XDG_CACHE_HOME="$root/xdg/cache"
export TMPDIR="$root/tmp"
mkdir -p "$HOME" "$XDG_CONFIG_HOME" "$XDG_DATA_HOME" "$XDG_STATE_HOME" "$XDG_CACHE_HOME" "$TMPDIR"
unset PTYMARK_CONFIG PTYMARK_INSTALL_STATE PTYMARK_DATA_HOME PTYMARK_CACHE_HOME PTYMARK_RENDERER_ROOT
unset NODE_PATH NODE_OPTIONS NPM_CONFIG_PREFIX NPM_CONFIG_USERCONFIG npm_config_userconfig
unset NPM_TOKEN NODE_AUTH_TOKEN

# shellcheck disable=SC1091
source "$repo_root/renderers/managed-bundle.env"
bundle_id="v${PTYMARK_MANAGED_BUNDLE_VERSION}-node${PTYMARK_MANAGED_NODE_VERSION}-mermaid${PTYMARK_MANAGED_MERMAID_VERSION}-mathjax${PTYMARK_MANAGED_MATHJAX_VERSION}"
install_root="$root/install"
bundle_root="$XDG_DATA_HOME/ptymark/renderer-bundles/$bundle_id"
config_path="$XDG_CONFIG_HOME/ptymark/config.toml"
state_path="$XDG_STATE_HOME/ptymark/install.toml"
installed_binary="$install_root/bin/ptymark"

current_stage=source-install
bash "$repo_root/scripts/installer.sh" \
  --root "$install_root" \
  --browser "$browser" \
  --skip-browser-download \
  >"$raw/installer.log" 2>&1

[[ -x "$installed_binary" ]] || {
  echo 'normal source installer did not create the expected Ptymark executable' >&2
  exit 1
}
export PATH="$install_root/bin:$safe_bin"
[[ "$(command -v ptymark)" == "$installed_binary" ]] || {
  echo 'post-install commands are not using the freshly installed Ptymark executable' >&2
  exit 1
}
for forbidden in node nodejs npm npx corepack; do
  if command -v "$forbidden" >/dev/null 2>&1; then
    printf 'managed installation leaked %s onto the test PATH\n' "$forbidden" >&2
    exit 1
  fi
done
[[ -r "$config_path" && -r "$state_path" ]] || {
  echo 'completed installation did not commit readable config and state files' >&2
  exit 1
}

current_stage=installed-state
ptymark --version >"$raw/version.txt" 2>"$raw/version.stderr"
ptymark install status >"$raw/install-status.txt" 2>"$raw/install-status.stderr"
ptymark config check >"$raw/config-check.txt" 2>"$raw/config-check.stderr"
ptymark config show >"$raw/config-show.toml" 2>"$raw/config-show.stderr"
ptymark engine check >"$raw/engine-check.txt" 2>"$raw/engine-check.stderr"
ptymark doctor >"$raw/doctor.txt" 2>"$raw/doctor.stderr"
ptymark doctor --json >"$raw/doctor.json" 2>"$raw/doctor-json.stderr"

cmp -s "$config_path" "$raw/config-show.toml" || {
  echo 'config show does not match the committed normalized user configuration' >&2
  exit 1
}
grep -F 'schema_version = 2' "$config_path" >/dev/null
test "$(grep -F -c 'provider = "managed"' "$config_path")" -eq 3
if grep -F 'program = ' "$config_path" >/dev/null; then
  echo 'portable user configuration unexpectedly contains resolved managed paths' >&2
  exit 1
fi
grep -F 'schema_version = 2' "$state_path" >/dev/null
grep -F "config_path = \"$config_path\"" "$state_path" >/dev/null
grep -E '^config_digest = "[0-9a-f]{64}"$' "$state_path" >/dev/null
test "$(grep -F -c 'origin = "managed"' "$state_path")" -eq 3
test "$(grep -F -c 'active = true' "$state_path")" -eq 3
for record in \
  $'mermaid\tmermaid-cli\tready' \
  $'math\tmathjax-cli\tready' \
  $'presenter\tchafa-symbols\tready'; do
  grep -F "$record" "$raw/install-status.txt" >/dev/null
done
for role_path in \
  "$bundle_root/bin/mmdc" \
  "$bundle_root/bin/tex2svg" \
  "$bundle_root/bin/chafa"; do
  [[ "$role_path" == "$bundle_root"/* && -x "$role_path" ]] || {
    printf 'managed role path is not executable inside the isolated bundle: %s\n' "$role_path" >&2
    exit 1
  }
  grep -F "$role_path" "$state_path" >/dev/null
  grep -F "$role_path" "$raw/engine-check.txt" >/dev/null
done
grep -F $'mermaid\tmermaid-cli' "$raw/engine-check.txt" >/dev/null
grep -F $'math\tmathjax-cli' "$raw/engine-check.txt" >/dev/null
grep -F $'presenter\tchafa-symbols' "$raw/engine-check.txt" >/dev/null
grep -F 'ptymark doctor: ready' "$raw/doctor.txt" >/dev/null
grep -F 'configuration: valid ' "$raw/doctor.txt" >/dev/null
grep -F 'installation: valid ' "$raw/doctor.txt" >/dev/null
grep -F 'engine mermaid: mermaid-cli (ready, origin=managed-bundle)' "$raw/doctor.txt" >/dev/null
grep -F 'engine math: mathjax-cli (ready, origin=managed-bundle)' "$raw/doctor.txt" >/dev/null
grep -F 'presenter: chafa-symbols (ready)' "$raw/doctor.txt" >/dev/null
grep -F '"schema": "ptymark.doctor.v1"' "$raw/doctor.json" >/dev/null
grep -F '"status": "ready"' "$raw/doctor.json" >/dev/null
test "$(grep -F -c '"origin": "managed-bundle"' "$raw/doctor.json")" -eq 2
grep -F '"public_safe_default": true' "$raw/doctor.json" >/dev/null
grep -F '"semantic_source": "excluded"' "$raw/doctor.json" >/dev/null
grep -F '"child_environment": "excluded"' "$raw/doctor.json" >/dev/null
grep -F '"renderer_stderr": "bounded-and-sanitized"' "$raw/doctor.json" >/dev/null

current_stage=direct-managed-engines
cat >"$fixtures/direct.mmd" <<'EOF_MERMAID'
flowchart LR
  FreshSource --> ManagedBundle --> Terminal
EOF_MERMAID
"$bundle_root/bin/mmdc" \
  --input "$fixtures/direct.mmd" \
  --output "$artifacts/direct-mermaid.svg" \
  >"$raw/direct-mermaid.txt" 2>"$raw/direct-mermaid.stderr"
test -s "$artifacts/direct-mermaid.svg"
grep -F '<svg' "$artifacts/direct-mermaid.svg" >/dev/null
"$bundle_root/bin/tex2svg" 'x_{fresh}^{2} + y_{fresh}^{2}' \
  >"$artifacts/direct-math.svg" 2>"$raw/direct-math.stderr"
test -s "$artifacts/direct-math.svg"
grep -F '<svg' "$artifacts/direct-math.svg" >/dev/null
"$bundle_root/bin/chafa" \
  --format symbols \
  --probe off \
  --polite on \
  --relative off \
  --animate off \
  --colors none \
  --size 48x \
  "$artifacts/direct-mermaid.svg" \
  >"$raw/direct-presenter.txt" 2>"$raw/direct-presenter.stderr"
test -s "$raw/direct-presenter.txt"

current_stage=strict-preview
cat >"$fixtures/preview.md" <<'EOF_PREVIEW'
preview-before
```mermaid
flowchart LR
  Preview --> Strict --> Managed
```
preview-between
$$
x_{preview}^{2} + y_{preview}^{2}
$$
preview-after
EOF_PREVIEW
ptymark preview --strict --columns 48 "$fixtures/preview.md" \
  >"$raw/preview.out" 2>"$raw/preview.stderr"
test -s "$raw/preview.out"
for source_text in '```mermaid' '$$' 'x_{preview}^{2} + y_{preview}^{2}'; do
  if grep -F "$source_text" "$raw/preview.out" >/dev/null; then
    printf 'strict preview retained semantic source: %s\n' "$source_text" >&2
    exit 1
  fi
done
preview_before="$(grep -n -m1 '^preview-before$' "$raw/preview.out" | cut -d: -f1)"
preview_between="$(grep -n -m1 '^preview-between$' "$raw/preview.out" | cut -d: -f1)"
preview_after="$(grep -n -m1 '^preview-after$' "$raw/preview.out" | cut -d: -f1)"
[[ -n "$preview_before" && -n "$preview_between" && -n "$preview_after" ]]
(( preview_before < preview_between && preview_between < preview_after ))
(( preview_between - preview_before > 1 && preview_after - preview_between > 1 ))

current_stage=native-pty
pty_script=$(cat <<'EOF_PTY'
printf 'pty-before\nactive=%s\n```mermaid\nflowchart LR\n  Native --> PTY --> Managed\n```\npty-between\n$$\nx_{pty}^{2} + y_{pty}^{2}\n$$\npty-after\n' "${PTYMARK_ACTIVE:-missing}"
exit 23
EOF_PTY
)
set +e
ptymark -- /bin/sh -c "$pty_script" >"$raw/native-pty.out" 2>"$raw/native-pty.stderr"
pty_status="$?"
set -e
printf '%s\n' "$pty_status" >"$raw/native-pty.status"
[[ "$pty_status" -eq 23 ]] || {
  printf 'native PTY did not preserve child exit status 23 (got %s)\n' "$pty_status" >&2
  exit 1
}
test -s "$raw/native-pty.out"
for source_text in '```mermaid' '$$' 'x_{pty}^{2} + y_{pty}^{2}'; do
  if grep -F "$source_text" "$raw/native-pty.out" >/dev/null; then
    printf 'native PTY retained semantic source: %s\n' "$source_text" >&2
    exit 1
  fi
done
tr -d '\r' <"$raw/native-pty.out" >"$raw/native-pty.lines"
pty_before="$(grep -n -m1 'pty-before' "$raw/native-pty.lines" | cut -d: -f1)"
pty_active="$(grep -n -m1 'active=1' "$raw/native-pty.lines" | cut -d: -f1)"
pty_between="$(grep -n -m1 'pty-between' "$raw/native-pty.lines" | cut -d: -f1)"
pty_after="$(grep -n -m1 'pty-after' "$raw/native-pty.lines" | cut -d: -f1)"
[[ -n "$pty_before" && -n "$pty_active" && -n "$pty_between" && -n "$pty_after" ]]
(( pty_before < pty_active && pty_active < pty_between && pty_between < pty_after ))
(( pty_between - pty_active > 1 && pty_after - pty_between > 1 ))
# Native child lines and successful rendered rows must all use terminal-safe
# CRLF boundaries. Reject every LF byte that is not immediately preceded by CR.
od -An -v -t u1 "$raw/native-pty.out" | awk '
  {
    for (field = 1; field <= NF; field++) {
      byte = $field + 0
      if (byte == 10 && previous != 13) bad = 1
      previous = byte
    }
  }
  END { exit bad }
'

current_stage=complete
printf 'ptymark fresh-source installation acceptance: ok\n'
