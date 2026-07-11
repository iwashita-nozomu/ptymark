<!--
@dependency-start
contract design
responsibility Defines ptymark configuration discovery, schema, profile, and typed policy boundaries.
upstream design ../documents/architecture.md pre-display renderer ownership boundary
upstream design ../documents/renderer-architecture.md renderer coordinator and cache abstractions
downstream implementation ../src/config.rs high-level configuration service
downstream implementation ../src/config/model.rs typed raw and resolved configuration models
downstream implementation ../src/config/source.rs source discovery and trust provenance
downstream implementation ../src/config/resolve.rs merge, profile inheritance, validation, and immutable snapshots
downstream test ../tests/config_contract.rs user-facing configuration contract tests
@dependency-end
-->

# ptymark configuration

## Purpose

The configuration file controls only the **pre-display rendering path**. It can select explicit
semantic detectors, rendering engines, presentation policy, latency limits, caches, and
diagnostics. It cannot change keyboard input, termios, signal forwarding, child exit status,
window-size forwarding, or the byte-exact handling of terminal control sequences.

The stream loop never reads raw TOML. Configuration is resolved before child launch into an
immutable session snapshot:

```text
ConfigSource[]
    -> ConfigManager
    -> schema validation
    -> layer merge
    -> profile inheritance
    -> ResolvedConfig
         -> DetectionPolicy
         -> EngineSelectionPolicy
         -> RenderPolicy
         -> PresentationPolicy
         -> CachePolicyConfig
         -> DiagnosticsPolicy
```

A configuration failure occurs before a future PTY host enters raw mode or starts the child.
There is no partially applied configuration.

## File format

The human-authored format is TOML with a required schema boundary:

```toml
schema_version = 1
default_profile = "interactive"
```

Unknown keys are errors. This prevents a typo from silently falling back to a different terminal
behavior. The copyable complete example is
[`examples/ptymark.example.toml`](../examples/ptymark.example.toml).

## Discovery and precedence

The implemented v1 ordering is:

```text
built-in defaults
    < user configuration, when present
    < PTYMARK_CONFIG, when set
    < --config PATH
    < PTYMARK_PROFILE / --profile session selection
    < explicit CLI options for the current command
```

User configuration path:

- `$XDG_CONFIG_HOME/ptymark/config.toml` when `XDG_CONFIG_HOME` is set;
- Linux fallback: `~/.config/ptymark/config.toml`;
- macOS fallback: `~/Library/Application Support/ptymark/config.toml`;
- Windows path handling is deferred with the ConPTY runtime.

A working-directory `.ptymark.toml` is reported by `ptymark config paths`, but is **not loaded
automatically**. Project files may eventually define external executables, so automatic project
trust must be implemented separately. Passing a file with `--config` is an explicit user action.

`--no-config` or `PTYMARK_NO_CONFIG=1` selects built-in profiles only.

## Built-in profiles

| Profile | Intended use |
| --- | --- |
| `interactive` | explicit Mermaid and block-math detection, memory cache, source fallback |
| `source` | preserve exact semantic source and disable image-oriented presentation |
| `private` | disable cache persistence and source-bearing diagnostics |
| `ci` | deterministic source presentation with wider time budget and no cache |

A profile may extend zero or one parent:

```toml
[profiles.my-shell]
extends = "interactive"

[profiles.my-shell.cache]
max_entries = 32
```

Inheritance cycles are startup errors. Arrays replace parent arrays; scalar values override their
parent. The resolved profile does not change during a running session.

## Detection policy

```toml
[profiles.interactive.detection]
mode = "explicit-blocks" # off | explicit-blocks
mermaid = true
block_math = true
max_buffer_bytes = 1048576
max_line_bytes = 65536
```

Disabling a kind or all detection can only make behavior stricter. The following remain fixed
product invariants and are not configuration keys:

- ANSI, OSC, DCS, APC, PM, and unknown control sequences remain byte-for-byte passthrough;
- alternate-screen and cursor-addressed screen output remain bypassed;
- carriage-return or backspace update regions are not interpreted as Markdown;
- incomplete, oversized, binary, or unsafe candidates return to exact source;
- already committed textual scrollback is never erased and replaced.

## Engine policy

