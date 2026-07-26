# Ptymark design

<!--
@dependency-start
contract design
responsibility Defines the current rendering architecture, terminal safety, format adaptation, and extension ownership.
upstream design ../README.md user-facing behavior
upstream design ./openmath.md structured math input contract
downstream implementation ../src/pipeline.rs display pipeline
downstream implementation ../src/terminal.rs terminal safety gate
downstream implementation ../src/format_adapter.rs source-format adaptation
downstream implementation ../src/runtime.rs canonical composition root
@dependency-end
-->

## 1. Scope

Ptymark owns one narrow boundary:

> Inspect child-process output immediately before display and replace only complete, explicitly delimited semantic blocks found in terminal-safe text.

The product does not interpret keyboard input, shell hooks, prompt definitions, completion bindings, mouse reports, bracketed paste, or arbitrary Markdown. Native session code owns PTY/ConPTY lifecycle, input forwarding, resize propagation, parent terminal restoration, and child exit status. Semantic rendering receives only output segments already classified as safe.

Current user paths are implemented:

```text
ptymark preview [FILE]       stream or file input
ptymark run -- COMMAND       pipe-oriented command execution
ptymark -- COMMAND           native Unix PTY or Windows ConPTY session
```

## 2. Current architecture

```text
child output bytes
    |
    v
TerminalOutputGate
    | SafeText                           | RawTerminalBytes
    v                                    +------------------------------+
SemanticDetector                                                        |
    | Passthrough / SemanticBlock                                       |
    v                                                                   |
DisplayPipeline                                                         |
    | complete semantic block                                           |
    v                                                                   |
RenderService <-----------------------> ArtifactCache                    |
    | cache miss                                                         |
    v                                                                   |
OpenMathAdapterRenderer                                                 |
    | unchanged block or OpenMath body adapted to TeX                   |
    v                                                                   |
RoutedRenderer                                                          |
    +--> RenderDecider                                                  |
    |      DecisionRequest -> RenderDecision                            |
    |                                                                   |
    +--> EngineHandoff                                                  |
           EngineRequest -> EngineResponse -> RenderArtifact            |
                                      | terminal-safe display bytes     |
                                      +---------------------> stdout <---+
```

Durable ownership boundaries:

1. `TerminalOutputGate` protects terminal behavior and emits byte-exact raw segments.
2. `SemanticDetector` recognizes only complete explicit blocks and retains exact source.
3. `DisplayPipeline` orders passthrough, render attempts, fallback, and one-time display commit.
4. `RenderService` owns cache lookup and insertion around one renderer identity.
5. `OpenMathAdapterRenderer` converts a structured math source format without creating a new engine role.
6. `RenderDecider` selects a logical route without I/O.
7. `EngineHandoff` invokes the selected implementation and returns an artifact without writing stdout.
8. `PipelineFactory` is the single composition root used by preview, filtered commands, and native sessions.
9. `PipelinePump` owns stream reads, flush policy, interrupted reads, and PTY EOF handling.
10. `NativeTerminalSession` owns operating-system terminal and child lifecycle.

There is no dynamic provider registry. A trait or registry is introduced only after a second materially different implementation needs runtime substitution.

## 3. Terminal and stream invariants

### 3.1 Byte-exact protected output

Concatenating all `TerminalOutputGate` segments reproduces the input exactly. The gate protects a line or sequence when it sees:

- ESC-based terminal controls;
- C0 controls other than newline and tab;
- a bare carriage return or backspace;
- OSC, DCS, APC, PM, and related string controls;
- cursor positioning or erase operations;
- alternate-screen entry until a safe boundary after exit;
- invalid UTF-8 or binary-like data that cannot safely enter semantic parsing.

Protected bytes never enter semantic detection, source-format conversion, render policy, an engine, presentation, or cache lookup. This preserves prompts, OSC shell integration, right prompts, autosuggestions, syntax highlighting, completion redraws, progress lines, fuzzy finders, pagers, editors, and full-screen applications.

