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
    v                             +------------------------------+
SemanticDetector                                                 |
    | Passthrough / SemanticBlock                                |
    v                                                            |
DisplayPipeline                                                  |
    | semantic block                                             |
    v                                                            |
RenderService                                                    |
    +--> ArtifactCache                                           |
    +--> selected Renderer                                       |
           |                                                     |
           +-- built-in preview/source -> display bytes          |
           |                                                     |
           +-- installed layout engine -> SVG                    |
                                           |                     |
                                           v                     |
                                      Chafa presenter             |
                                           | terminal-safe bytes |
                                           +---------> stdout <---+
```

The durable components are:

1. `TerminalOutputGate`;
2. `SemanticDetector`;
3. `DisplayPipeline`;
4. `RenderService` plus `Renderer`;
5. independent `ArtifactCache`;
6. a concrete SVG presenter used only by external engines.

There is no runtime registry or provider graph. Concrete components are directly constructed from the
validated configuration.

## 3. Invariants

### 3.1 Terminal compatibility

Concatenating all `TerminalOutputGate` segments reproduces the input exactly.

The safety gate marks a line as raw when it sees:

- ESC-based control sequences;
- C0 controls other than newline and tab;
- carriage return or backspace;
- OSC, DCS, APC, PM, and related string controls;
- alternate-screen entry until a safe line boundary after exit.

Raw bytes never enter the semantic detector. An engine or presenter is never allowed to inspect input
bytes, signal state, termios, resize events, or existing terminal-control output.

### 3.2 Detection

The built-in detector recognizes only line-bounded explicit forms:

- ` ```mermaid ... ``` `;
- `$$ ... $$`;
- ` ```math|latex|tex ... ``` `.

A candidate remains buffered only while it can still be an opening fence. Ordinary lines are
released as soon as the prefix is no longer a valid opener.

An incomplete, unsafe, non-UTF-8, or oversized block is emitted as its exact source. Detection is
independent of input chunk boundaries.

### 3.3 Rendering and commit

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

A renderer returns a `RenderArtifact` containing display bytes or an error. It does not write stdout
directly.

The pipeline commits exactly one of:

- cached display bytes;
- newly rendered display bytes;
- original source after a non-strict error;
- an error before replacement bytes in strict mode.

An external layout engine does not produce final terminal bytes. It produces SVG, which is validated
and then handed to a presenter. This prevents Mermaid, MathJax, browser code, or another layout engine
from owning terminal escape sequences.

### 3.4 Cache

`ArtifactCache` is an independent object with `get`, `put`, `clear`, and `stats`.

The key includes complete values, not only a hash:

- selected renderer identity, including configured executable paths;
- semantic kind;
- exact source bytes;
- terminal columns;
- color permission;
- theme fingerprint.

The initial cache is process-local and bounded by both entry count and total key-plus-value byte
weight. A no-op implementation provides private or deterministic behavior without changing renderer
selection.

Only successful final display bytes are cached. Engine failures, malformed SVG, presenter failures,
source fallback, and strict errors are not cached.

Persistent cache, TTL, multi-tier storage, and serialization are excluded from the first merge.

## 4. Configuration

The schema controls only implemented behavior:

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
- user-defined semantic kinds;
- terminal capability forcing;
- persistent cache paths;
- scheduler and cancellation settings;
- hot reload.

A later change adds a field only together with runtime behavior and acceptance tests that use it.

## 5. Rendering-engine design

### 5.1 Concrete slots, not a registry

There are two semantic engine slots:

```text
Mermaid slot
  preview
  source
  mermaid-cli

Math slot
  preview
  source
  mathjax-cli
