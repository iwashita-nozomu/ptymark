-- Minimal ~/.wezterm.lua example for ptymark.
--
-- Run `bash scripts/install.sh` first. The installer writes the default ptymark
-- configuration to ~/.config/ptymark/config.toml and normally installs the
-- binary to ~/.cargo/bin/ptymark. Override either path below when using a
-- custom Cargo root or config location.

local wezterm = require 'wezterm'
local config = wezterm.config_builder()

local home = wezterm.home_dir
local ptymark_binary = os.getenv 'PTYMARK_BINARY'
  or (home .. '/.cargo/bin/ptymark')
local ptymark_config = os.getenv 'PTYMARK_CONFIG'
  or (home .. '/.config/ptymark/config.toml')
local shell = os.getenv 'SHELL' or '/bin/sh'

-- WezTerm accepts both HTTPS plugin URLs and file:// URLs. For local plugin
-- development, replace this URL with an absolute file URL such as:
-- file:///home/user/src/ptymark
local ptymark = wezterm.plugin.require(
  'https://github.com/iwashita-nozomu/ptymark'
)

ptymark.apply_to_config(config, {
  binary = ptymark_binary,
  config_file = ptymark_config,
  shell = shell,
  login_shell = true,
  cwd = home,
  label = 'ptymark shell',
  key = {
    key = 'P',
    mods = 'CTRL|SHIFT',
  },
})

-- Add normal WezTerm options here. The plugin appends to existing `keys` and
-- `launch_menu` entries instead of replacing them.
-- config.color_scheme = 'Builtin Solarized Dark'
-- config.font_size = 13.0

return config
