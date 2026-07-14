# ptymark examples

## WezTerm

[`wezterm.lua`](./wezterm.lua) is a complete minimal `~/.wezterm.lua` for the ptymark launcher plugin on Linux, macOS, WSL, and Windows.

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

When `~/.wezterm.lua` already exists, copy the `wezterm.plugin.require(...)` and
`ptymark.apply_to_config(...)` blocks into the existing file rather than replacing it.

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

GUI applications may not inherit the same environment as an interactive shell. Installer-generated absolute paths avoid renderer PATH ambiguity; explicit `PTYMARK_BINARY` and `PTYMARK_CONFIG` values remain the most predictable launcher setup.

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

`source` keeps semantic detection but displays exact source, `safe` bypasses detection and external
renderers, and `private` keeps rendering while disabling the process-local cache. The plugin only
constructs argv; validation and behavior remain in the native ptymark process.
