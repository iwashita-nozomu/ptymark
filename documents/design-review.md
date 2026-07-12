# ptymark Design Review

<!--
@dependency-start
contract review
responsibility Records architecture review findings, resolutions, acceptance gates, and residual risks for the initial ptymark implementation.
upstream design ./system-design.md defines the reviewed architecture
upstream implementation ../src/runtime.rs implements the composition root
downstream workflow ../.github/workflows/ptymark-ci.yml verifies review acceptance gates
@dependency-end
-->

## 1. Review scope

This review evaluates the initial `ptymark` pull request against four goals:

1. terminal behavior must remain correct and lossless;
2. configuration must be resolved before terminal mutation;
3. renderer/cache/presenter implementations must remain independently replaceable;
4. the design must accommodate a real PTY host, persistent workers, image protocols, and persistent caches without rewriting the stream contract.

Reviewed surfaces:

```text
terminal output safety gate
semantic detector
pre-display commit path
engine registry/selector/coordinator
process execution
artifact cache
artifact presentation
configuration and profile resolution
WezTerm bridge
Docker/CI and performance evidence
user and developer documentation
```

## 2. Review method

The review proceeded from abstract responsibilities to implementation details.

```text
product invariants
  → four-plane architecture
  → session lifecycle
  → data contracts
  → provider/factory boundaries
  → component state and failure behavior
  → tests and CI evidence
```

Each finding is classified as:

- **Blocker** — merge would violate a product invariant or security boundary;
- **Major** — architecture would make a required next-stage feature expensive or unsafe;
- **Moderate** — behavior is correct but difficult to diagnose, maintain, or extend;
- **Minor** — documentation, naming, or ergonomics issue.

## 3. Findings and resolutions

### DR-01 — Runtime composition was hard-coded in the CLI

**Severity:** Major

**Observed:** `ResolvedConfig` contained engine, cache, and presentation policies, but `preview` constructed a fixed preview renderer directly. External engines and provider selection were therefore configuration-shaped but not runtime-owned.

**Risk:** Every new engine/cache/presenter would require editing CLI code. Command-mode PTY integration would duplicate construction logic.

**Resolution:** Introduce `RuntimeBuilder` as the sole composition root. Add detector, engine, cache, and presenter provider traits. CLI and future PTY host both request a runtime from the same builder.

**Acceptance:** Runtime contract tests prove that a custom provider can register an engine without modifying CLI, coordinator, detector, or presenter code.

### DR-02 — Built-in engine IDs were inconsistent with configuration IDs

**Severity:** Major

**Observed:** configuration used `preview` and `source`, while implementation descriptors used `builtin/preview` and `builtin/source`.

**Risk:** valid configuration could select an engine that the registry did not contain; cache identity and diagnostics would be misleading.

**Resolution:** Use stable public IDs `preview` and `source`. Namespace third-party engines, but do not add an undocumented prefix to built-ins.

**Acceptance:** tests compare configured candidate IDs to registered descriptor IDs.

### DR-03 — Configured process engines inherited the entire host environment

**Severity:** Blocker

**Observed:** the low-level external renderer created `Command` with the process environment inherited by default, while configuration exposed an explicit inheritance allowlist.

**Risk:** secrets, proxy credentials, cloud tokens, and project-specific state could enter renderer processes unintentionally.

**Resolution:** Add a strict `ProcessEngine` used by runtime composition. It clears the environment, restores only explicitly allowed variables, applies explicit configured values, and then adds the versioned ptymark protocol variables.

**Acceptance:** process tests verify that an unrelated secret environment variable is absent and an allowlisted variable is present.

### DR-04 — Process engine descriptor ignored configured kinds and limits

**Severity:** Major

**Observed:** the legacy external renderer advertised both built-in semantic kinds and used a fixed stderr limit. Configured semantic kinds, stderr limit, working directory, and environment policy were not connected.

**Risk:** an engine could be called for unsupported input; configuration would appear effective while being ignored.

**Resolution:** `ProcessEngineConfig` carries supported kinds, formats, layout, execution model, stdout/stderr bounds, timeout, working directory, explicit environment, and inheritance allowlist. Runtime conversion validates all fields before launch.

**Acceptance:** tests cover kind mismatch, timeout, stdout overflow, stderr truncation, working directory, and environment handling.

### DR-05 — Session configuration was mutable after resolution

