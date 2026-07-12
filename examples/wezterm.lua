-- Minimal ~/.wezterm.lua example for ptymark on Linux, macOS, and Windows.
--
-- Install first:
--   Linux/macOS: bash scripts/install.sh
--   Windows:     pwsh -File scripts/install.ps1
--
-- PTYMARK_BINARY and PTYMARK_CONFIG override the platform defaults below.

local wezterm = require 'wezterm'
local config = wezterm.config_builder()

local home = wezterm.home_dir
local target = wezterm.target_triple or ''
local is_windows = target:find('windows', 1, true) ~= nil

local default_binary
local default_config
local default_shell
local login_shell

if is_windows then
  local appdata = os.getenv 'APPDATA' or (home .. '/AppData/Roaming')
  default_binary = home .. '/.cargo/bin/ptymark.exe'
  default_config = appdata .. '/ptymark/config.toml'
  default_shell = os.getenv 'COMSPEC' or 'cmd.exe'
  login_shell = false
else
  default_binary = home .. '/.cargo/bin/ptymark'
  default_config = home .. '/.config/ptymark/config.toml'
  default_shell = os.getenv 'SHELL' or '/bin/sh'
  login_shell = true
end

local ptymark_binary = os.getenv 'PTYMARK_BINARY' or default_binary
local ptymark_config = os.getenv 'PTYMARK_CONFIG' or default_config

-- WezTerm accepts both HTTPS plugin URLs and file:// URLs. For local plugin
-- development, replace this URL with an absolute file URL such as:
-- file:///home/user/src/ptymark
local ptymark = wezterm.plugin.require(
  'https://github.com/iwashita-nozomu/ptymark'
)

ptymark.apply_to_config(config, {
  binary = ptymark_binary,
  config_file = ptymark_config,
  shell = default_shell,
  login_shell = login_shell,
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
