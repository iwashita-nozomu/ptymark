# ptymark design

## 1. Scope

`ptymark` owns one narrow boundary:

> Inspect child-process output immediately before display and replace only
> complete, explicitly delimited semantic blocks.

The product does not own keyboard input, termios, signal routing, PTY sizing,
process supervision, shell startup files, prompt definitions, completion
bindings, mouse reports, bracketed paste, or terminal emulation.

The current `ptymark preview` command exercises the implemented display
pipeline. `ptymark -- COMMAND` currently validates configuration and performs a
transparent launch; the interactive PTY host and Windows ConPTY host remain
separate follow-up work.

## 2. Minimal architecture

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
    |      DecisionRequest -> RenderDecision                          |
    |                                                                 |
    +--> EngineHandoff                                                |
           EngineRequest -> EngineResponse -> RenderArtifact          |
                                      | terminal-safe display bytes   |
                                      +-------------------> stdout <---+
```

Durable boundaries:

1. `TerminalOutputGate` protects existing terminal behavior.
2. `SemanticDetector` creates complete semantic blocks.
3. `RenderDecider` selects a logical route without performing I/O.
4. `EngineHandoff` owns transfer to a concrete renderer implementation.
5. `ArtifactCache` is independent of decision and execution.
6. `DisplayPipeline` commits rendered bytes or exact source once.

There is no general provider catalog or dynamic plugin registry. Public traits
allow substitution without forcing a runtime registry before users need one.

## 3. Terminal and stream invariants

### 3.1 Byte-exact protected output

Concatenating all `TerminalOutputGate` segments reproduces the input exactly.
The gate marks a line or sequence as protected when it sees:

- ESC-based terminal controls;
- C0 controls other than newline and tab;
- carriage return or backspace;
- OSC, DCS, APC, PM, and related string controls;
- cursor positioning and erase operations;
- alternate-screen entry until a safe boundary after exit.

Protected bytes never enter semantic detection, render policy, an engine,
presentation, or cache lookup.

This rule covers prompt colors, OSC shell integration, right prompts,
autosuggestions, syntax highlighting, completion redraws, progress lines,
history selectors, fuzzy finders, and full-screen file browsers.

### 3.2 Explicit detection only

The built-in detector recognizes line-bounded forms:

- `` ```mermaid ... ``` ``;
- ``$$ ... $$``;
- `` ```math|latex|tex ... ``` ``.

A candidate is buffered only while it can still become a complete supported
block. Incomplete, unsafe, non-UTF-8, ambiguous, or oversized input is emitted
as exact source. Detection is independent of read chunk boundaries.

### 3.3 Single display commit

For every complete block, the pipeline commits exactly one result:

- cached final display bytes;
- newly rendered final display bytes;
- exact source after a non-strict failure;
- an error before replacement bytes in strict mode.

Neither a decider nor an engine handoff writes directly to terminal stdout.

## 4. Render decision contract

`DecisionRequest` contains only data relevant to deterministic rendering policy:

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

It excludes raw terminal control sequences, keyboard input, PTY descriptors,
signal state, child process control, and mutable configuration files.

Current routes are:

```text
Preview
Source
ConfiguredEngine
```

The configured policy maps semantic kind and selected backend to one route. It
performs no executable discovery, process launch, artifact validation, or
presentation.

A future policy may consider explicit terminal capability, block metadata, size
threshold, or quality mode. Any field that can change output must participate in
renderer or cache identity and must have a documented fallback.

## 5. Engine handoff contract

`EngineRequest` keeps these values separate:

```text
EngineRequest
  RenderDecision
  SemanticBlock
    exact source
    semantic body
    semantic kind
  RenderContext
```

The exact source is retained for lossless fallback. An engine receives the body
without reparsing terminal output.

`EngineResponse` contains:

```text
engine_id
RenderArtifact
  final display bytes
  cacheability
