# ptymark System Design

<!--
@dependency-start
contract design
responsibility Defines the abstract-to-component architecture, lifecycle, invariants, and extension model for ptymark.
upstream design ./architecture.md defines the pre-display product boundary
upstream design ./configuration.md defines the user configuration contract
downstream implementation ../src/runtime.rs composes session services from an immutable configuration snapshot
downstream implementation ../src/process_engine.rs executes trusted external engines without a shell
downstream test ../tests/runtime_contract.rs verifies composition and extension boundaries
@dependency-end
-->

## 1. Purpose

`ptymark` is a **pre-display semantic rendering host**. It receives child-process output before a terminal emulator displays it, preserves terminal behavior, recognizes only explicit safe semantic blocks, delegates layout to existing engines, and commits either a rendered artifact or the original source exactly once.

This document proceeds from the most abstract product model to individual parts. It is the architectural source of truth for implementation choices. User-facing configuration is defined separately in [Configuration](./configuration.md); operational usage is defined in [Usage](./usage.md).

## 2. Product boundary

The product is divided into four planes.

```text
┌─────────────────────────────────────────────────────────────────┐
│ Transport plane                                                 │
│ keyboard ↔ terminal ↔ PTY host ↔ child process                 │
│ termios, signals, resize, exit status, process lifecycle        │
└───────────────────────────────┬─────────────────────────────────┘
                                │ child output bytes
┌───────────────────────────────▼─────────────────────────────────┐
│ Safety plane                                                    │
│ terminal-control observer and display-output gate               │
│ unsafe/control regions → byte-exact passthrough                 │
└───────────────────────────────┬─────────────────────────────────┘
                                │ safe text regions
┌───────────────────────────────▼─────────────────────────────────┐
│ Render plane                                                    │
│ detector → coordinator → engine → artifact → presenter          │
│ cache, ordering, fallback, bounds, cancellation                 │
└───────────────────────────────┬─────────────────────────────────┘
                                │ presentable bytes
┌───────────────────────────────▼─────────────────────────────────┐
│ Control plane                                                   │
│ config discovery, immutable snapshot, runtime composition,      │
│ dependency inventory, diagnostics, metrics, extension catalog  │
└─────────────────────────────────────────────────────────────────┘
```

### 2.1 Transport plane

The transport plane owns interaction fidelity. It is not configurable through renderer settings.

It owns:

- child PTY creation and process launch;
- keyboard input forwarding;
- termios/raw-mode restoration;
- signal and resize forwarding;
- child exit-status propagation;
- terminal capability and viewport observations.

It does not own semantic detection, rendering, caching, or presentation policy.

### 2.2 Safety plane

The safety plane decides whether bytes are eligible for semantic processing. It is deliberately conservative.

It owns:

- ANSI/CSI/OSC/DCS/APC/PM boundary observation;
- alternate-screen detection;
- carriage-return/backspace update-region bypass;
- invalid UTF-8 or binary-like region bypass;
- exact flushing when a transformable region becomes unsafe.

It never interprets the meaning of Mermaid, TeX, or Markdown.

### 2.3 Render plane

The render plane operates only on safe text regions. Its central rule is:

> A closed semantic block commits exactly one of a compatible rendered result or its original source.

The render plane owns:

- explicit semantic boundary detection;
- ordered engine selection and fallback;
- process/runtime limits;
- artifact identity and validation;
- cache lookup/admission;
- terminal presentation;
- pre-display commit ordering.

### 2.4 Control plane

The control plane resolves policy before terminal mutation and composes runtime services.

It owns:

- configuration source discovery and trust classification;
- profile resolution and validation;
- immutable session snapshots;
- runtime factory/provider registration;
- engine/runtime availability reporting;
- diagnostics and metrics policy;
- compatibility/version boundaries.

The stream loop never reads raw TOML tables.

## 3. Session lifecycle

A session follows one directional lifecycle.

```text
1. Discover sources
2. Parse and validate schema
3. Merge layers and resolve profile
4. Apply explicit session overrides
5. Freeze ConfigSnapshot
6. Compose RuntimePlan from providers
7. Validate required capabilities and dependencies
8. Start preview stream or child PTY
9. Process output through safety and render planes
10. Flush, restore transport state, return child status
```

No step after `ConfigSnapshot` may mutate configuration. Reloading creates a new generation for a future session; it never changes a running session underneath a child process.

