#!/usr/bin/env bash
# Canonical ptymark installer for POSIX shells and Windows Bash environments.
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
repo_root="$(cd "$script_dir/.." && pwd -P)"
host_os="$(uname -s)"

usage() {
  cat <<'EOF_USAGE'
Usage: bash scripts/installer.sh [OPTIONS]

Install ptymark, resolve renderer paths once, and install the isolated default
renderer bundle only for missing roles. The script supports Linux, macOS, WSL,
Git Bash, MSYS2, and Cygwin.

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
  --skip-browser-download   require an existing browser
  --offline                 perform no downloads or npm install
  --force-managed           reinstall the managed app bundle
  --reprobe                 prefer system commands again, then managed fallbacks
  --reset                   discard existing configuration before resolving
  --dry-run                 print the resolver plan; do not write config/state
  -h, --help                show this help

Windows Bash delegates to scripts/installer.ps1 after converting POSIX paths to
native Windows paths. WSL is treated as Linux and installs Linux-native tools.
EOF_USAGE
}

need_value() {
  [[ -n "${2:-}" ]] || { printf 'missing value after %s\n' "$1" >&2; exit 2; }
}

is_windows_bash=0
case "$host_os" in
  MINGW*|MSYS*|CYGWIN*) is_windows_bash=1 ;;
esac

if [[ "$is_windows_bash" -eq 1 ]]; then
  command -v cygpath >/dev/null 2>&1 || {
    echo 'cygpath is required for Git Bash, MSYS2, or Cygwin installation' >&2
    exit 1
  }

  powershell=""
  for candidate in pwsh.exe pwsh powershell.exe powershell; do
    if command -v "$candidate" >/dev/null 2>&1; then
      powershell="$(command -v "$candidate")"
      break
    fi
  done
  [[ -n "$powershell" ]] || {
    echo 'PowerShell is required for Windows-native installation' >&2
    exit 1
  }

  to_windows_path() {
    cygpath -aw "$1"
  }

  to_windows_program() {
    local value="$1"
    case "$value" in
      auto|keep|preview|source) printf '%s\n' "$value" ;;
      *[\\/]*|.*)
        printf '%s\n' "$(to_windows_path "$value")"
        ;;
      *)
        if [[ -e "$value" ]]; then
          printf '%s\n' "$(to_windows_path "$value")"
        else
          printf '%s\n' "$value"
        fi
        ;;
    esac
  }

  ps_args=()
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --root) need_value "$1" "${2:-}"; ps_args+=(-Root "$(to_windows_path "$2")"); shift 2 ;;
      --binary) need_value "$1" "${2:-}"; ps_args+=(-Binary "$(to_windows_path "$2")"); shift 2 ;;
      --skip-core) ps_args+=(-SkipCore); shift ;;
      --config) need_value "$1" "${2:-}"; ps_args+=(-Config "$(to_windows_path "$2")"); shift 2 ;;
      --state) need_value "$1" "${2:-}"; ps_args+=(-State "$(to_windows_path "$2")"); shift 2 ;;
      --mermaid) need_value "$1" "${2:-}"; ps_args+=(-Mermaid "$(to_windows_program "$2")"); shift 2 ;;
      --math) need_value "$1" "${2:-}"; ps_args+=(-Math "$(to_windows_program "$2")"); shift 2 ;;
      --presenter) need_value "$1" "${2:-}"; ps_args+=(-Presenter "$(to_windows_program "$2")"); shift 2 ;;
      --managed) need_value "$1" "${2:-}"; ps_args+=(-Managed "$2"); shift 2 ;;
      --managed-root) need_value "$1" "${2:-}"; ps_args+=(-ManagedRoot "$(to_windows_path "$2")"); shift 2 ;;
      --browser) need_value "$1" "${2:-}"; ps_args+=(-Browser "$(to_windows_path "$2")"); shift 2 ;;
      --skip-browser-download) ps_args+=(-SkipBrowserDownload); shift ;;
      --offline) ps_args+=(-Offline); shift ;;
      --force-managed) ps_args+=(-ForceManaged); shift ;;
      --reprobe) ps_args+=(-Reprobe); shift ;;
      --reset) ps_args+=(-Reset); shift ;;
      --dry-run) ps_args+=(-DryRun); shift ;;
      -h|--help) usage; exit 0 ;;
      *) printf 'unknown installer option: %s\n' "$1" >&2; usage >&2; exit 2 ;;
    esac
  done

  ps_script="$(to_windows_path "$script_dir/installer.ps1")"
  MSYS2_ARG_CONV_EXCL='*' "$powershell" \
    -NoProfile -ExecutionPolicy Bypass -File "$ps_script" "${ps_args[@]}"
  exit $?
fi

case "$host_os" in
  Darwin|Linux) ;;
  *) printf 'unsupported Bash installer host: %s\n' "$host_os" >&2; exit 1 ;;
esac

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

case "$host_os" in
  Darwin) default_data_root="${HOME:?HOME is required}/Library/Application Support/ptymark" ;;
  Linux) default_data_root="${XDG_DATA_HOME:-${HOME:?HOME is required}/.local/share}/ptymark" ;;
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

find_browser() {
  local candidate
  for candidate in chromium chromium-browser google-chrome google-chrome-stable microsoft-edge microsoft-edge-stable; do
    if command -v "$candidate" >/dev/null 2>&1; then
      absolute_command "$candidate"
      return 0
    fi
  done
  if [[ "$host_os" == Darwin ]]; then
    for candidate in \
      '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome' \
      '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge' \
      '/Applications/Chromium.app/Contents/MacOS/Chromium'; do
      if [[ -x "$candidate" ]]; then printf '%s\n' "$candidate"; return 0; fi
    done
  fi
  return 1
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
    if [[ -z "$browser_path" ]]; then browser_path="$(find_browser || true)"; fi
    if [[ -n "$browser_path" ]]; then skip_browser_download=1; fi
    bundle_args=(--root "$managed_root" --launcher "$binary")
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
