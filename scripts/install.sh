#!/usr/bin/env bash
# Install ptymark and resolve a complete renderer pipeline with user-local fallbacks.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
# shellcheck disable=SC1091
source "$repo_root/renderers/managed-bundle.env"

install_root=""
binary=""
skip_core=0
dry_run=0
reset=0
reprobe=0
config_path=""
state_path=""
managed_mode=auto
managed_root=""
browser_path=""
skip_browser_download=0
offline=0
force_managed=0
mermaid_value=""
math_value=""
presenter_value=""

usage() {
  cat <<'EOF'
Usage: bash scripts/install.sh [OPTIONS]

Install the ptymark core, prefer compatible user-installed renderer commands,
and provision the pinned default bundle in a versioned user-data directory for
any missing slot. No global npm package or PATH entry is created.

Options:
  --root DIR                pass DIR to `cargo install --root`
  --binary PATH             use PATH as the installed ptymark binary
  --skip-core               do not run `cargo install` (requires --binary)
  --config PATH             write/update this ptymark configuration
  --state PATH              write the installation snapshot here
  --mermaid VALUE           auto | keep | preview | source | EXECUTABLE
  --math VALUE              auto | keep | preview | source | EXECUTABLE
  --presenter VALUE         auto | keep | EXECUTABLE
  --managed MODE            auto (default) | always | never
  --managed-root DIR        install/use the isolated renderer bundle in DIR
  --browser PATH            use an existing Chromium-compatible browser
  --skip-browser-download   do not let Puppeteer download a private browser
  --offline                 use existing core/bundle files; perform no downloads
  --force-managed           reinstall the managed app bundle
  --reprobe                 prefer system commands again, then managed fallbacks
  --reset                   discard existing configuration before resolving
  --dry-run                 print the resolver plan; do not write config/state
  -h, --help                show this help

Default engines:
  Mermaid  @mermaid-js/mermaid-cli 11.16.0
  Math     MathJax 4.1.3
  Display  ptymark browser-backed ANSI presenter
EOF
}

need_value() {
  [[ -n "${2:-}" ]] || { printf 'missing value after %s\n' "$1" >&2; exit 2; }
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --root) need_value "$1" "${2:-}"; install_root="$2"; shift 2 ;;
    --binary) need_value "$1" "${2:-}"; binary="$2"; shift 2 ;;
    --skip-core) skip_core=1; shift ;;
    --config) need_value "$1" "${2:-}"; config_path="$2"; shift 2 ;;
    --state) need_value "$1" "${2:-}"; state_path="$2"; shift 2 ;;
    --mermaid) need_value "$1" "${2:-}"; mermaid_value="$2"; shift 2 ;;
    --math) need_value "$1" "${2:-}"; math_value="$2"; shift 2 ;;
    --presenter) need_value "$1" "${2:-}"; presenter_value="$2"; shift 2 ;;
    --managed)
      need_value "$1" "${2:-}"
      case "$2" in auto|always|never) managed_mode="$2" ;; *) echo '--managed must be auto, always, or never' >&2; exit 2 ;; esac
      shift 2
      ;;
    --managed-root) need_value "$1" "${2:-}"; managed_root="$2"; shift 2 ;;
    --browser) need_value "$1" "${2:-}"; browser_path="$2"; shift 2 ;;
    --skip-browser-download) skip_browser_download=1; shift ;;
    --offline) offline=1; shift ;;
    --force-managed) force_managed=1; shift ;;
    --reprobe) reprobe=1; shift ;;
    --reset) reset=1; shift ;;
    --dry-run) dry_run=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) printf 'unknown installer option: %s\n' "$1" >&2; usage >&2; exit 2 ;;
  esac
done

if [[ "$skip_core" -eq 0 ]]; then
  command -v cargo >/dev/null 2>&1 || { echo 'cargo is required to install the ptymark core binary' >&2; exit 1; }
  cargo_args=(install --locked --force --path "$repo_root")
  [[ -z "$install_root" ]] || cargo_args+=(--root "$install_root")
  cargo "${cargo_args[@]}"
fi

if [[ -z "$binary" ]]; then
  if [[ -n "$install_root" ]]; then binary="$install_root/bin/ptymark"
  elif [[ -n "${CARGO_INSTALL_ROOT:-}" ]]; then binary="$CARGO_INSTALL_ROOT/bin/ptymark"
  elif [[ -n "${CARGO_HOME:-}" ]]; then binary="$CARGO_HOME/bin/ptymark"
  else binary="${HOME:?HOME is required when --binary and --root are omitted}/.cargo/bin/ptymark"
  fi
fi
[[ -x "$binary" ]] || { printf 'installed ptymark binary was not found or is not executable: %s\n' "$binary" >&2; exit 1; }
binary="$(cd "$(dirname "$binary")" && pwd -P)/$(basename "$binary")"

if [[ -z "$config_path" ]]; then
  config_path="${PTYMARK_CONFIG:-${XDG_CONFIG_HOME:-${HOME:?HOME is required}/.config}/ptymark/config.toml}"
fi

host_os="$(uname -s)"
case "$host_os" in
  Darwin) default_data_root="${HOME:?HOME is required}/Library/Application Support/ptymark" ;;
  Linux) default_data_root="${XDG_DATA_HOME:-${HOME:?HOME is required}/.local/share}/ptymark" ;;
  *) echo "unsupported installer host: $host_os" >&2; exit 1 ;;