An exact CRLF pair may be treated as one logical newline while both bytes remain recoverable. A bare carriage return is a redraw control and protects the rest of that line.

### 3.2 Explicit detection only

The detector recognizes these line-bounded forms:

````text
```mermaid ... ```
$$ ... $$
```math|latex|tex ... ```
```openmath ... ```
````

OpenMath is detected only through the explicit fence. Raw `OMOBJ` XML elsewhere is ordinary text. Inline `$...$`, headings, lists, and XML-looking prompt output are not interpreted.

A candidate is buffered only while it can still become a complete supported block. Incomplete, oversized, unsafe, or ambiguous input is emitted as exact source. Detection is independent of read chunk boundaries.

### 3.3 Single display commit

For every complete block, the pipeline commits exactly one outcome:

- cached final display bytes;
- newly rendered final display bytes;
- exact original source after a non-strict conversion or rendering failure;
- an error before replacement bytes in strict mode.

No decider, adapter, engine, or presenter writes directly to terminal stdout. Failed results are not cached.

## 4. Semantic kind and source format

`SemanticBlock` deliberately separates renderer role from source representation:

```text
SemanticBlock
  kind    Math | Mermaid
  format  Tex | OpenMath | Mermaid
  source  exact fenced bytes
  body    source-format body bytes
```

`BlockKind` remains small because engine policy is role-oriented. TeX and OpenMath both target the `Math` role. `SemanticFormat` records how to interpret the body before that role is rendered.

This separation prevents a structured encoding from duplicating:

- math engine configuration;
- installer roles;
- presenter configuration;
- fallback policy;
- cache ownership;
- terminal safety rules.

A new source format should reuse an existing semantic role when it ultimately reaches the same engine contract. A new role is justified only when selection, installation, or output protocol is materially different.

## 5. OpenMath adaptation

OpenMath conversion is local, bounded, and deterministic:

```text
OpenMath XML body
  -> bounded XML object parser
  -> standard-CD-aware TeX presentation
  -> existing Math preview or MathJax-compatible body protocol
```

The adapter:

- requires one `OMOBJ` root in the OpenMath namespace;
- performs no file access, external entity resolution, network request, or remote Content Dictionary lookup;
- rejects `DOCTYPE`, unsupported declarations, malformed XML, excessive depth/node count, and `OMR` references;
- maps common official symbols to readable TeX;
- preserves unknown `cd.name` symbols through a deterministic generic operator;
- retains the original fenced source separately from the converted body.

Source mode selects `SourceRenderer` before the adapter is composed. A configured math backend of `source` disables adaptation. Therefore malformed OpenMath remains exactly recoverable without first becoming valid XML.

The full format contract and supported constructors are in [`openmath.md`](./openmath.md).

## 6. Render decision and handoff

`DecisionRequest` contains only:

```text
SemanticBlock
  kind
  source format
  exact source bytes
  semantic body bytes
RenderContext
  terminal columns
  color permission
  theme fingerprint
```

It excludes raw terminal sequences, keyboard input, PTY descriptors, signals, child process control, and mutable configuration files.

Current routes are:

```text
Preview
Source
ConfiguredEngine
```

`ConfiguredDecider` maps semantic role and selected backend to one route. It performs no executable discovery, process launch, conversion, artifact validation, or presentation.

`EngineRequest` keeps the decision, block, and render context separate. `EngineResponse` returns an engine identity and `RenderArtifact` containing final display bytes plus cacheability.

`ConfiguredHandoff` currently executes:

```text
Preview
  -> builtin/preview-v1

Source
  -> builtin/source-v1

ConfiguredEngine
  -> Mermaid CLI or MathJax-compatible layout engine
  -> validated standalone SVG
  -> terminal-safe symbols presenter
  -> final display bytes
```