```

`ConfiguredHandoff` currently supports:

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

A later persistent worker, in-process engine, capability-aware presenter, or
bounded remote renderer can implement `EngineHandoff` without changing the
terminal gate, detector, cache, or display-commit boundary.

## 6. Installed renderer protocols

### 6.1 Mermaid

The Mermaid adapter invokes an executable directly, without a shell:

```text
stdin or input file       Mermaid body
fixed argv                --input INPUT --output OUTPUT.svg
output file               standalone SVG
```

The managed default is Mermaid CLI 11.16.0. A fixed Puppeteer configuration is
written at install time so the CLI uses the selected browser consistently.

### 6.2 Math

The math adapter invokes a `tex2svg`-compatible executable:

```text
argv[1]  one TeX expression
stdout   standalone SVG
```

The managed default is MathJax 4.1.3. Its adapter extracts the standalone
`<svg>...</svg>` element rather than returning the surrounding
`<mjx-container>` wrapper. The expression limit is 32 KiB because the initial
protocol uses one argument.

### 6.3 Presenter

External layout engines produce SVG, not terminal bytes. The initial presenter
accepts the stable Chafa-compatible subset used by ptymark:

```text
--format symbols
--colors full|none
--size COLUMNSx
SVG_PATH
```

The managed presenter rasterizes through the selected Chromium-compatible
browser and emits ANSI/Unicode symbols. It does not send capability-blind Kitty,
iTerm2, or Sixel placement commands.

### 6.4 Process policy

External programs are launched directly with fixed argv. No arbitrary shell
string, pipe, redirect, command substitution, or user-provided argv template is
accepted.

Initial limits:

- 30-second wall-clock cold-start ceiling per process;
- 8 MiB layout artifact;
- 8 MiB final display output;
- 64 KiB diagnostic output;
- 32 KiB initial math argument.

Missing executables, non-zero exits, timeout, oversized output, malformed SVG,
and presenter failure are renderer errors. Normal mode restores exact source;
strict mode returns the error before replacement bytes; failed results are not
cached.

## 7. Installation and executable resolution

Each configured executable is either:

1. an absolute path; or
2. a bare command name resolved through `PATH` during explicit resolution.

Relative paths containing directories are rejected. Generated installer output
always stores a native absolute path.

Installation selection order:

```text
explicit user selection
    -> compatible executable visible to the installer shell
    -> existing complete ptymark-managed bundle
    -> install the pinned managed bundle
    -> built-in preview when managed installation is disabled