**Severity:** Major

**Observed:** `LoadedConfig` exposed a mutable resolved structure and private mode mutated it before use. No explicit generation or snapshot identity existed.

**Risk:** future hot reload or shared services could observe partial policy changes; cache identity could diverge from active behavior.

**Resolution:** apply overrides before freezing and create `ConfigSnapshot` with generation, stable fingerprint, `Arc<ResolvedConfig>`, and `Arc<ConfigProvenance>`.

**Acceptance:** snapshot tests prove clone identity, generation change behavior, and fingerprint stability for equal policy.

### DR-06 — Presenter/cache/detector replacement lacked a composition contract

**Severity:** Major

**Observed:** traits existed, but there was no single place to choose implementations or report what had been selected.

**Risk:** future image presenters, disk cache, and document detectors would each add special cases.

**Resolution:** provider traits and `RuntimeBuildReport` become the extension contract. Duplicate provider IDs are errors. Unsupported configured backends fail before stream processing.

**Acceptance:** custom provider tests and duplicate-ID tests.

### DR-07 — Artifact and semantic enums did not communicate forward compatibility

**Severity:** Moderate

**Observed:** downstream exhaustive matches could become source incompatible when formats/kinds expand.

**Resolution:** mark public extension-sensitive enums non-exhaustive where practical and document schema-level requirements for new user-configurable semantic kinds. Artifact formats retain stable media-type mapping.

**Acceptance:** public documentation includes wildcard matching guidance.

### DR-08 — Render context represented only terminal width

**Severity:** Moderate

**Observed:** coordinator synthesized a fixed 24-row viewport from an optional width.

**Risk:** image/layout caches would be wrong once pixel geometry and live resize arrive.

**Resolution:** runtime request owns a complete `Viewport`; `RenderContext` carries it while preserving the current width compatibility accessor. Cache keys use layout-sensitive viewport data.

**Acceptance:** tests cover column-only, pixel, and full-viewport identity.

### DR-09 — Real renderer bundle had no runtime adapter path

**Severity:** Major

**Observed:** Docker validated Mermaid/MathJax/KaTeX, but the Rust runtime could not construct adapters from the resolved renderer bundle.

**Risk:** performance evidence and user configuration were disconnected from actual product execution.

**Resolution:** add a versioned raw-stdio mode to the renderer worker and a built-in engine provider that registers available one-shot adapters. Persistent worker transport remains a replaceable provider in the next stage.

**Acceptance:** Docker smoke renders through the same stdio-v1 protocol used by `ProcessEngine`; missing bundle still falls back to source/preview.

### DR-10 — Static CI checks were grouped into one opaque step

**Severity:** Moderate

**Observed:** Lua, Bash syntax, ShellCheck, and Python compilation ran in one step, obscuring the failure source.

**Resolution:** split plugin, Bash syntax, ShellCheck, Python, and Node syntax into separate named steps and retain a local aggregate target.

**Acceptance:** every static check has a distinct Actions result.

### DR-11 — Repository CI ran broad template checks for product-only changes

**Severity:** Moderate

**Observed:** the repository workflow classified some expensive gates, but the general repository job still failed on product PR state without an immediately clear product relationship.

**Resolution:** preserve the inherited repository job, capture its exact failure, and avoid weakening template/AgentCanon ownership. Product changes may fix integration defects but do not delete the inherited checks.

**Acceptance:** repository CI passes unchanged in intent; any adjustment is narrowly documented.

### DR-12 — README mixed implemented and planned behavior

**Severity:** Moderate

**Observed:** user instructions, architecture, dependency policy, and future behavior were interleaved. Some engine descriptions could be read as available in normal command mode despite the PTY host being unimplemented.

**Resolution:** rewrite README around user tasks, explicit availability tables, safe defaults, configuration examples, troubleshooting, extension links, and exact implementation boundaries.

**Acceptance:** every command shown is either currently executable or visibly marked planned.

## 4. Invariant review