A future persistent worker, in-process engine, capability-aware presenter, or bounded remote renderer may implement `EngineHandoff` without changing terminal classification, detection, format adaptation, cache ownership, or display commit.

## 7. Installed renderer protocols

### 7.1 Mermaid

The Mermaid adapter invokes an executable directly, without a shell:

```text
stdin or input file  Mermaid body
fixed argv           --input INPUT --output OUTPUT.svg
output file          standalone SVG
```

The managed default is Mermaid CLI 11.16.0. Install-time browser configuration keeps the selected Chromium-compatible browser deterministic.

### 7.2 Math

The math adapter invokes a `tex2svg`-compatible executable:

```text
argv[1]  one TeX expression
stdout   standalone SVG
```

The managed default is MathJax 4.1.3. The adapter extracts a standalone `<svg>...</svg>` element. The initial argument protocol limits the converted TeX expression to 32 KiB.

OpenMath does not add another process. Its bounded in-process conversion produces the TeX body consumed here.

### 7.3 Presenter

External layout engines produce SVG, not terminal bytes. The current presenter accepts the Chafa-compatible subset:

```text
--format symbols
--colors full|none
--size COLUMNSx
SVG_PATH
```

The managed presenter emits ANSI/Unicode symbols and does not send capability-blind Kitty, iTerm2, or Sixel placement commands.

### 7.4 Process policy and bounds

External programs are launched directly with fixed argv. Configuration cannot contain a shell string, pipe, redirect, command substitution, or arbitrary argv template.

Current bounds include:

- 10 seconds across one engine-and-presentation attempt;
- 8 MiB layout artifact;
- 8 MiB final display output;
- 1 MiB pending later output while a semantic block resolves;
- 64 KiB sanitized diagnostic output;
- 32 KiB initial MathJax argument;
- 1 MiB semantic block source, plus OpenMath depth and node-count limits.

Missing executables, non-zero exits, timeout, oversized output, malformed SVG, conversion failure, and presenter failure are render errors. Normal mode restores exact source; strict mode returns the error before replacement bytes.

## 8. Installation and executable resolution

Each configured executable is either an absolute path or a bare command name resolved through `PATH` during explicit resolution. Relative paths containing directories are rejected. Installer-generated configuration stores native absolute paths.

Selection order:

```text
explicit user selection
  -> compatible executable visible to the installer shell
  -> existing complete managed bundle
  -> install the pinned managed bundle
  -> built-in preview when managed installation is disabled
```

The managed bundle is versioned, user-local, and absent from global `PATH`. Normal rendering performs no package installation, browser download, or network access.

Platform frontends remain thin:

```text
scripts/installer.sh   Linux, macOS, WSL, Git Bash, MSYS2, Cygwin
scripts/installer.ps1  Windows-native installation
scripts/installer.cmd  cmd.exe bridge to PowerShell
```

The canonical installation contract is [`ptymark-installer.md`](./ptymark-installer.md).

## 9. Configuration

The current strict schema controls implemented behavior only:

