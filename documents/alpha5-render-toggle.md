<!--
@dependency-start
contract design
responsibility Defines the Alpha.5 session-local rendering toggle and its input, display, and WezTerm boundaries.
upstream design ./interactive-session.md native PTY and ConPTY ownership
upstream design ./ptymark-design.md terminal-safety and exact-source invariants
downstream implementation ../src/native_session.rs exact input recognition and session state
downstream implementation ../src/pipeline.rs pause, source restoration, and safe resume
downstream implementation ../plugin/init.lua append-only WezTerm binding
downstream implementation ../tests/render_toggle_contract.rs real PTY and ConPTY evidence
@dependency-end
-->

# Alpha.5 session-local rendering toggle

## User contract

Inside `ptymark shell -- COMMAND`, one key can pause or resume semantic replacement without restarting the child or changing persistent configuration. Each session starts with the baseline mode enabled and discards the temporary state on exit.

The toggle only suppresses rendering. It cannot make `--source` or `--safe` render, cannot enable cache in `--private`, and does not modify TOML, install state, shell profiles, WezTerm files, child argv, or the child environment beyond the existing `PTYMARK_ACTIVE=1` marker.

## Input contract

The only reserved input value is supplementary private-use scalar `U+10FFFD`:

```text
Unicode scalar   U+10FFFD
UTF-8 bytes      f4 8f bf bd
```

The native input pump consumes an exact four-byte match and toggles one process-local atomic state. Every split across input reads is recognized. Every partial or mismatching prefix is forwarded to the child byte-for-byte and in order. Ordinary Escape is forwarded immediately because `0x1b` is not a prefix.

The reserved scalar itself is intentionally unavailable as child input through a Ptymark-hosted session. Other keyboard bytes, Ctrl+C, bracketed-paste framing/content, mouse reports, and terminal escape sequences remain on the existing forwarding path.

## Display contract

The display pipeline reads the requested state before each bounded output update:

- disabling flushes detector-retained partial content as exact source before passthrough;
- terminal-control classification remains active in both states;
- an already executing renderer may finish;
- enabling at a clean logical-line boundary resumes detection immediately;
- enabling mid-line keeps that remainder as passthrough through the next newline, so a delimiter cannot be assembled across the transition.

No status line is injected into child output. The native session is the single source of truth, so Lua-side state cannot drift from the actual renderer state.

## WezTerm integration

The plugin appends this default binding:

```lua
render_toggle_key = {
  key = 'R',
  mods = 'CTRL|SHIFT|ALT',
}
```

It uses `wezterm.action.SendString` to send `U+10FFFD` to the active pane. Existing `config.keys` and `config.launch_menu` entries are retained. A custom key table is accepted, and `render_toggle_key = false` omits the binding.

Because the binding targets the active pane, use it inside a Ptymark-hosted pane. Outside Ptymark the reserved scalar would be delivered to the foreground application.

## Verification

Alpha.5 requires:

- unit coverage for exact match, every split point, every mismatch position, every EOF prefix, immediate Escape, order, and repeated toggles;
- pipeline coverage for partial-source restoration, disabled passthrough, raw-terminal newline recovery, safe resume, and mid-line protection;
- real Unix PTY and Windows ConPTY coverage proving one rendered block followed by one exact-source block after the toggle;
- executable WezTerm smoke coverage for default, custom, disabled, invalid, and append-only key behavior;
- unchanged terminal-safety, child-argv, resize, signal, exit-status, privacy, and source-only release gates.
