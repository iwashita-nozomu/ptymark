#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

# shellcheck disable=SC1091
source docker/ptymark-versions.env

fail() {
  printf 'ptymark dependency check: %s\n' "$*" >&2
  exit 1
}

rustc --version | grep -F "rustc ${RUST_VERSION} " >/dev/null 
cargo --version >/dev/null
node --version | grep -E '^v22\.' >/dev/null
mmdc --version | grep -F "${MERMAID_CLI_VERSION}" >/dev/null
typst --version | grep -F "${TYPST_VERSION}" >/dev/null
lua5.4 -v >/dev/null
chromium --version >/dev/null

npm list --global --depth=0 "@mermaid-js/mermaid-cli@${MERMAID_CLI_VERSION}" >/dev/null

grep -F "rust-version = \"${RUST_VERSION}\"" Cargo.toml >/dev/null 
grep -F "channel = \"${RUST_VERSION}\"" rust-toolchain.toml >/dev/null 
grep -F "ARG RUST_VERSION=${RUST_VERSION}" docker/ptymark.Dockerfile >/dev/null 
grep -F "ARG MERMAID_CLI_VERSION=${MERMAID_CLI_VERSION}" docker/ptymark.Dockerfile >/dev/null 
grep -F "ARG TYPST_VERSION=${TYPST_VERSION}" docker/ptymark.Dockerfile >/dev/null 

grep -F "RUST_VERSION: \${RUST_VERSION:-${RUST_VERSION}}" docker/ptymark-compose.yaml >/dev/null || \
  fail "Compose Rust fallback does not match docker/ptymark-versions.env"

grep -F "MERMAID_CLI_VERSION: \${MERMAID_CLI_VERSION:-${MERMAID_CLI_VERSION}}" docker/ptymark-compose.yaml >/dev/null || \
  fail "Compose Mermaid fallback does not match docker/ptymark-versions.env"

grep -F "TYPST_VERSION: \${TYPST_VERSION:-${TYPST_VERSION}}" docker/ptymark-compose.yaml >/dev/null || \
  fail "Compose Typst fallback does not match docker/ptymark-versions.env"

printf 'ptymark dependency check: ok\n'