| Invariant | Review result | Evidence |
| --- | --- | --- |
| ordinary output remains byte-exact | accepted | terminal and pre-display contract tests |
| control sequences never enter engines | accepted with future PTY wiring gate | terminal compatibility tests |
| one source or artifact commit per block | accepted | pre-display tests |
| safety uncertainty means passthrough | accepted | gate/detector tests |
| configuration fails before child launch | accepted | config contract test |
| failed artifacts are not cached | accepted | coordinator/cache tests |
| private mode disables persistence/source diagnostics | accepted | config tests and runtime provider policy |
| custom engines execute without a shell | accepted after ProcessEngine implementation | process-engine tests |
| environment inheritance is explicit | accepted after ProcessEngine implementation | process-engine tests |
| unsupported terminal protocol is never emitted | accepted for current text/source presenters | presenter contract |
| committed scrollback is not retroactively rewritten | accepted | architecture and current writer model |

## 5. Extensibility review

### 5.1 Engine extension

**Result:** accepted after provider/catalog introduction.

An engine requires a descriptor, implementation, and provider registration. No detector, cache, presenter, or CLI edits are required.

### 5.2 Presenter extension

**Result:** accepted for session-level presenter selection.

Image protocol implementations remain separate providers. A multi-presenter negotiation registry is intentionally deferred but does not require changing engines or detectors.

### 5.3 Cache extension

**Result:** accepted at trait boundary; persistent correctness requirements remain open.

The current key contains all in-memory identity inputs. A persistent backend must add a cryptographic source digest and key schema version.

### 5.4 Detector extension

**Result:** accepted for existing kinds and explicit document profiles.

Adding a new user-configurable semantic kind is a schema change, not an incidental plugin operation.

### 5.5 Transport extension

**Result:** accepted in design, not implemented.

A Unix PTY host can feed the existing `DisplayInterceptor` without changing render-plane interfaces. Windows ConPTY requires a later platform design.

## 6. Performance review

The architecture avoids per-block containers and surprise installation. Real-time performance depends on execution model:

```text
in-process preview/source        lowest overhead
persistent worker               target interactive path
one-shot process                compatibility/fallback path
cache hit                       no engine execution
```

The CI benchmark records p50/p95/max and artifact bytes for worker/one-shot paths plus the Rust coordinator cache. Budgets are regression gates, not universal latency guarantees; hosted runner variance is expected.

Future scheduler review gates:

- monotonic commit ordering;
- bounded in-flight count and pending bytes;
- cancellation on hard deadline or stale viewport generation;
- source fallback under backpressure;
- no detached worker/process leaks.

## 7. Security review

Accepted controls:

- no shell string in custom-engine configuration;
- no project config auto-load;
- environment deny-by-default for strict process engines;
- absolute user-configured executable and working-directory paths;
- bounded stdout/stderr/time;
- process-group termination where supported;
- redacted effective configuration;
- private no-cache mode;
- no install during render.

Open security work:

- cryptographic project trust store;
- renderer bundle signature/checksum policy;
- browser network/sandbox policy on every supported platform;
- persistent cache permissions and corruption handling;
- Windows process-tree termination semantics.

## 8. Merge acceptance gates

The PR may be marked ready and integrated only when all applicable gates pass:

```text
[ ] Rust format and Clippy on Linux and macOS
[ ] all library and contract tests
[ ] runtime composition and process-security tests
[ ] configuration examples validated
[ ] WezTerm plugin smoke
[ ] Bash syntax, ShellCheck, Python and Node static checks
[ ] real Mermaid/MathJax/KaTeX/Typst smoke
[ ] renderer/cache benchmark budgets
[ ] release archive/checksum smoke
[ ] inherited repository CI
[ ] inherited Docker pack CI
[ ] no unresolved review thread
[ ] PR is mergeable against current base
```

A skipped optional capability test is acceptable only when the profile treats the capability as optional and source fallback is exercised. A skipped required check is not acceptable.

## 9. Residual risks accepted for this PR

The following remain intentionally outside the initial integration:

- interactive child PTY hosting;
- persistent Rust worker client and worker recycling;
- live terminal capability queries;
- image protocol placement/lifecycle;
- asynchronous scheduler, cancellation, and backpressure runtime;
- disk/tiered cache;
- trusted project configuration approval UX;
- Windows ConPTY support.

These are acceptable because the current interfaces preserve the required ownership boundaries and source fallback. They remain tracked as explicit follow-up work rather than hidden TODOs.

## 10. Review conclusion

The initial architecture is acceptable for integration after the blocker/major findings above are implemented and all merge gates are green. The critical decision is to keep runtime composition and process security out of CLI-specific code. With that change, future PTY, worker, presenter, and cache work can proceed independently without weakening terminal safety or configuration determinism.