Engine order is explicit and deterministic:

```toml
[profiles.interactive.engines.mermaid]
candidates = ["mermaid-worker", "mermaid-cli", "source"]
preferred_artifacts = ["image/svg+xml", "text/plain"]

[profiles.interactive.engines.math]
candidates = ["mathjax-worker", "katex", "source"]
preferred_artifacts = ["image/svg+xml", "application/mathml+xml", "text/plain"]
```

The resolver makes `source` reachable as the final fallback. Built-in engine identifiers are
stable logical IDs, not executable paths. User-defined engines use an argv-based process record;
the schema has no shell command string.

## Presentation policy

```toml
[profiles.interactive.presentation]
mode = "auto" # auto | text | source
prefer = ["image/svg+xml", "text/plain"]
image_protocols = ["kitty", "iterm2", "sixel"]
unsupported = "source"
transparent_background = true
max_columns = 120
max_rows = 40
preserve_aspect_ratio = true
```

This is an allowlist, not a force switch. Runtime capability detection remains authoritative; an
unknown or unsupported transport must not receive binary image escape sequences.

## Scheduling and cache policy

```toml
[profiles.interactive.render]
soft_latency_budget_ms = 250
hard_timeout_ms = 1500
max_in_flight = 1
ordering = "strict"
prewarm = true
worker_idle_ms = 300000
worker_max_requests = 1000

[profiles.interactive.cache]
backend = "memory" # none | memory | disk | tiered
max_entries = 128
max_bytes = 33554432
private = false
```

The current executable supports `none` and bounded process-local `memory`. Disk and tiered
backends are represented in the schema so their ownership boundary is stable, but are not yet
connected to runtime storage. `private = true` resolves to no cache and disables source-bearing
diagnostics.

## Diagnostics

```toml
[diagnostics]
level = "warn"
format = "text"
sink = "stderr"
include_source = false
metrics = true
```

Renderer diagnostics never share stdout with terminal content. A file path is required when the
sink includes a file. Source inclusion defaults to false and is forced off by private mode.

## Commands

```bash
ptymark config paths
ptymark config check
ptymark config check --config ./ptymark.toml
ptymark config show --profile interactive
ptymark config show --config ./ptymark.toml --provenance
```

`paths` labels the project candidate as untrusted and not loaded. `check` performs full parsing,
inheritance, and cross-field validation. `show` prints the normalized immutable session policy;
provenance is emitted separately so it cannot be mistaken for effective TOML.

Preview accepts the same source selectors:

```bash
ptymark preview --config ./ptymark.toml --profile private document.md
ptymark preview --no-config --no-cache document.md
```

## Implementation status and issue ownership

The typed model, discovery, built-in profiles, single-parent inheritance, validation, CLI
introspection, and preview wiring are implemented in the renderer skeleton branch. The remaining
user-facing concerns stay independently tracked:

- [#5 configuration umbrella](https://github.com/iwashita-nozomu/ptymark/issues/5)
- [#6 discovery, precedence, provenance, and project trust](https://github.com/iwashita-nozomu/ptymark/issues/6)
- [#7 profiles, inheritance, and session overrides](https://github.com/iwashita-nozomu/ptymark/issues/7)
- [#8 detection and immutable terminal safety](https://github.com/iwashita-nozomu/ptymark/issues/8)
- [#9 engine selection and custom adapter trust](https://github.com/iwashita-nozomu/ptymark/issues/9)
- [#10 presentation and terminal capabilities](https://github.com/iwashita-nozomu/ptymark/issues/10)
- [#11 latency, cancellation, ordering, and backpressure](https://github.com/iwashita-nozomu/ptymark/issues/11)
- [#12 cache backends and privacy](https://github.com/iwashita-nozomu/ptymark/issues/12)
- [#13 diagnostics and benchmark reporting](https://github.com/iwashita-nozomu/ptymark/issues/13)
- [#14 WezTerm profile bridge](https://github.com/iwashita-nozomu/ptymark/issues/14)
- [#15 validation, editor schema, and migration](https://github.com/iwashita-nozomu/ptymark/issues/15)
- [#16 dependency provisioning and compatibility](https://github.com/iwashita-nozomu/ptymark/issues/16)
