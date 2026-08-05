<!--
@dependency-start
contract design
responsibility Defines native PTY and ConPTY session behavior.
upstream design ./ptymark-design.md terminal-safety boundary
upstream design ./openmath.md structured math input contract
downstream implementation ../src/interactive.rs session composition
downstream implementation ../src/native_session.rs child environment contract
downstream implementation ../tests/interactive_pty_contract.rs native runtime validation
downstream implementation ../tests/openmath_contract.rs structured math safety evidence
@dependency-end
-->

# Interactive PTY and ConPTY session

## User-facing command

```text
ptymark [--config PATH] [--source|--safe] [--private] [--allow-nested] -- COMMAND [ARG...]
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
    -> source-format adapter when required
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

The interactive host is retained in every rendering mode; only pre-display rendering policy changes:

- `--source` keeps semantic detection active and replaces each complete block with its exact source;
- `--safe` uses the passthrough detector and never invokes a semantic renderer, source-format adapter, or presenter;
- `--private` keeps the selected rendering policy but selects `NoopCache` for the invocation;
- `--allow-nested` is a development/debug escape hatch that permits a session to start when its parent already has `PTYMARK_ACTIVE=1`.

`--source` and `--safe` are mutually exclusive. `--private` can accompany either mode. All options are
resolved before configuration is loaded, the child is spawned, or parent terminal raw mode is entered.
They are immutable for the lifetime of the session and never change keyboard forwarding, resizing,
signal behavior, child argv, or child exit status.

The current runtime has only process-local memory caching and no persistent source-bearing diagnostic
sink. `--private` disables that cache now and owns the forward-compatible contract for suppressing any
future persistent diagnostics without changing the CLI.


## Temporary rendering toggle

An active interactive session has one in-memory rendering gate. It starts enabled relative to the selected baseline mode and is discarded when the session exits. The gate can pause semantic replacement, but it does not rewrite TOML, installation state, shell profiles, WezTerm configuration, child argv, cache policy, or the child process. In particular, toggling cannot make `--source` or `--safe` render, and `--private` remains cache-disabled.

The WezTerm plugin appends a `render_toggle_key` binding, defaulting to `CTRL|SHIFT|ALT+R`. The binding sends supplementary private-use scalar `U+10FFFD`, whose UTF-8 bytes are `f4 8f bf bd`. The native input pump consumes only that exact four-byte value. Every split across input reads is recognized; partial or mismatching prefixes are forwarded to the child byte-for-byte and in order. Escape is not a prefix, so ordinary Escape, Ctrl+C, bracketed-paste framing, mouse reports, and normal terminal escape sequences are not delayed. The reserved scalar itself is the sole exception and should not be pasted as application data through a Ptymark-hosted pane.

Disabling applies at the next display update. A renderer already executing may finish. Any partially detected semantic block is restored as exact source before passthrough begins. Terminal-control classification remains active while paused. Re-enabling in the middle of a logical line keeps that remainder on the passthrough path through the next newline, preventing a false opener assembled across the state transition.

The plugin is append-only: existing `config.keys` and `config.launch_menu` entries are retained. Set `render_toggle_key = false` to omit the binding or provide another `{ key = ..., mods = ... }` table. Because `SendString` targets the active pane, use the binding in a Ptymark-hosted pane. Ptymark intentionally does not inject a status line or persist Lua-side state; the native session remains the single source of truth.

## Session visibility and nesting

Every interactive child receives one stable public marker:

```text
PTYMARK_ACTIVE=1
```

Ptymark deliberately does not expose a public nesting depth, parent PID, or other unstable process
metadata. A command can determine whether it is inside Ptymark by reading `PTYMARK_ACTIVE`; no process-tree inspection is required.

Starting another interactive Ptymark session while that marker is already `1` is rejected before
configuration loading or child launch:

```text
ptymark: already running inside Ptymark.
Exit the current session first, or pass `--allow-nested` for development and debugging.
```

Intentional nesting remains available through `--allow-nested`, but each nested launch must opt in
explicitly. This keeps accidental chains of PTY proxies out of the normal user path.

Ptymark never edits `.bashrc`, `.zshrc`, Fish configuration, Nushell configuration, a PowerShell
profile, or a prompt-framework configuration. A concise prompt marker is opt-in. For Bash:

```bash
if [[ ${PTYMARK_ACTIVE:-0} == 1 ]]; then
  PS1="[ptymark] $PS1"
fi
```

For Zsh:

```zsh
if [[ ${PTYMARK_ACTIVE:-0} == 1 ]]; then
  PROMPT="[ptymark] $PROMPT"
