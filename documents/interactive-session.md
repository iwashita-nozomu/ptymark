<!--
@dependency-start
contract design
responsibility Defines the native PTY/ConPTY interactive session used by `ptymark -- COMMAND`.
upstream implementation ../src/interactive.rs orchestrates one interactive command session.
upstream implementation ../src/native_session.rs owns native PTY/ConPTY allocation, parent terminal mode, input forwarding, resize, and child lifecycle.
upstream implementation ../src/stream.rs owns byte pumping, flush policy, interrupted reads, and platform PTY EOF handling.
upstream design ./ptymark-design.md defines terminal-safety and pre-display rendering invariants.
downstream test ../tests/interactive_pty_contract.rs exercises real Unix PTY and Windows ConPTY processes.
@dependency-end
-->

# Interactive PTY and ConPTY session

## User-facing command

```text
ptymark [--config PATH] [--source|--safe] [--private] -- COMMAND [ARG...]
```

This is the practical interactive path. `ptymark` allocates the operating system's native
pseudo-terminal, launches the child as its foreground terminal process, forwards keyboard bytes,
filters child output immediately before display, propagates terminal size changes, and returns the
child exit status.

```text
parent keyboard ---------------------------> PTY / ConPTY input
parent terminal size ----------------------> PTY / ConPTY resize
child PTY output
    -> TerminalOutputGate
    -> SemanticDetector
    -> RenderDecider
    -> EngineHandoff
    -> ArtifactCache
    -> parent terminal display
```

The implementation uses the operating system backend selected by `portable-pty`:

```text
Linux / macOS / other Unix  native Unix PTY
Windows                     native ConPTY
```

No shell command string is synthesized. The executable and each argument remain separate values.

## Per-session modes

The interactive host is retained in every mode; only pre-display rendering policy changes:

- `--source` keeps semantic detection active and replaces each complete block with its exact source;
- `--safe` uses the passthrough detector and never invokes a semantic renderer or presenter;
- `--private` keeps the selected rendering policy but selects `NoopCache` for the invocation.

`--source` and `--safe` are mutually exclusive. `--private` can accompany either mode. All options are
resolved before configuration is loaded, the child is spawned, or parent terminal raw mode is entered.
They are immutable for the lifetime of the session and never change keyboard forwarding, resizing,
signal behavior, child argv, or child exit status.

The current runtime has only process-local memory caching and no persistent source-bearing diagnostic
sink. `--private` disables that cache now and owns the forward-compatible contract for suppressing any
future persistent diagnostics without changing the CLI.

## Responsibility split

The runtime is intentionally split at stable ownership boundaries:

```text
interactive.rs
    command-level orchestration and failure precedence

native_session.rs
    parent terminal state
    native PTY / ConPTY child lifecycle
    input forwarding
    resize observation

runtime.rs
    detector / renderer / cache composition

stream.rs
    reader -> DisplayPipeline -> display pumping
    standard-stream versus interactive flush and EOF policy
```

`NativeTerminalSession` is a concrete cross-platform object backed by `portable-pty`, not a speculative
registry. A new session abstraction should be introduced only when a second materially different host
requires substitution. `PipelineFactory` remains the public composition seam; CLI parsing and process
ownership remain crate-internal.

## Terminal ownership

When both parent stdin and stdout are terminals, ptymark enables raw mode on the parent terminal for
the lifetime of the session. The child PTY retains its own terminal line discipline. This allows
normal shell behavior:

- typed bytes and bracketed paste reach the child;
- the child controls echo and canonical input;
- Ctrl+C and related control bytes are interpreted by the child terminal's foreground process group;
- shell prompts, completion, mouse reports, and full-screen applications continue to use terminal
  protocols;
- raw mode is restored before ptymark exits through the normal command path.

When parent stdin or stdout is redirected, ptymark still creates a real PTY/ConPTY for the child but
does not change the parent terminal mode. This is used by reproducible CI and allows scripted
integration tests without replacing the PTY with a mock.

## Output safety

Only complete explicit Mermaid and block-math forms on safe text lines are eligible for rendering.
The following remain byte-exact:

- ANSI/CSI styling and cursor movement;
- OSC hyperlinks, cwd markers, shell integration, and titles;
- DCS, APC, PM, and unknown string controls;
- carriage-return progress and line-editor redraws;
- alternate-screen applications;
- incomplete, oversized, invalid, or failed semantic blocks.

PTY line endings commonly arrive as CRLF. An exact CRLF pair is treated as a logical safe newline
while preserving both bytes. A bare carriage return remains a redraw control and puts the rest of the
line on the raw bypass path.

Full-screen Codex, fuzzy finders, editors, pagers, and other alternate-screen or cursor-addressed
interfaces are intentionally preserved rather than rewritten. Line-oriented Markdown emitted outside
those protected regions can be rendered.

## Resize behavior

The initial child size comes from the parent terminal when available, otherwise from `LINES`,
`COLUMNS`, and the configured rendering width. While attached to a terminal, ptymark polls the parent
size and calls the native PTY/ConPTY resize API after a change. The updated column count is also used
for semantic blocks completed after the resize.

The current MVP is synchronous: a renderer already executing during a resize is allowed to finish.
Generation-based cancellation of an in-flight stale render remains follow-up work.

## Process and failure behavior

- child stdout and stderr are combined by the PTY, as they are in a normal terminal;
- ordinary child exit codes are returned by ptymark;
- EOF or the platform's closed-PTY indication completes the display pipeline before exit;
- a strict rendering failure terminates the child and returns a ptymark error;
- a non-strict rendering failure restores the exact semantic source;
- display write failure terminates the child rather than continuing invisibly;
- normal rendering never installs dependencies or performs network access.

## Real integration evidence

`tests/interactive_pty_contract.rs` launches real operating-system children and verifies:

- Unix PTY or Windows ConPTY allocation is visible to the child as a terminal;
- real child Markdown reaches the rendering pipeline;
- alternate-screen bytes remain unrendered;
- exit status is preserved;
- on Unix, a Ctrl+C byte reaches the foreground process group;
- a real Unix PTY resize changes the size observed by `stty`.

Managed-renderer smoke tests additionally run Mermaid and MathJax through the interactive PTY/ConPTY
path, so an engine failure cannot be hidden by a mock adapter or source fallback. Session-mode
contracts run through the same real host and verify that source/safe output remains exact, private mode
continues to render, and conflicting options fail before child launch.