esac
bundle_id="v${PTYMARK_MANAGED_BUNDLE_VERSION}-node${PTYMARK_MANAGED_NODE_VERSION}-mermaid${PTYMARK_MANAGED_MERMAID_VERSION}-mathjax${PTYMARK_MANAGED_MATHJAX_VERSION}"
managed_root="${managed_root:-$default_data_root/renderer-bundles/$bundle_id}"
managed_bin="$managed_root/bin"
managed_mermaid="$managed_bin/mmdc"
managed_math="$managed_bin/tex2svg"
managed_presenter="$managed_bin/chafa"

absolute_command() {
  local command_name="$1"
  local found
  found="$(command -v "$command_name" 2>/dev/null || true)"
  [[ -n "$found" ]] || return 1
  (cd "$(dirname "$found")" && printf '%s/%s\n' "$(pwd -P)" "$(basename "$found")")
}

is_builtin_choice() {
  case "$1" in preview|source) return 0 ;; *) return 1 ;; esac
}

config_exists=0
[[ -f "$config_path" ]] && config_exists=1
resolve_defaults=0
if [[ "$config_exists" -eq 0 || "$reset" -eq 1 || "$reprobe" -eq 1 \
  || -n "$mermaid_value" || -n "$math_value" || -n "$presenter_value" ]]; then
  resolve_defaults=1
fi

if [[ "$resolve_defaults" -eq 1 ]]; then
  system_mermaid="$(absolute_command mmdc || true)"
  system_math="$(absolute_command tex2svg || true)"
  system_presenter="$(absolute_command chafa || true)"

  for variable in mermaid_value math_value presenter_value; do
    value="${!variable}"
    [[ "$value" != auto ]] || printf -v "$variable" '%s' ''
  done

  need_managed=0
  if [[ "$managed_mode" == always ]]; then
    need_managed=1
  elif [[ "$managed_mode" == auto ]]; then
    [[ -n "$mermaid_value" || -n "$system_mermaid" ]] || need_managed=1
    [[ -n "$math_value" || -n "$system_math" ]] || need_managed=1
    [[ -n "$presenter_value" || -n "$system_presenter" ]] || need_managed=1
  fi

  managed_ready=0
  if [[ -x "$managed_mermaid" && -x "$managed_math" && -x "$managed_presenter" ]]; then managed_ready=1; fi
  if [[ "$need_managed" -eq 1 && "$managed_ready" -eq 0 && "$dry_run" -eq 0 ]]; then
    bundle_args=(--root "$managed_root")
    [[ -z "$browser_path" ]] || bundle_args+=(--browser "$browser_path")
    [[ "$skip_browser_download" -eq 0 ]] || bundle_args+=(--skip-browser-download)
    [[ "$offline" -eq 0 ]] || bundle_args+=(--offline)
    [[ "$force_managed" -eq 0 ]] || bundle_args+=(--force)
    bash "$repo_root/scripts/install-managed-bundle.sh" "${bundle_args[@]}"
    managed_ready=1
  fi

  if [[ -z "$mermaid_value" ]]; then
    if [[ "$managed_mode" == always && "$managed_ready" -eq 1 ]]; then mermaid_value="$managed_mermaid"
    elif [[ -n "$system_mermaid" ]]; then mermaid_value="$system_mermaid"
    elif [[ "$managed_ready" -eq 1 ]]; then mermaid_value="$managed_mermaid"
    else mermaid_value=preview
    fi
  fi
  if [[ -z "$math_value" ]]; then
    if [[ "$managed_mode" == always && "$managed_ready" -eq 1 ]]; then math_value="$managed_math"
    elif [[ -n "$system_math" ]]; then math_value="$system_math"
    elif [[ "$managed_ready" -eq 1 ]]; then math_value="$managed_math"
    else math_value=preview
    fi
  fi

  external_selected=0
  is_builtin_choice "$mermaid_value" || [[ "$mermaid_value" == keep ]] || external_selected=1
  is_builtin_choice "$math_value" || [[ "$math_value" == keep ]] || external_selected=1
  if [[ -z "$presenter_value" && "$external_selected" -eq 1 ]]; then
    if [[ "$managed_mode" == always && "$managed_ready" -eq 1 ]]; then presenter_value="$managed_presenter"
    elif [[ -n "$system_presenter" ]]; then presenter_value="$system_presenter"
    elif [[ "$managed_ready" -eq 1 ]]; then presenter_value="$managed_presenter"
    else
      echo 'no terminal presenter is available; select preview/source or allow the managed bundle' >&2
      exit 1
    fi
  fi
fi

resolve_args=(install resolve --config "$config_path")
[[ -z "$state_path" ]] || resolve_args+=(--state "$state_path")
[[ -z "$mermaid_value" ]] || resolve_args+=(--mermaid "$mermaid_value")
[[ -z "$math_value" ]] || resolve_args+=(--math "$math_value")
[[ -z "$presenter_value" ]] || resolve_args+=(--presenter "$presenter_value")
[[ "$reset" -eq 0 ]] || resolve_args+=(--reset)
[[ "$dry_run" -eq 0 ]] || resolve_args+=(--dry-run)
"$binary" "${resolve_args[@]}"

if [[ "$dry_run" -eq 0 ]]; then
  status_args=(install status)
  [[ -z "$state_path" ]] || status_args+=(--state "$state_path")
  "$binary" "${status_args[@]}"
fi
