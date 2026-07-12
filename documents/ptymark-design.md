# ptymark design

## 1. Scope

`ptymark` owns one narrow boundary:

> inspect child-process output immediately before display and replace only complete, explicitly
> delimited semantic blocks.

It does not own keyboard input, termios, signal routing, PTY sizing, process supervision, shell
integration, or terminal emulation. The future PTY host must preserve those facilities and insert
this library only in the child-output path.

## 2. Architecture

```text
child output bytes
    |
    v
TerminalOutputGate
    | SafeText                         | RawTerminalBytes
    v                                  +------------------------------+
SemanticDetector                                                      |
    | Passthrough / SemanticBlock                                     |
    v                                                                 |
DisplayPipeline                                                       |
    | semantic block                                                  |
    v                                                                 |
RenderService <--------------------> ArtifactCache                     |
    |                                                                 |
    v                                                                 |
RoutedRenderer                                                        |
    |                                                                 |
    +--> RenderDecider                                                |
    |      DecisionRequest                                            |
    |          -> RenderDecision                                      |
    |                                                                 |
    +--> EngineHandoff                                                |
           EngineRequest                                              |
               -> EngineResponse                                      |
               -> RenderArtifact                                      |
                         | terminal-safe bytes                         |
                         +-------------------------------> stdout <----+
```

The durable boundaries are:

1. `TerminalOutputGate` protects existing terminal behavior;
2. `SemanticDetector` creates complete semantic blocks;
3. `RenderDecider` selects a logical route without performing I/O;
4. `EngineHandoff` owns transfer to a concrete renderer implementation;
5. `ArtifactCache` is independent of decision and execution;
6. `DisplayPipeline` commits rendered bytes or exact source once.

There is no general plugin registry or provider catalog. The first implementation uses concrete
configured decision and handoff objects, while the public traits permit later substitution without
changing the terminal pipeline.

## 3. Terminal and stream invariants

### 3.1 Byte-exact bypass

Concatenating all `TerminalOutputGate` segments reproduces the input exactly.

The gate marks a line or sequence as raw when it sees:

- ESC-based terminal controls;
- C0 controls other than newline and tab;
- carriage return or backspace;
- OSC, DCS, APC, PM, and related string controls;
- alternate-screen entry until a safe line boundary after exit.

Raw bytes never enter the semantic detector, decision stage, handoff stage, or cache.

### 3.2 Explicit detection only

The built-in detector recognizes only line-bounded forms:

- ` ```mermaid ... ``` `;
- `$$ ... $$`;
- ` ```math|latex|tex ... ``` `.

A candidate remains buffered only while it can still be an opener or a complete block. Incomplete,
unsafe, non-UTF-8, or oversized input is emitted as exact source. Detection is independent of chunk
boundaries.

### 3.3 Single commit

For every complete block, the pipeline commits exactly one of:

- cached final display bytes;
- newly rendered final display bytes;
- exact source after a non-strict failure;
- an error before replacement bytes in strict mode.

Neither a decider nor an engine handoff writes directly to terminal stdout.

## 4. Decision contract

### 4.1 Request

`DecisionRequest` contains only data that is safe and relevant to rendering policy:

```text
DecisionRequest
  SemanticBlock
    kind
    exact source bytes
    semantic body bytes
  RenderContext
    terminal columns
    color permission
    theme fingerprint
```

It intentionally excludes:

- raw terminal control sequences;
- keyboard input;
- PTY descriptors;
- signal state;
- child environment and process control;
- mutable configuration files.

### 4.2 Result

`RenderDecision` currently selects one `RenderRoute`:

```text
Preview
Source
ConfiguredEngine
```

The route is logical. It does not contain shell text, arbitrary argv, executable discovery results,
or terminal bytes.

### 4.3 Policy behavior

`RenderDecider` implementations must be deterministic and side-effect free. The configured policy
currently maps each semantic kind to the corresponding configured backend:

```text
Mermaid + preview       -> Preview
Mermaid + source        -> Source
Mermaid + mermaid-cli   -> ConfiguredEngine
Math + preview          -> Preview
Math + source           -> Source
Math + mathjax-cli      -> ConfiguredEngine
```

