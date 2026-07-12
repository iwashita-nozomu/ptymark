# ptymark examples

## WezTerm

[`wezterm.lua`](./wezterm.lua) is a complete minimal `~/.wezterm.lua` for the ptymark launcher plugin on Linux, macOS, and Windows.

Run the platform installer first:

```bash
# Linux or macOS
bash scripts/install.sh
```

```powershell
# Windows PowerShell 7+
pwsh -File scripts/install.ps1
```

For a new WezTerm configuration:

```bash
cp examples/wezterm.lua ~/.wezterm.lua
```

```powershell
Copy-Item examples/wezterm.lua $HOME/.wezterm.lua
```

When `~/.wezterm.lua` already exists, copy the `wezterm.plugin.require(...)` and
`ptymark.apply_to_config(...)` blocks into the existing file rather than replacing it.

The example chooses platform defaults:

```text
Linux/macOS binary  ~/.cargo/bin/ptymark
Windows binary      %USERPROFILE%/.cargo/bin/ptymark.exe
Linux/macOS config  ~/.config/ptymark/config.toml
Windows config      %APPDATA%/ptymark/config.toml
key                 CTRL|SHIFT+P
menu label          ptymark shell
shell               $SHELL or /bin/sh; %COMSPEC% on Windows
```

Override the binary or config without editing the example by setting these environment variables before WezTerm starts:

```text
PTYMARK_BINARY
PTYMARK_CONFIG
```

GUI applications may not inherit the same environment as an interactive shell. Editing the example to use explicit absolute paths is therefore the most predictable setup on macOS, desktop Linux, and Windows GUI launches.

For local plugin development, replace the HTTPS plugin URL with an absolute file URL:

```lua
local ptymark = wezterm.plugin.require(
  'file:///absolute/path/to/ptymark'
)
```

The plugin appends to `config.keys` and `config.launch_menu`; it does not replace existing entries.
