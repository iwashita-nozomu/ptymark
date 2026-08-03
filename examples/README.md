# Ptymark examples

<!--
@dependency-start
contract reference
responsibility Routes users to supported configuration, semantic input, and WezTerm examples.
upstream design ../README.md user-facing surface
upstream design ../documents/README.md task-oriented documentation map
upstream design ../documents/ptymark-design.md architecture contract
upstream design ../documents/openmath.md structured math input contract
downstream implementation ../tests/plugin_smoke.lua executable WezTerm example
downstream implementation ../tests/openmath_contract.rs executable OpenMath behavior
@dependency-end
-->

## Choose an example

| Goal | Example |
| --- | --- |
| Start from a validated runtime configuration | [`ptymark.toml`](./ptymark.toml) |
| Select explicit renderer executables | [`external-engines.toml`](./external-engines.toml) |
| Preview structured mathematical input | [`openmath.md`](./openmath.md) |
| Add an append-only WezTerm launcher | [`wezterm.lua`](./wezterm.lua) |

## Semantic input samples

Run the OpenMath sample through the same preview path used for files and streams:

```bash
ptymark preview examples/openmath.md
```

Verify exact source recovery without invoking conversion or an external math engine:

```bash
ptymark preview --source examples/openmath.md
```

The sample includes both standard OpenMath Content Dictionary symbols and a project-specific symbol. The complete parsing, conversion, safety, and non-goal contract is in [`../documents/openmath.md`](../documents/openmath.md).

Mermaid and TeX do not need separate files. Their supported explicit forms are shown in the root [preview guide](../README.md#use-preview).

## Configuration

[`ptymark.toml`](./ptymark.toml) uses the strict schema accepted by `ptymark config check`. [`external-engines.toml`](./external-engines.toml) shows role-by-role executable selection without arbitrary command strings or argument templates.

Validate either file before use:

```bash
ptymark --config examples/ptymark.toml config check
ptymark --config examples/external-engines.toml config check
```

OpenMath shares `profiles.<name>.detection.math` and `profiles.<name>.engines.math`; it does not require another configuration section or executable role.

## WezTerm

[`wezterm.lua`](./wezterm.lua) is a complete minimal `~/.wezterm.lua` for the Ptymark launcher plugin on Linux, macOS, WSL, and Windows.

Run the platform installer first.

Linux, macOS, or WSL:

```bash
bash scripts/installer.sh
```

Windows PowerShell:

```powershell
pwsh -File scripts/installer.ps1
```

Windows cmd.exe:

```bat
scripts\installer.cmd
```

Git Bash, MSYS2, or Cygwin:

```bash
bash scripts/installer.sh
```

The Windows Bash path delegates to the PowerShell installer after converting path-valued arguments to native Windows paths. WSL remains a Linux installation.

For a new WezTerm configuration:

```bash
cp examples/wezterm.lua ~/.wezterm.lua
```

```powershell
Copy-Item examples/wezterm.lua $HOME/.wezterm.lua
```

When `~/.wezterm.lua` already exists, copy the `wezterm.plugin.require(...)` and `ptymark.apply_to_config(...)` blocks into the existing file rather than replacing it.

The example chooses platform defaults:

```text
Linux/macOS/WSL binary  ~/.cargo/bin/ptymark
Windows binary          %USERPROFILE%/.cargo/bin/ptymark.exe
Linux/macOS/WSL config  ~/.config/ptymark/config.toml
Windows config          %APPDATA%/ptymark/config.toml
key                     CTRL|SHIFT+P
menu label              ptymark shell
shell                   $SHELL or /bin/sh; %COMSPEC% on Windows
```

Override the binary or config without editing the example by setting these environment variables before WezTerm starts:

```text
PTYMARK_BINARY
PTYMARK_CONFIG
```

GUI applications may not inherit the same environment as an interactive shell. Managed and installer-discovered renderer paths are resolved from machine-local install state; explicit `PTYMARK_BINARY` and `PTYMARK_CONFIG` values remain the most predictable launcher setup.

For local plugin development, replace the HTTPS plugin URL with an absolute file URL:

```lua
local ptymark = wezterm.plugin.require(
  'file:///absolute/path/to/ptymark'
)
```

The plugin appends to `config.keys` and `config.launch_menu`; it does not replace existing entries.

Choose a mode for only the sessions created by the launcher entry:

```lua
ptymark.apply_to_config(config, {
  mode = 'safe', -- source | safe | private
})
```

`source` keeps semantic detection but displays exact source, `safe` bypasses detection and external renderers, and `private` keeps rendering while disabling the process-local cache. The plugin only constructs argv; validation and behavior remain in the native Ptymark process.