A future policy may consider additional explicit request fields, such as terminal capabilities,
block metadata, size thresholds, or user-selected quality mode. When a new field can change output,
it must also participate in renderer identity or cache identity.

Environment-dependent automatic selection is not added implicitly. A new heuristic requires:

1. a user-visible reason;
2. deterministic precedence;
3. diagnostics explaining the selected route;
4. cache-key review;
5. fallback tests.

## 5. Engine handoff contract

### 5.1 Typed request

After a decision is made, `RoutedRenderer` creates an `EngineRequest`:

```text
EngineRequest
  RenderDecision
  SemanticBlock
    exact source
    body
    semantic kind
  RenderContext
```

The exact source and body remain separate. An adapter does not need to reparse terminal output, and
source fallback can remain lossless.

### 5.2 Typed response

`EngineHandoff` returns an `EngineResponse`:

```text
EngineResponse
  engine_id
  RenderArtifact
    final display bytes
    cacheability
```

`engine_id` provides a stable attribution point for future diagnostics, timing, and failure reports.
The first `RenderService` consumes the final artifact; it does not yet expose per-engine metrics.

### 5.3 Current handoff

`ConfiguredHandoff` owns three concrete destinations:

```text
Preview route
  -> builtin/preview-v1

Source route
  -> builtin/source-v1

ConfiguredEngine route
  -> ConfiguredRenderer
       Mermaid CLI or MathJax-compatible layout
       -> validated SVG
       -> Chafa symbols presenter
       -> terminal-safe bytes
```

This keeps existing installed-engine behavior while removing backend selection from the terminal
pipeline.

### 5.4 Future handoff implementations

A later implementation can replace `EngineHandoff` without modifying `DisplayPipeline`,
`TerminalOutputGate`, `SemanticDetector`, or `ArtifactCache`. Examples include:

- a persistent Mermaid or MathJax worker;
- an in-process engine;
- a typed SVG artifact pipeline with a capability-aware presenter;
- a bounded remote renderer with an explicit trust policy;
- a test handoff that records requests without running a process.

A new handoff must still return final display bytes or a failure. It may not emit partial bytes to
stdout.

## 6. Renderer identity and cache

`RoutedRenderer` derives its identity from both components:

```text
render decision ID
engine handoff ID
```

The configured decision ID includes selected logical backends. The configured handoff ID includes
installed-engine and presenter paths through the underlying configured renderer identity.

The cache key stores complete values, not only a hash:

- routed renderer identity;
- semantic kind;
- exact source bytes;
- terminal columns;
- color permission;
- theme fingerprint.

The initial cache is process-local and bounded by entry count and total key-plus-value bytes. A
no-op implementation provides deterministic or private behavior. Failed decisions, failed handoffs,
source fallback, and strict errors are not cached.

Persistent cache, TTL, serialization, and tiering remain out of scope until measured on the real PTY
path.

## 7. Configuration

The schema controls implemented behavior only:

```toml
schema_version = 1

[detection]
mermaid = true
math = true
max_block_bytes = 1048576

[rendering]
mode = "preview"
strict = false
columns = 80

[cache]
enabled = true
max_entries = 128
max_bytes = 33554432

[engines.mermaid]
backend = "preview"
path = "mmdc"

[engines.math]
backend = "preview"
path = "tex2svg"

[engines.presenter]
path = "chafa"
```

The schema deliberately excludes:

- profile inheritance;
- automatic project configuration;
- arbitrary command strings and argv templates;
- user-defined semantic kinds from untrusted files;
- persistent cache paths;
- scheduling and cancellation settings;
- hot reload.

A later field is added only with implemented behavior and acceptance tests.

## 8. Installed engine protocols

### 8.1 Mermaid CLI

The `mermaid-cli` adapter expects:

```text
stdin                  Mermaid body
argv                   --input - --output TEMP.svg
temporary output file  SVG artifact
```

The adapter verifies UTF-8, uses a unique temporary directory, invokes `mmdc` without a shell,
bounds execution and output, validates SVG, removes temporary files, and passes the artifact to the
presenter.

### 8.2 MathJax-compatible CLI

The `mathjax-cli` adapter expects:

```text
argv[1]  one TeX expression
stdout   SVG artifact
```

The known compatibility implementation is `tex2svg` from `mathjax-node-cli`. A wrapper around a
newer MathJax release may be selected when it follows the same narrow contract. The initial adapter
limits the expression to 32 KiB because it is passed as one argument.

### 8.3 Chafa presenter

External engines produce SVG, not terminal display bytes. Chafa is invoked with a fixed safe profile:

```text
--format symbols
--probe off
--polite on
--relative off
--animate off
--colors full|none
--size COLUMNSx
```

This avoids capability-blind pixel protocols and cursor-relative image placement. A future pixel
presenter belongs in a new handoff or presenter implementation; it must not change engine syntax.

### 8.4 Executable resolution and installation

Each executable is either:

1. an absolute path, used exactly; or
2. a bare name resolved through the `ptymark` process `PATH`.

Relative paths with directories are rejected. There is no implicit `npx`, package-manager command,
project-local search, or automatic install.

Installation ownership remains explicit:

```text
ptymark binary  Cargo or future release package
mmdc            user-managed Mermaid CLI installation
tex2svg         user-managed compatible MathJax CLI or wrapper
chafa           user-managed OS package
```

`ptymark config check` validates schema and path form. `ptymark engine check` resolves only selected
external tools.

## 9. Process and failure policy

External processes are invoked directly with fixed argv. No shell is used.

Initial limits:

- 5-second wall-clock timeout per process;
- 8 MiB layout artifact;
- 8 MiB final display output;
- 64 KiB diagnostic output.

Missing executables, non-zero exits, timeout, oversized output, malformed SVG, and presenter failure
are renderer errors.

- normal mode restores exact source;
- strict mode returns the error before replacement bytes;
- failed results are not cached;
- stream order is preserved.

## 10. Extension rules

### 10.1 Extend decision behavior

Implement a new `RenderDecider` when selection policy changes but engine protocols do not. Required
evidence:

- the new input field or rule;
- deterministic route selection tests;
- cache identity review;
- a documented fallback;
- no access to raw terminal or PTY control state.

### 10.2 Extend engine handoff

Implement a new `EngineHandoff` when invocation, worker lifetime, artifact transport, or presentation
changes. Required evidence:

- a stable handoff ID;
- a fixed request/response protocol;
- bounded resource use;
- installation and version ownership;
- protocol-faithful fake tests;
- at least one real integration smoke test;
- exact-source fallback.

### 10.3 Add a route

Add a new `RenderRoute` only when neither existing route can express the behavior. A new route must
arrive with both a decider case and a handoff implementation. Unimplemented route names are not
reserved in configuration.

### 10.4 Avoid premature registries

The trait boundary supports embedding and testing without requiring a dynamic registry. A registry
becomes justified only when there are multiple independently installed implementations that users
must enumerate or select at runtime. Until then, direct construction remains simpler and safer.

## 11. Test strategy

Required layers:

```text
unit
  terminal safety gate
  detector state machine
  configured decision mapping
  typed decision-to-handoff transfer
  cache bounds and complete keys
  executable resolution
  engine and presenter process protocols

integration
  custom RenderDecider substitution
  custom EngineHandoff substitution
  exact source/body/context preservation
  chunk-boundary independence
  source fallback and strict failure
  alternate-screen preservation
  CLI configuration and engine checks
  configured external rendering
  child exit status
  WezTerm append-only behavior

canonical Docker
  Rust format, Clippy, all tests
  real Mermaid CLI SVG generation
  real Chafa symbol presentation
  real configured Mermaid path through ptymark
  MathJax SVG correctness smoke

native GitHub Actions
  Linux and macOS Rust checks
```

No test may assert that an interactive PTY host, persistent worker, or pixel presenter already exists.

## 12. Next sequence

1. connect the pre-display pipeline to a child PTY without changing input behavior;
2. carry terminal capability context into `DecisionRequest` only after it is measured and tested;
3. introduce a typed intermediate artifact if a second presenter requires it;
4. add resize generation and cancellation to the PTY runtime;
5. measure cold, warm, and cache-hit latency on the real path;
6. add persistent workers or cache only when measurements justify them.