## 4. Core data contracts

### 4.1 `ConfigSnapshot`

A snapshot contains:

```text
generation
stable policy fingerprint
Arc<ResolvedConfig>
Arc<ConfigProvenance>
```

The fingerprint participates in renderer-option and cache identity. It is not a security digest and must not be used as a trust-store key. Project trust requires a cryptographic digest in a future implementation.

### 4.2 `SemanticBlock`

A semantic block contains:

```text
kind
exact source bytes
engine body bytes
```

The exact source is retained until presentation succeeds. An engine receives the body, not terminal-control bytes and not unrelated surrounding output.

### 4.3 `RenderRequest`

A render request contains:

```text
SemanticBlock reference
RenderContext
preferred artifact format
```

`RenderContext` carries layout and presentation inputs that affect output identity, including viewport, theme, color policy, and an immutable options fingerprint.

### 4.4 `RenderArtifact`

A render artifact contains:

```text
artifact format/media type
bytes
engine ID and version
semantic kind
layout sensitivity
cacheability
bounded diagnostics
```

The coordinator rejects an artifact when its engine identity, semantic kind, or format does not match the selected descriptor and presenter contract.

### 4.5 `PresentableBytes`

The presenter produces terminal-safe bytes or a fallback decision. Engine implementations never emit terminal protocol escape sequences directly.

## 5. Runtime composition root

`RuntimeBuilder` is the only component allowed to translate resolved policy into concrete runtime objects.

```text
ConfigSnapshot + RuntimeRequest + RuntimeCatalog
    ↓
DetectorProvider
EngineProvider[]
CacheProvider
PresenterProvider
    ↓
RuntimePlan
    ├─ SemanticDetector
    ├─ EngineRegistry
    ├─ EngineSelector
    ├─ ArtifactCache
    ├─ ArtifactPresenter
    ├─ RenderCoordinator
    ├─ DisplayOutputGate
    └─ RenderContext
```

This removes engine/cache/presenter construction from CLI parsing and prevents TOML knowledge from leaking into stream processing.

### 5.1 Provider rules

Providers are ordinary Rust traits. They may be supplied by the built-in crate or by an embedding application.

Every provider must:

- declare a stable provider ID;
- fail before child launch when its required policy is invalid;
- report unavailable optional components without preventing source fallback;
- avoid writing stdout directly;
- avoid changing input, termios, signals, or PTY state;
- preserve deterministic registration order.

### 5.2 Duplicate ownership

Duplicate engine IDs are errors by default. An embedding application that intentionally replaces a built-in must do so before build through an explicit catalog override operation. Silent last-write-wins registration is forbidden.

### 5.3 Runtime plan report

Composition produces a report containing:

```text
snapshot generation/fingerprint
selected profile
registered engines and descriptors
unavailable optional engines and reasons
selected cache backend
selected presenter
terminal capability fingerprint
warnings
```

The report powers future `engine doctor` and structured diagnostics without exposing secret configuration values.

## 6. Component design

### 6.1 `DisplayOutputGate`

Input:

```text
arbitrary child-output chunks
```

Output:

```text
SafeText(bytes)
RawTerminalBytes(bytes)
```

Invariants:

- concatenating all output segments reproduces input exactly;
- control-sequence chunking does not affect final bytes;
- alternate-screen content is always raw;
- an incomplete control sequence is never sent to a semantic detector;
- safety policy can become stricter, never weaker, through extensions.

### 6.2 `SemanticDetector`

Input: safe text only.

Output:

```text
Passthrough(bytes)
Semantic(SemanticBlock)
```

The built-in detector recognizes explicit line-bounded Mermaid and block-math fences. Detector extensions may recognize additional explicit document constructs, but general interactive-shell mode must not enable ambiguous inline syntax.

Detector bounds include:

- maximum candidate line bytes;
- maximum semantic source bytes;
- explicit enabled kinds;
- explicit fence aliases.

Overflow restores exact source and resumes at a safe boundary.

### 6.3 `EngineRegistry`

The registry owns instantiated engines by stable ID. Descriptors are available without rendering.

Required operations:

```text
register(engine)
register_boxed(engine)
contains(id)
descriptor(id)
descriptors()
render(id, request)
```

Stable IDs are configuration contracts. Implementation changes update the engine version and therefore invalidate incompatible cache entries.