```

The slots are part of the product contract because their protocols are known. The implementation does
not accept an arbitrary executable plus arbitrary arguments. This avoids recreating a plugin system,
shell language, trust store, engine-ranking policy, or general protocol negotiation before any of
those are needed.

The built-in choices have no external dependencies:

- `preview`: terminal-safe textual representation;
- `source`: exact original fenced source.

### 5.2 Mermaid CLI adapter

The `mermaid-cli` backend expects the `mmdc` interface:

```text
stdin                  Mermaid body
argv                   --input - --output TEMP.svg
temporary output file  SVG artifact
```

The adapter:

1. verifies UTF-8 input;
2. creates a private unique temporary directory;
3. invokes `mmdc` directly without a shell;
4. waits with a fixed timeout;
5. bounds stdout and stderr;
6. reads the SVG with an artifact size limit;
7. verifies that the artifact contains an SVG element;
8. removes the temporary directory;
9. passes the SVG to the presenter.

Browser lifecycle and Mermaid layout remain owned by Mermaid CLI.

### 5.3 MathJax CLI-compatible adapter

The `mathjax-cli` backend expects a narrow `tex2svg` interface:

```text
argv[1]  one TeX expression
stdout   SVG artifact
```

The known compatibility implementation is the `tex2svg` command from `mathjax-node-cli`. A wrapper
around a newer MathJax installation may be selected when it implements the same contract.

The first adapter limits the expression to 32 KiB because it is passed as one argument. A stdin or
persistent-worker protocol is not added until real runtime measurements justify it.

### 5.4 SVG presenter

External engines return SVG. The first presenter is Chafa with fixed arguments equivalent to:

```text
chafa
  --format symbols
  --probe off
  --polite on
  --relative off
  --animate off
  --colors full|none
  --size COLUMNSx
  ARTIFACT.svg
```

Design reasons:

- `symbols` is broadly compatible ANSI/Unicode output;
- terminal probing is disabled inside the pre-display path;
- relative cursor manipulation is disabled;
- animation is disabled;
- width comes from `RenderContext`;
- engines never emit Kitty, iTerm2, Sixel, or WezTerm-specific escapes.

A future pixel presenter must be a separate implementation with capability detection, multiplexer
passthrough, placement, deletion, resize, and fallback tests. It must not change an engine adapter.

### 5.5 Path resolution

Each executable path is either:

1. an absolute path, used exactly; or
2. a bare executable name, resolved through the process `PATH`.

Relative paths containing directory components are rejected because their meaning would depend on the
child command's working directory.

Examples:

```toml
path = "mmdc"
path = "/opt/homebrew/bin/mmdc"
```

Resolution is intentionally conventional. There is no application-specific search directory,
project-local executable discovery, implicit `npx`, package-manager invocation, or auto-install.

GUI terminal applications may have a different `PATH` from an interactive shell. The installation
document therefore recommends absolute paths for WezTerm sessions.

### 5.6 Installation ownership

The native `ptymark` installation and optional engine installation are separate:

```text
ptymark binary       user installs with Cargo or a future release package
mmdc                 user installs and updates through npm or another supported Mermaid route
tex2svg              user installs or supplies as a compatible wrapper
chafa                user installs and updates through the OS package manager
```

`ptymark` does not install, update, download, or remove external engines during rendering. It also
does not download Chromium or fonts.

This keeps licensing, security updates, browser policy, package-manager ownership, and offline use
visible to the user.

### 5.7 Verification flow

Two checks have distinct purposes:

```text
ptymark config check
  syntax, schema, value limits, path form

ptymark engine check
  selected backend inventory, PATH resolution, executable-bit verification
```

`engine check` resolves only selected external engines. A configured but unselected default path does
not become a runtime dependency.

### 5.8 Failure policy

Missing executables, non-zero exits, timeout, oversized output, malformed SVG, and presenter failure
are renderer errors.

- normal mode restores exact source;
- strict mode returns the error before replacement bytes;
- failed results are not cached;
- the child-output stream remains ordered.

Fixed initial bounds:

- 5-second wall-clock timeout per external process;
- 8 MiB layout artifact;
- 8 MiB final display bytes;
- 64 KiB diagnostic output.

## 6. WezTerm integration

The WezTerm plugin is a launcher, not a renderer. It appends a launch-menu item and key binding that
start the native binary.

```text
WezTerm Lua
  choose binary, config path, shell, cwd, key

