# ptymark post-merge roadmap

This roadmap orders the work remaining after the reviewed portable core was merged in PR #30.
GitHub issues remain the detailed acceptance-criteria source; this file owns delivery order and
cross-issue dependencies.

## P0 — merge health and immediately useful execution

1. Restore `vendor/agent-canon` to the submodule mode declared by `.gitmodules` (#24 / PR #32).
2. Add non-interactive child-stdout filtering through `ptymark run -- COMMAND` (PR #33).
3. Keep `ptymark -- COMMAND` transparent until an actual PTY/ConPTY host owns input, resize, and
   signal semantics.

## P1 — interactive runtime and operational visibility

1. Implement the Unix PTY host and byte-exact live terminal regression suite (#3).
2. Add immutable configuration snapshots and restart-required change reporting (#20).
3. Add diagnostics, timing, cache/fallback counters, and source-redaction controls (#13).
4. Add Windows ConPTY parity after the Unix lifecycle contract is stable (#3).

## P2 — configuration lifecycle

1. Config discovery, precedence, provenance, and project trust (#6).
2. Named profiles, inheritance, and explicit session overrides (#7).
3. Context-aware pre-launch profile routing (#21).
4. Path/environment/secret references without shell evaluation (#22).
5. Introspection, editor schema, and deliberate migrations (#15).
6. Complete the typed detector policy and stricter-only safety controls (#8).

## P3 — production renderer and presentation evolution

1. Concrete engine selection and trusted adapter contracts (#9, #25).
2. Dependency doctor, version/protocol compatibility, and explicit bundle management (#16).
3. Persistent workers and measured cold/warm latency budgets (#4, #11, #25).
4. Capability-aware text/image presentation with lossless fallback (#10).
5. Opt-in bounded persistent/tiered cache (#12).
6. WezTerm per-session profile/metric bridge (#14).
7. End-to-end configuration and acceptance examples (#18).

## Delivery rules

- Every PR keeps terminal-control bytes outside semantic rendering.
- Interactive ownership is not claimed until PTY/ConPTY, resize, signals, and exit status are tested
  together.
- New engine or handoff abstractions require a measured user case; no speculative generic registry.
- Normal rendering performs no dependency installation or network access.
- Each required verification item must pass for the current PR head before merge.
