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
    | SafeText                    | RawTerminalBytes
    v                             +----------------------+
SemanticDetector                                         |
    | Passthrough / SemanticBlock                        |
    v                                                    |
DisplayPipeline                                          |
    | semantic block                                     |
    v                                                    |
RenderService                                            |
    +--> ArtifactCache                                   |
    +--> Renderer                                        |
    | display bytes                                      |
    +-----------------------------> terminal stdout <----+
```

The architecture has five durable components. No runtime registry or provider graph is required to
construct them.

## 3. Invariants

### 3.1 Terminal compatibility

Concatenating all `TerminalOutputGate` segments reproduces the input exactly.

The safety gate marks a line as raw when it sees:

- ESC-based control sequences;
- C0 controls other than newline and tab;
- carriage return or backspace;
- OSC, DCS, APC, PM, and related string controls;
- alternate-screen entry until a safe line boundary after exit.

Raw bytes never enter the semantic detector.

### 3.2 Detection

The built-in detector recognizes only line-bounded explicit forms:

- ` ```mermaid ... ``` `;
- `$$ ... $$`;
- ` ```math|latex|tex ... ``` `.

A candidate remains buffered only while it can still be an opening fence. Ordinary lines are
released as soon as the prefix is no longer a valid opener.

An incomplete or oversized block is emitted as its exact source. Detection is independent of input
chunk boundaries.

### 3.3 Rendering

A renderer receives:

```text
SemanticBlock
  kind
  exact source bytes
  body bytes

RenderContext
  terminal columns
  color permission
  theme fingerprint
```

A renderer returns bytes or an error. It does not write stdout directly.

The pipeline commits exactly one of:

- rendered bytes;
- original source after a non-strict error;
- an error before any replacement bytes in strict mode.

### 3.4 Cache

`ArtifactCache` is an independent object with `get`, `put`, `clear`, and `stats`.

The in-memory key includes:

- renderer ID;
- semantic kind;
- exact source;
- terminal columns;
- color permission;
- theme fingerprint.

The initial cache is process-local and bounded by both entry count and byte count. A no-op
implementation provides private or deterministic behavior without changing renderer selection.

Persistent cache, TTL, multi-tier storage, and serialization are excluded from the first merge.

## 4. Configuration

The schema controls only behavior already implemented:

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
```

The schema deliberately excludes:

- profile inheritance;
- automatic project configuration;
- custom executable definitions;
- terminal capability forcing;
- persistent cache paths;
- scheduler and cancellation settings;
- hot reload.

Those fields are not reserved in advance. A later change adds a field only together with the
runtime behavior and acceptance tests that use it.

## 5. Renderer selection

The first executable contains two renderers:

- `builtin/preview-v1`: terminal-safe textual preview;
- `builtin/source-v1`: exact source.

Mermaid CLI 11.16.0 and MathJax 4.1.3 are selected as external layout engines for later adapters and
are smoke-tested in the canonical Docker environment.

They are not connected to terminal output in this merge because SVG generation and terminal image
placement are distinct responsibilities:

```text
semantic source -> layout engine -> SVG artifact -> terminal presenter -> escape sequence
```

Connecting SVG directly to stdout would be incorrect. The next implementation should add a small
artifact type and one presenter only after a terminal protocol has been selected and tested.

## 6. WezTerm integration

The WezTerm plugin is a launcher, not a renderer. It appends a launch-menu item and key binding that
start the native binary.

This preserves separation:

```text
WezTerm Lua:
  choose command, cwd, key, config path

Rust binary:
  validate config, own stream processing

future presenter:
  emit a capability-checked image protocol
```

Lua does not duplicate the TOML schema.

## 7. Rejected overdesign

The following structures were reviewed and removed from the integration scope.

### Provider and runtime catalogs

A provider graph for detector, engine, cache, and presenter construction added indirection without
a second implementation of those services. Direct construction is sufficient. A provider
interface may be introduced when an embedding application actually needs to replace a component.

### General engine registry and doctor CLI

The initial executable has two built-in renderers. A registry, candidate ranking, engine inventory,
and doctor command do not improve current behavior. External engine diagnostics should arrive with
the first real external adapter.

### Profile inheritance and project trust

Single-file explicit configuration covers the current behavior. Automatic project configuration
would create an executable trust boundary before custom executables exist.

### Persistent cache and cache schema

Only in-memory reuse exists. Disk formats, migration, privacy metadata, and tiered storage would be
speculative.

### Scheduler, cancellation, and backpressure configuration

Rendering is synchronous in the preview path. Scheduling policy belongs with the asynchronous PTY
runtime and cannot be validated before that runtime exists.

### Multiple overlapping design documents

Architecture, renderer, UI, extension, and review notes are consolidated here. Issues hold
future-work detail; the tracked tree keeps one current design contract.

### Performance gate before production path

Renderer benchmarks are useful after an external adapter and presenter are on the real display
path. The first merge uses correctness smoke tests and leaves latency budgets to the runtime issue.

### Release automation before releasable behavior

A release workflow is deferred until command mode hosts a PTY and the archive has a stable runtime
dependency story.

## 8. Extension rules

A new component is justified only when all of the following exist:

1. a user-visible use case;
2. a second concrete implementation or protocol;
3. a test demonstrating substitution;
4. a failure fallback;
5. documentation of dependency and security ownership.

Examples:

- add `ProcessRenderer` when Mermaid or MathJax output is consumed by the pipeline;
- add an image presenter when one protocol is capability-checked end to end;
- add a persistent cache when repeated engine startup is measured on the real runtime;
- add a PTY host when input, signals, resize, and exit status have compatibility tests.

## 9. Test strategy

The required test layers are:

```text
unit
  detector state machine
  terminal safety gate
  cache bounds
  renderer behavior

integration
  chunk-boundary independence
  source fallback
  alternate-screen preservation
  CLI config validation and exit status
  WezTerm append-only behavior

canonical Docker
  Rust format, Clippy, all tests
  Lua plugin smoke
  Mermaid CLI SVG smoke
  MathJax SVG smoke

native GitHub Actions
  Linux and macOS Rust checks
```

No test may assert that a future PTY host or image presenter already exists.

## 10. Next issues

The post-merge sequence is:

1. child PTY host and compatibility suite;
2. typed external artifact and one-shot Mermaid/MathJax adapter;
3. capability-checked WezTerm image presentation;
4. resize generation and cancellation;
5. performance measurements on that path;
6. persistent cache only if measurements justify it.
