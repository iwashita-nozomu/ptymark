<!--
@dependency-start
contract design
responsibility Defines the native PTY/ConPTY interactive session used by `ptymark -- COMMAND`.
upstream implementation ../src/interactive.rs owns PTY allocation, raw mode, input forwarding, resize, output filtering, and child lifecycle.
upstream design ./ptymark-design.md defines terminal-safety and pre-display rendering invariants.
downstream test ../tests/interactive_pty_contract.rs exercises real Unix PTY and Windows ConPTY processes.
@dependency-end
-->

# Interactive PTY and ConPTY session

## User-facing command

```text
ptymark [--config PATH] -- COMMAND [ARG...]
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
path, so an engine failure cannot be hidden by a mock adapter or source fallback.