### 6.4 `EngineSelector`

The selector returns an ordered candidate list for a semantic kind. It does not start processes or inspect terminal bytes.

Selection policy may consider:

- semantic kind;
- presenter-accepted formats;
- explicit profile order;
- availability information prepared by the runtime builder.

The final source fallback remains reachable unless a validated strict profile explicitly chooses pre-launch failure.

### 6.5 `RenderCoordinator`

The coordinator owns the render transaction.

```text
candidate descriptor
    ↓ cache lookup
cache miss
    ↓ bounded engine execution
validate artifact
    ↓ cache admission
return RenderOutcome
```

`RenderOutcome` records every attempted engine, elapsed duration, cache disposition, and error reason. Failed or partial artifacts are never presented or cached.

Ordering is strict in v1. Future asynchronous scheduling must preserve a commit sequence number so later child output cannot overtake an earlier semantic block.

### 6.6 Process engine

A configured process engine is executed without a shell.

Security contract:

- `program` and `args` are separate;
- user-configured programs must be absolute paths in v1;
- environment inheritance is deny-by-default;
- only explicitly listed variables are inherited;
- explicit environment values override inherited values;
- working directory is explicit and absolute when supplied;
- stdin is the semantic body;
- stdout and stderr are independently bounded;
- timeout terminates the process group where supported;
- non-zero exit, overflow, timeout, and malformed artifacts are failures;
- no install or network action occurs implicitly.

The legacy external renderer remains a low-level API; the runtime builder uses the stricter process-engine implementation.

### 6.7 `ArtifactCache`

The cache is a replaceable service object.

```text
get(key)
insert(key, artifact)
invalidate(scope)
clear()
stats()
```

The cache key includes all material output inputs:

- exact source fingerprint;
- semantic kind;
- engine ID/version;
- artifact format;
- layout-sensitive viewport portion;
- theme/options fingerprints;
- presenter ID;
- capability fingerprint;
- cache-key schema version in persistent implementations.

The built-in memory backend is bounded by entry count and total bytes. Private mode uses `NoopArtifactCache` regardless of user cache settings.

### 6.8 `ArtifactPresenter`

The presenter owns artifact-to-terminal conversion. It receives verified terminal capabilities and presentation policy.

Presenters must:

- declare a stable ID and accepted artifact formats;
- emit only verified terminal protocols;
- preserve source fallback;
- scope image IDs and deletion to ptymark-owned artifacts;
- avoid changing input or mouse modes;
- include dimensions/theme inputs in capability fingerprints.

V1 includes source and terminal-text presenters. Image presenters remain optional extensions.

### 6.9 Diagnostics

Diagnostics are events, not text written by engines to display stdout.

A future `DiagnosticSink` implementation consumes structured events such as:

```text
config-source-loaded
engine-unavailable
engine-attempt-failed
render-timeout
cache-hit
presentation-fallback
```

Events carry IDs, durations, and bounded reasons. Source bytes are omitted by default and always omitted in private mode.

## 7. Extension model

The architecture supports six independent extension axes.

| Axis | Extension boundary | Core changes required |
| --- | --- | --- |
| Semantic detection | `DetectorProvider` / `SemanticDetector` | no, for existing semantic kinds |
| Rendering engine | `EngineProvider` / `RenderEngine` | no |
| Artifact presentation | `PresenterProvider` / `ArtifactPresenter` | no |
| Cache backend | `CacheProvider` / `ArtifactCache` | no |
| Terminal transport | future `TerminalHost` | no render-plane change |
| Diagnostics | future event sink/provider | no stream-loop change |

Adding a new semantic kind to the user configuration schema is intentionally a schema-level change. The Rust enums are marked non-exhaustive where practical so embedding applications are forced to handle future variants safely.

Detailed extension procedures are in [Extension Guide](./extension-guide.md).

## 8. Error and fallback model

Errors are classified by stage.

| Stage | Default behavior |
| --- | --- |
| Discovery/parse/schema/profile | fail before child launch |
| Required capability missing | fail before child launch only when profile explicitly requires it |
| Optional engine unavailable | record reason, continue candidate chain |
| Engine timeout/non-zero/overflow | continue candidate chain |
| Artifact validation failure | discard artifact, continue candidate chain |
| Presenter incompatibility | try compatible artifact/presenter, then source |
| Safety uncertainty | exact passthrough; strict render mode does not override safety |
| Display I/O failure | stop and return I/O error |

