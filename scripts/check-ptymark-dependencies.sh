#!/usr/bin/env bash
# @dependency-start
# contract test
# responsibility Verifies the canonical ptymark compiler and existing-renderer dependency graph.
# upstream environment ../docker/ptymark-versions.env declares expected versions.
# upstream environment ../renderers/package-lock.json declares exact JavaScript resolution.
# downstream workflow ../.github/workflows/ptymark-ci.yml runs the check in Docker.
# @dependency-end
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

# shellcheck disable=SC1091
source docker/ptymark-versions.env
export MERMAID_CLI_VERSION MERMAID_VERSION MATHJAX_VERSION KATEX_VERSION PUPPETEER_VERSION

fail() {
  printf 'ptymark dependency check: %s\n' "$*" >&2
  exit 1
}

node_version="${NODE_IMAGE#node:}"
node_version="${node_version%-bookworm}"
renderer_root="${PTYMARK_RENDERER_ROOT:-/opt/ptymark-renderers}"

rustc --version | grep -F "rustc ${RUST_VERSION} " >/dev/null
cargo --version >/dev/null
node --version | grep -Fx "v${node_version}" >/dev/null
mmdc --version | grep -F "${MERMAID_CLI_VERSION}" >/dev/null
katex --version | grep -F "${KATEX_VERSION}" >/dev/null
typst --version | grep -F "${TYPST_VERSION}" >/dev/null
lua5.4 -v >/dev/null
chromium --version >/dev/null

node --input-type=module <<'NODE'
import fs from 'node:fs';
const expected = {
  '@mermaid-js/mermaid-cli': process.env.MERMAID_CLI_VERSION,
  mermaid: process.env.MERMAID_VERSION,
  '@mathjax/src': process.env.MATHJAX_VERSION,
  '@mathjax/mathjax-newcm-font': process.env.MATHJAX_VERSION,
  katex: process.env.KATEX_VERSION,
  puppeteer: process.env.PUPPETEER_VERSION,
};
const packageJson = JSON.parse(fs.readFileSync('renderers/package.json', 'utf8'));
const lock = JSON.parse(fs.readFileSync('renderers/package-lock.json', 'utf8'));
for (const [name, version] of Object.entries(expected)) {
  if (packageJson.dependencies[name] !== version) {
    throw new Error(`${name} package.json mismatch: ${packageJson.dependencies[name]} != ${version}`);
  }
  const resolved = lock.packages[`node_modules/${name}`]?.version;
  if (resolved !== version) {
    throw new Error(`${name} lock mismatch: ${resolved} != ${version}`);
  }
}
NODE

npm list --prefix "$renderer_root" --depth=0 \
  "@mermaid-js/mermaid-cli@${MERMAID_CLI_VERSION}" \
  "mermaid@${MERMAID_VERSION}" \
  "@mathjax/src@${MATHJAX_VERSION}" \
  "@mathjax/mathjax-newcm-font@${MATHJAX_VERSION}" \
  "katex@${KATEX_VERSION}" \
  "puppeteer@${PUPPETEER_VERSION}" >/dev/null

grep -F "rust-version = \"${RUST_VERSION}\"" Cargo.toml >/dev/null
grep -F "channel = \"${RUST_VERSION}\"" rust-toolchain.toml >/dev/null
grep -F "ARG NODE_IMAGE=${NODE_IMAGE}" docker/ptymark.Dockerfile >/dev/null
grep -F "ARG RUST_VERSION=${RUST_VERSION}" docker/ptymark.Dockerfile >/dev/null
grep -F "ARG TYPST_VERSION=${TYPST_VERSION}" docker/ptymark.Dockerfile >/dev/null

grep -F "NODE_IMAGE: \${NODE_IMAGE:-${NODE_IMAGE}}" docker/ptymark-compose.yaml >/dev/null || \
  fail "Compose Node fallback does not match docker/ptymark-versions.env"
grep -F "RUST_VERSION: \${RUST_VERSION:-${RUST_VERSION}}" docker/ptymark-compose.yaml >/dev/null || \
  fail "Compose Rust fallback does not match docker/ptymark-versions.env"
grep -F "TYPST_VERSION: \${TYPST_VERSION:-${TYPST_VERSION}}" docker/ptymark-compose.yaml >/dev/null || \
  fail "Compose Typst fallback does not match docker/ptymark-versions.env"

printf 'ptymark dependency check: ok\n'