```toml
schema_version = 1

[detection]
mermaid = true
math = true                 # TeX and OpenMath
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

The schema excludes automatic project configuration, arbitrary commands, untrusted user-defined semantic kinds, persistent cache paths, scheduling, and hot reload. A field is added only with implemented behavior and acceptance tests.

## 10. Cache identity

`ArtifactCache` stores complete key values rather than a hash alone:

- complete renderer identity, including the OpenMath adapter version and enabled state;
- semantic kind;
- exact source bytes;
- terminal columns;
- color permission;
- theme fingerprint.

`RoutedRenderer` identity includes decision and handoff identities. The outer adapter identity includes its converter and inner renderer, so a mapping or transport change invalidates prior entries. The current cache is process-local and bounded by entry count and total key-plus-value bytes. `NoopCache` supports source, safe, private, and deterministic operation.

## 11. Shell, plugin, and WezTerm coexistence

The installer does not edit shell profiles. Transparent launch preserves shell-hook environment variables. Compatibility is reviewed by terminal behavior profile rather than brand-specific code. Fixtures cover safe glyph text, ANSI/OSC prompts, right prompts, line-editor redraw, completion menus, progress updates, cursor addressing, and alternate-screen interfaces across Bash, Zsh, Fish, PowerShell, and Nushell.

`plugin/init.lua` is an append-only WezTerm launcher. It adds a launch-menu entry and optional key binding without replacing existing configuration. The spawned Ptymark process—not Lua—owns the active native PTY/ConPTY host, validation, detection, rendering, fallback, and session modes.

The complete matrix is [`shell-plugin-compatibility.md`](./shell-plugin-compatibility.md). Native session behavior is [`interactive-session.md`](./interactive-session.md).

## 12. Source distribution and local verification

GitHub Releases are source-only: immutable tag, release notes, and GitHub-generated source snapshots. Ptymark does not upload project-built executables, installer archives, renderer bundles, binary checksums, binary manifests, or binary attestations.

CI still builds native executables on Ubuntu, macOS, and Windows, assembles package layouts in temporary workspaces, runs package-local installation/configuration/doctor/preview smoke tests, and deletes outputs. No executable package is uploaded as a workflow artifact.

Any future binary channel requires an approved signing, notarization, package-manager trust, lifecycle, rollback, and revocation contract. See [`release.md`](./release.md).

## 13. Extension rules

### 13.1 New source format

Add a `SemanticFormat` and a bounded adapter when the source representation changes but the semantic engine role does not. Required evidence:

- explicit, line-bounded detection;
- parser/resource limits and no hidden I/O;
- deterministic conversion identity;
- exact-source fallback and strict failure behavior;
- source/safe/private behavior;
- cache-identity review;
- arbitrary chunk-boundary and protected-terminal tests;
- one format contract under `documents/` and one runnable example.

### 13.2 Decision behavior

Implement a new `RenderDecider` when selection policy changes but engine protocols do not. It must be deterministic, side-effect free, cache-reviewed, and unable to inspect raw terminal or PTY state.

### 13.3 Engine handoff or role

Implement a new `EngineHandoff` when invocation, worker lifetime, artifact transport, or presentation changes. It requires a stable ID, fixed protocol, bounded resources, installation/version ownership, protocol-faithful tests, real integration smoke, and exact-source fallback.

Add a renderer role only when existing roles cannot express the user need. A role requires configuration, installation, integrity, fallback, and Ubuntu/macOS/Windows evidence.

### 13.4 Shell integration

A new plugin name does not require code. Add a behavior profile only when it emits an interaction not represented by current fixtures. The profile requires byte-exact chunk tests, safe/raw classification, fallback, and native session evidence.

## 14. Test strategy and merge gates

Required evidence includes:

```text
unit and contract
  terminal gate and detector state machines
  arbitrary chunk boundaries
  OpenMath parser bounds, entities, references, and CD fallback
  source-format adapter and exact source preservation
  render decisions and typed handoff
  cache bounds and complete keys
  executable resolution and installed protocols

native GitHub Actions
  Rust format, Clippy with warnings denied, and all tests on Ubuntu/macOS/Windows
  real Unix PTY and Windows ConPTY behavior
  unchanged shell profiles and hook environment
  PowerShell/cmd/Git Bash frontends

real renderer integration
  managed Mermaid/MathJax/presenter smoke
  strict end-to-end preview
  canonical Docker checks

ephemeral package smoke
  release-mode executable
  package-local installer
  config, doctor, and preview validation
  deletion without executable artifact upload

documentation
  task-oriented root map
  format contract and runnable example
  links to the current architecture and recovery path
```

No test may claim an unimplemented pixel-placement protocol, persistent worker, persistent cache, or signed binary distribution channel.
