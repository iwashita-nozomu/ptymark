#!/usr/bin/env python3
# @dependency-start
# contract test
# responsibility Validates interactive renderer and coordinator latency budgets in CI.
# upstream implementation ../renderers/benchmark.mjs measures existing engine latency.
# upstream implementation ../examples/benchmark_core.rs measures cache-hit orchestration latency.
# downstream workflow ../.github/workflows/ptymark-ci.yml publishes benchmark evidence.
# @dependency-end

from __future__ import annotations

import json
import os
import sys
from pathlib import Path


def read_json(path: str) -> dict[str, object]:
    return json.loads(Path(path).read_text(encoding="utf-8"))


def nested_number(data: dict[str, object], *keys: str) -> float:
    current: object = data
    for key in keys:
        if not isinstance(current, dict) or key not in current:
            raise KeyError(".".join(keys))
        current = current[key]
    if not isinstance(current, (int, float)):
        raise TypeError(".".join(keys))
    return float(current)


def main() -> int:
    if len(sys.argv) != 3:
        print("usage: check-ptymark-benchmarks.py RENDERER_JSON CORE_JSON", file=sys.stderr)
        return 2

    renderer = read_json(sys.argv[1])
    core = read_json(sys.argv[2])
    limits = {
        "math persistent p95 ms": float(os.getenv("PTYMARK_MATH_P95_MS", "500")),
        "mermaid persistent p95 ms": float(os.getenv("PTYMARK_MERMAID_P95_MS", "2000")),
        "core cache-hit p95 ns": float(os.getenv("PTYMARK_CACHE_HIT_P95_NS", "2000000")),
    }
    observed = {
        "math persistent p95 ms": nested_number(renderer, "engines", "math", "persistent", "p95Ms"),
        "mermaid persistent p95 ms": nested_number(
            renderer, "engines", "mermaid", "persistent", "p95Ms"
        ),
        "core cache-hit p95 ns": nested_number(core, "cache_hit_ns", "p95"),
    }

    failures: list[str] = []
    for name, value in observed.items():
        limit = limits[name]
        print(f"{name}: observed={value:.3f} limit={limit:.3f}")
        if value > limit:
            failures.append(f"{name} exceeded budget: {value:.3f} > {limit:.3f}")

    if failures:
        print("PTYMARK_BENCHMARK_BUDGET=fail", file=sys.stderr)
        for failure in failures:
            print(f"- {failure}", file=sys.stderr)
        return 1
    print("PTYMARK_BENCHMARK_BUDGET=pass")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
