#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

compose=(
  docker compose
  --env-file docker/ptymark-versions.env
  --file docker/ptymark-compose.yaml
)

if [[ $# -eq 0 ]]; then
  exec "${compose[@]}" run --rm dev bash
fi

run_args=(run --rm)
if [[ ! -t 0 || ! -t 1 ]]; then
  run_args+=(--no-TTY)
fi

exec "${compose[@]}" "${run_args[@]}" dev "$@"
