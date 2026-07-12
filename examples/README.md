# ptymark examples

## WezTerm

[`wezterm.lua`](./wezterm.lua) is a complete minimal `~/.wezterm.lua` for the ptymark launcher plugin.

Run the installer first:

```bash
bash scripts/install.sh
```

For a new WezTerm configuration:

```bash
cp examples/wezterm.lua ~/.wezterm.lua
```

When `~/.wezterm.lua` already exists, copy the `wezterm.plugin.require(...)` and
`ptymark.apply_to_config(...)` blocks into the existing file rather than replacing it.

The example uses these defaults:

```text
binary       ~/.cargo/bin/ptymark
config file  ~/.config/ptymark/config.toml
key          CTRL|SHIFT+P
menu label   ptymark shell
shell        $SHELL, falling back to /bin/sh
```

Override the two paths without editing the example by setting environment variables before WezTerm
starts:

```text
PTYMARK_BINARY
PTYMARK_CONFIG
```

GUI applications may not inherit the same environment as an interactive shell. Editing the example
to use explicit absolute paths is therefore the most predictable setup on macOS and desktop Linux.

For local plugin development, replace the HTTPS plugin URL with an absolute file URL:

```lua
local ptymark = wezterm.plugin.require(
  'file:///absolute/path/to/ptymark'
)
```

The plugin appends to `config.keys` and `config.launch_menu`; it does not replace existing entries.
