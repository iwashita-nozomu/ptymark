<!--
@dependency-start
contract design
responsibility Defines the non-interactive child-stdout filtering contract for `ptymark run -- COMMAND`.
upstream implementation ../src/filtered_run.rs owns parsing, child process execution, and display-pipeline composition.
upstream design ./ptymark-design.md defines terminal-safety, semantic detection, rendering, and display-commit invariants.
downstream test ../tests/filtered_run_contract.rs verifies rendering, help, and exit-status preservation.
@dependency-end
-->

# Filtered command execution

## Purpose

`ptymark run -- COMMAND [ARG...]` is the command-execution path for batch tools and other
non-interactive programs whose standard output may contain complete Mermaid or block-math fences.
It complements the native PTY/ConPTY path with a simpler pipe-oriented mode for commands that do
not need terminal attachment.

```text
child stdin  <---------------- inherited terminal or pipe
child stderr ----------------> inherited terminal or pipe
child stdout -> TerminalOutputGate -> SemanticDetector -> renderer -> parent stdout
```

## Contract

- only child **stdout** enters the pre-display pipeline;
- child stdin and stderr are inherited directly;
- the child's arguments are passed to `Command` as separate values, never through a shell string;
- the child exit code is returned after buffered output is committed;
- ANSI, OSC, DCS/APC/PM, carriage-return output, and alternate-screen sequences retain the existing
  byte-exact safety behavior;
- non-strict renderer failure restores exact source;
- strict renderer failure terminates the child and returns a ptymark error;
- stdout is a pipe, not a PTY.

The last rule is intentional. Interactive shells, prompts, line editors, and full-screen TUIs may
change behavior when stdout is not a terminal. They use `ptymark -- COMMAND`, which allocates the
native Unix PTY or Windows ConPTY host.

## Usage

```bash
ptymark run -- command-that-prints-markdown
ptymark run --no-cache -- command
ptymark run --columns 100 -- command
ptymark run --strict -- command
ptymark --config /absolute/path/config.toml run -- command
```

The options mirror the stream controls available to `preview`:

```text
--source
--strict
--no-cache
--color
--columns N
--config PATH
```

The `--` separator is mandatory so child arguments can begin with `-` without being interpreted as
ptymark options.

## Deliberate boundary

This command intentionally does not allocate a PTY or ConPTY, change parent terminal mode, propagate
window size, or claim interactive prompt and TUI behavior. Those responsibilities belong to the
implemented `ptymark -- COMMAND` native-session path. Keeping the two modes separate makes pipe
semantics explicit and avoids guessing whether a command expects a terminal.