TOML
  choose renderer backends and executable paths

Rust binary
  validate config, own stream processing, execute known adapters
```

Lua does not duplicate the engine schema. Engine paths stay in the same TOML used by CLI preview and
the future PTY host.

Absolute binary and engine paths are recommended where the GUI process does not inherit the shell
`PATH`.

## 7. Rejected overdesign

### Provider and runtime catalogs

A provider graph for detector, engine, cache, and presenter construction added indirection without a
second embedding application. Direct construction remains sufficient.

### General engine registry and arbitrary process definitions

Concrete Mermaid and math adapters are justified by real installation and protocol contracts. A
general registry, candidate ranking, arbitrary command string, arbitrary argv template, and doctor
framework are still excluded.

### Profile inheritance and project trust

Single-file explicit configuration covers current behavior. Automatic project configuration would
create an executable trust boundary now that paths can select installed programs. Project-local
configuration therefore remains explicit through `--config`.

### Persistent cache and cache schema

Only in-memory reuse exists. Disk formats, migration, privacy metadata, and tiered storage remain
speculative.

### Scheduler, cancellation, and backpressure configuration

Rendering is synchronous in the preview path. Scheduling policy belongs with the asynchronous PTY
runtime and cannot be validated before that runtime exists.

### Pixel image protocol before a presenter contract

Raw SVG is not terminal output. Direct Kitty, iTerm2, Sixel, or WezTerm image placement is deferred
until one protocol is capability-checked end to end. Chafa symbols provide a safe first presenter.

### Performance gate before the production PTY path

Renderer benchmarks become meaningful after external adapters are on the interactive PTY path.
Correctness, process bounds, and real Docker smoke tests are the current merge gates.

### Release automation before releasable behavior

A release workflow is deferred until command mode hosts a PTY and runtime dependency expectations are
stable.

## 8. Extension rules

A new backend or component is justified only when all of the following exist:

1. a user-visible use case;
2. a concrete installation route;
3. a fixed input/output protocol;
4. a test using either the real engine or a protocol-faithful fake;
5. a failure fallback;
6. dependency, license, and security ownership;
7. cache identity inputs;
8. documentation of terminal compatibility.

Examples:

- add a current-MathJax adapter when its executable packaging is defined;
- add a persistent Mermaid worker after startup cost is measured on the PTY path;
- add a pixel presenter when one terminal protocol is capability-checked end to end;
- add a persistent cache only if repeated real engine work remains material after worker reuse;
- add a PTY host when input, signals, resize, and exit status have compatibility tests.

## 9. Test strategy

Required layers:

```text
unit
  detector state machine
  terminal safety gate
  cache bounds and complete keys
  built-in renderer behavior
  path validation and executable resolution
  Mermaid/MathJax adapter protocol with fake executables
  Chafa presenter protocol with a fake executable

integration
  chunk-boundary independence
  source fallback
  alternate-screen preservation
  CLI config validation
  engine check with absolute paths
  configured external rendering through the pipeline
  child exit status
  WezTerm append-only behavior

canonical Docker
  Rust format, Clippy, all tests
  Lua plugin smoke
  real Mermaid CLI SVG generation
  real Chafa symbol presentation
  real ptymark Mermaid selection through TOML
  MathJax library SVG correctness smoke

native GitHub Actions
  Linux and macOS Rust checks
```

No test may assert that an interactive PTY host or pixel presenter already exists.

## 10. Next issues

The post-merge sequence is:

1. child PTY host and terminal compatibility suite;
2. connect configured engines to the child-output path without changing input behavior;
3. choose and test one capability-checked WezTerm pixel presentation path;
4. add resize generation and cancellation;
5. measure cold, warm, and cache-hit latency on that real path;
6. add persistent workers or cache only when measurements justify them.