fi
```

For PowerShell profiles that use the built-in `prompt` function:

```powershell
if ($env:PTYMARK_ACTIVE -eq '1') {
    $script:PtymarkBasePrompt = $function:prompt
    function global:prompt {
        '[ptymark] ' + (& $script:PtymarkBasePrompt)
    }
}
```

Fish, Nushell, Starship, Oh My Posh, and other prompt frameworks can use the same marker in their own
conditional syntax. The relevant predicates are `test "$PTYMARK_ACTIVE" = 1` in Fish,
`$env.PTYMARK_ACTIVE -eq '1'` in PowerShell, and the framework's environment-variable condition for
`PTYMARK_ACTIVE=1` elsewhere.

## Responsibility split

The runtime is intentionally split at stable ownership boundaries:

```text
interactive.rs
    command-level orchestration and failure precedence
    active-session and nesting policy

native_session.rs
    parent terminal state
    native PTY / ConPTY child lifecycle
    child environment marker
    input forwarding and exact render-toggle recognition
    session-local rendering state
    resize observation

runtime.rs
    detector / source-format adapter / renderer / cache composition

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

- typed bytes and bracketed paste reach the child, except the explicitly reserved `U+10FFFD` toggle value;
- the child controls echo and canonical input;
- Ctrl+C and related control bytes are interpreted by the child terminal's foreground process group;
- shell prompts, completion, mouse reports, and full-screen applications continue to use terminal
  protocols;
- raw mode is restored before ptymark exits through the normal command path.

When parent stdin or stdout is redirected, ptymark still creates a real PTY/ConPTY for the child but
does not change the parent terminal mode. This is used by reproducible CI and allows scripted
integration tests without replacing the PTY with a mock.

## Output safety

Only complete explicit Mermaid, TeX block-math, and OpenMath forms on safe text lines are eligible for rendering. OpenMath uses the same `math` role and is converted locally before the configured math renderer. The following remain byte-exact:

- ANSI/CSI styling and cursor movement;
- OSC hyperlinks, cwd markers, shell integration, and titles;
- DCS, APC, PM, and unknown string controls;
- carriage-return progress and line-editor redraws;
- alternate-screen applications;
- incomplete, oversized, invalid, unsupported, or failed semantic blocks.

PTY line endings commonly arrive as CRLF. An exact CRLF pair is treated as a logical safe newline
while preserving both bytes. A bare carriage return remains a redraw control and puts the rest of the
line on the raw bypass path.

The interactive display path converts lone LF bytes produced by a successful renderer or presenter to
CRLF before writing them to the parent terminal. Existing CRLF pairs are retained. Child passthrough,
raw terminal-control bytes, and exact-source fallback remain byte-for-byte unchanged. This prevents a
raw parent terminal from advancing to the next row while retaining the previous cursor column.

An eligible semantic block must begin on a clean logical line. If a prompt or shell integration emits
control bytes on the same logical line, the safety gate preserves that line as raw terminal output.
Emit a leading newline before the opening delimiter when producing a block directly from an
interactive prompt.

Full-screen Codex, fuzzy finders, editors, pagers, and other alternate-screen or cursor-addressed
interfaces are intentionally preserved rather than rewritten. Line-oriented semantic blocks emitted outside
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
- a strict conversion or rendering failure terminates the child and returns a ptymark error;
- a non-strict conversion or rendering failure restores the exact semantic source;
- display write failure terminates the child rather than continuing invisibly;
- normal rendering never installs dependencies or performs network access.

## Real integration evidence

`tests/interactive_pty_contract.rs` launches real operating-system children and verifies:

- Unix PTY or Windows ConPTY allocation is visible to the child as a terminal;
- real child Markdown reaches the rendering pipeline and rendered rows use terminal-safe CRLF;
- `PTYMARK_ACTIVE=1` reaches the child on Unix and Windows;
- accidental nesting is rejected before launch and `--allow-nested` is an explicit escape hatch;
- alternate-screen bytes remain unrendered;
- a real session renders one block, consumes the toggle value, and restores the next block as exact source;
- exit status is preserved;
- on Unix, a Ctrl+C byte reaches the foreground process group;
- a real Unix PTY resize changes the size observed by `stty`.

Managed-renderer smoke tests additionally run Mermaid and MathJax through the interactive PTY/ConPTY
path, so an engine failure cannot be hidden by a mock adapter or source fallback. OpenMath contracts exercise the same canonical pipeline, including chunk independence, exact fallback, strict failure, source/safe modes, and protected terminal regions. Session-mode contracts verify that source/safe output remains exact, private mode continues to render, and conflicting options fail before child launch.