The original source remains owned by the transaction until presentation bytes have been written successfully.

## 9. Concurrency, cancellation, and backpressure

V1 execution is synchronous, one block at a time. The extension-ready scheduling contract is nevertheless fixed now.

Future scheduler requirements:

```text
monotonic commit sequence
bounded in-flight renders
bounded pending source/output bytes
hard deadline per render
viewport generation
cancellation token
stale-result rejection
```

When a queue or byte budget is exceeded, the oldest uncommitted semantic block is committed as exact source. The stream must continue instead of waiting indefinitely.

## 10. Security model

### 10.1 Trust classes

```text
built-in code
user-owned configuration
explicitly selected configuration
trusted project configuration
untrusted project candidate
```

Project files are never auto-executed in v1. Trust state is stored separately from project configuration. A future trust store must bind canonical directory, cryptographic configuration digest, and approval metadata.

### 10.2 Secrets

- effective configuration output redacts external-engine environment values;
- fingerprint material is never printed;
- private mode disables source-bearing diagnostics and persistent storage;
- renderer stderr is bounded and treated as diagnostic data;
- child environment and renderer environment are separate contracts.

### 10.3 Browser engines

Browser-backed engines use bounded temporary state and a documented sandbox policy. No remote font/icon fetch is allowed by default. Network access requires a separate explicit policy and review.

## 11. Compatibility and versioning

The following versions evolve independently:

```text
configuration schema version
ptymark crate version
worker protocol version
engine ID/version
artifact format/media type
presenter ID/version
persistent cache-key schema version
runtime build report schema version
```

Compatibility is explicit. A compatible range is allowed only when the corresponding protocol defines forward/backward behavior. Otherwise exact versions are required.

## 12. Test strategy

### 12.1 Pure contracts

- detector chunk-boundary equivalence;
- terminal gate byte equality;
- profile merge and cycle detection;
- engine selection/fallback;
- cache identity/admission/invalidation;
- presenter compatibility;
- snapshot generation/fingerprint stability;
- provider duplicate-ID rejection.

### 12.2 Process contracts

- argv is passed without shell interpretation;
- environment is cleared and allowlisted;
- working directory is explicit;
- timeout kills the process group;
- stdout/stderr limits are independent;
- non-zero exit and invalid artifact are discarded.

### 12.3 End-to-end contracts

- ordinary bytes remain exact;
- semantic blocks are replaced before display;
- source fallback is exact;
- terminal control data never reaches an engine;
- invalid configuration prevents child launch;
- preview runtime is constructed entirely through `RuntimeBuilder`;
- Linux and macOS native builds behave consistently;
- canonical Docker verifies real renderer artifacts and latency budgets.

## 13. Implementation map

| Abstract responsibility | Implementation |
| --- | --- |
| immutable control-plane input | `config::ConfigSnapshot` |
| runtime composition root | `runtime::RuntimeBuilder` |
| extension registration | provider traits in `runtime` |
| terminal safety | `terminal::TerminalOutputGate` |
| explicit semantic detection | `detector::FencedDetector` |
| engine catalog | `engine::EngineRegistry` |
| render transaction | `coordinator::RenderCoordinator` |
| secure process execution | `process_engine::ProcessEngine` |
| cache service | `cache::ArtifactCache` implementations |
| presentation | `presenter::ArtifactPresenter` implementations |
| pre-display commit | `predisplay::DisplayInterceptor` |

## 14. Delivery stages

### Stage A — completed in this PR

- abstract interfaces and strict safety gate;
- typed configuration and immutable snapshot;
- runtime composition root and providers;
- built-in preview/source engines;
- configured process engines;
- memory/no-op cache providers;
- source/text presenters;
- detailed user and extension documentation;
- native and canonical Docker contract checks.

### Stage B — next

- real child PTY host;
- terminal capability probes;
- persistent worker client;
- generation-aware scheduler and cancellation;
- structured diagnostics sink.

### Stage C — optional extensions

- Kitty/iTerm2/Sixel presenters;
- disk/tiered cache;
- trusted project configuration store;
- additional explicit document detectors;
- managed renderer bundle installer.

Each stage preserves the four-plane boundary and the one-commit-per-block invariant.