```

The managed bundle is versioned, user-local, and absent from the global `PATH`.
It may contain a private Node runtime, Mermaid, MathJax, Puppeteer, a private
browser cache, and native ptymark aliases named `mmdc`, `tex2svg`, and `chafa`.
Normal rendering performs no package installation, browser download, or network
access.

OS and shell frontends are thin:

```text
scripts/installer.sh   Linux, macOS, WSL, Git Bash, MSYS2, Cygwin
scripts/installer.ps1  Windows-native installation
scripts/installer.cmd  cmd.exe bridge to PowerShell
```

Git Bash/MSYS2/Cygwin converts path-valued arguments with `cygpath` before
calling PowerShell. WSL remains a Linux installation.

The canonical installation contract is in `documents/ptymark-installer.md`.

## 8. Configuration

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

The schema deliberately excludes profile inheritance, automatic project
configuration, arbitrary commands, user-defined semantic kinds from untrusted
files, persistent cache paths, scheduling settings, and hot reload. A field is
added only with implemented behavior and acceptance tests.

## 9. Cache identity

`ArtifactCache` stores complete key values rather than a hash alone:

- routed renderer identity;
- semantic kind;
- exact source bytes;
- terminal columns;
- color permission;
- theme fingerprint.

`RoutedRenderer` identity includes both decision and handoff identities, so a
policy, engine path, presenter, or transport change invalidates prior entries.
The current cache is process-local and bounded by entry count and total
key-plus-value bytes. `NoopCache` supports deterministic/private operation.

Persistent cache, serialization, TTL, and tiering remain deferred until the
interactive PTY path is measured.

## 10. Shell and plugin coexistence

The installer must not edit `.bashrc`, `.zshrc`, Fish configuration, Nushell
configuration, or PowerShell profiles. Transparent command launch must preserve
shell-hook environment variables.

Compatibility is reviewed by terminal behavior profile rather than by adding
brand-specific branches. The machine-readable inventory contains twenty entries
each for Bash, Zsh, Fish, PowerShell, and Nushell. Profiles cover:

- safe glyph-rich text;
- environment and directory hooks;
- ANSI/OSC prompts;
- right prompts;
- line-editor redraw;
- completion menus;
- carriage-return progress output;
- alternate-screen interfaces.

Every profile has arbitrary chunk-boundary tests. Full-screen and
cursor-addressed fixtures must never enter semantic detection. A protected
prompt may precede a later safe Mermaid/math block, which still renders.

The complete matrix and verification-level definitions are in
`documents/shell-plugin-compatibility.md`.

## 11. WezTerm boundary

`plugin/init.lua` is an append-only launcher integration. It adds a launch-menu
entry and optional key binding without replacing existing WezTerm configuration.
`examples/wezterm.lua` selects platform defaults and permits explicit binary and
config overrides.

The plugin does not implement the future PTY interception host. Image placement
and pane capability queries remain separate work.

## 12. Release package contract

GitHub Actions builds native release executables on Ubuntu, macOS, and Windows.
Each package contains:

- the target-native `ptymark` executable;
- package-local `install.sh`, `install.ps1`, and `install.cmd` entrypoints;
- platform installer and managed-bundle scripts;
- locked renderer metadata and fixed managed adapters;
- the WezTerm plugin and example;
- configuration examples, README, license, and design documents;
- a package manifest and SHA-256 archive checksum.

A package job must execute the package-local installer with managed rendering
disabled, validate the generated configuration, run the packaged binary, and
render a built-in preview before uploading the archive.

Release publishing and code signing remain separate from artifact construction.

## 13. Extension rules

### 13.1 Decision behavior

Implement a new `RenderDecider` when selection policy changes but engine
protocols do not. Required evidence:

- explicit input field or deterministic rule;
- route-selection tests;
- cache-identity review;
- documented fallback;
- no access to raw terminal or PTY control state.

### 13.2 Engine handoff

Implement a new `EngineHandoff` when invocation, worker lifetime, artifact
transport, or presentation changes. Required evidence:

- stable handoff ID;
- fixed request/response protocol;
- bounded resource use;
- installation and version ownership;
- protocol-faithful fake tests;
- at least one real integration smoke;
- exact-source fallback.

### 13.3 New route or engine role

Add a route only when existing routes cannot express the behavior. Add an engine
role only with a concrete user need, configuration representation, installation
path, integrity policy, fallback, and Ubuntu/macOS/Windows tests.

A dynamic registry becomes justified only when users must enumerate multiple
independently installed implementations at runtime. Until then, direct typed
construction is simpler and safer.

### 13.4 New shell integration

A new plugin name does not require code. Add a behavior profile only when the
integration emits a terminal interaction not represented by current fixtures.
The profile requires byte-exact chunk tests, safe/raw classification, fallback,
and a live PTY test once the interactive host exists.

## 14. Test strategy and merge gates

Required evidence:

```text
unit and contract
  terminal gate and detector state machines
  arbitrary chunk boundaries
  render decision and typed handoff
  cache bounds and full keys
  executable resolution
  installed engine protocols
  100-entry shell compatibility inventory

native GitHub Actions
  Rust format, Clippy, and all tests on Ubuntu/macOS/Windows
  unchanged shell-profile checks
  hook environment propagation
  PowerShell/cmd/Git Bash frontends

real renderer integration
  Windows managed Mermaid/MathJax/presenter direct smoke
  Windows strict end-to-end preview
  canonical Docker managed renderer smoke

release packages
  Linux, macOS, and Windows release executable
  package-local installer
  config validation and preview smoke
  archive and SHA-256 artifact

terminal integration
  WezTerm append-only launcher example
  byte-exact prompt/OSC/completion/progress/full-screen fixtures
```

No test may assert that the interactive PTY host, persistent worker, pixel
presenter, or ConPTY host already exists.
