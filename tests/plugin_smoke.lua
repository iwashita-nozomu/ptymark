local ptymark_module

local wezterm_stub = {
  home_dir = '/home/user',
  target_triple = 'x86_64-unknown-linux-gnu',
  action = {
    SpawnCommandInNewTab = function(spec)
      return { kind = 'SpawnCommandInNewTab', spec = spec }
    end,
  },
  config_builder = function()
    return {}
  end,
  plugin = {
    require = function(url)
      assert(url == 'https://github.com/iwashita-nozomu/ptymark')
      assert(ptymark_module ~= nil)
      return ptymark_module
    end,
  },
}

package.preload['wezterm'] = function()
  return wezterm_stub
end

ptymark_module = dofile('plugin/init.lua')
local ptymark = ptymark_module

local command = ptymark.command({
  binary = '/usr/local/bin/ptymark',
  config_file = '/home/user/.config/ptymark/config.toml',
  shell = '/bin/zsh',
})
assert(command[1] == '/usr/local/bin/ptymark')
assert(command[2] == '--config')
assert(command[3] == '/home/user/.config/ptymark/config.toml')
assert(command[4] == '--')
assert(command[5] == '/bin/zsh')
assert(command[6] == '-l')

for _, mode in ipairs({ 'source', 'safe', 'private' }) do
  local mode_command = ptymark.command({
    binary = 'ptymark',
    mode = mode,
    shell = '/bin/sh',
    login_shell = false,
  })
  assert(mode_command[1] == 'ptymark')
  assert(mode_command[2] == '--' .. mode)
  assert(mode_command[3] == '--')
  assert(mode_command[4] == '/bin/sh')
end

local valid_mode, mode_error = pcall(function()
  ptymark.command({ mode = 'unknown' })
end)
assert(not valid_mode)
assert(tostring(mode_error):find('source, safe, private', 1, true) ~= nil)

local config = {
  launch_menu = { { label = 'existing', args = { 'sh' } } },
  keys = { { key = 'E', mods = 'CTRL' } },
}
ptymark.apply_to_config(config, {
  binary = 'ptymark',
  shell = '/bin/fish',
  cwd = '/workspace',
  label = 'Rendered shell',
  key = { key = 'R', mods = 'CTRL|SHIFT' },
})

assert(#config.launch_menu == 2)
assert(config.launch_menu[1].label == 'existing')
assert(config.launch_menu[2].label == 'Rendered shell')
assert(config.launch_menu[2].cwd == '/workspace')
assert(config.launch_menu[2].args[1] == 'ptymark')
assert(config.launch_menu[2].args[2] == '--')
assert(config.launch_menu[2].args[3] == '/bin/fish')

assert(#config.keys == 2)
assert(config.keys[1].key == 'E')
assert(config.keys[2].key == 'R')
assert(config.keys[2].action.kind == 'SpawnCommandInNewTab')

local no_side_effects = {}
ptymark.apply_to_config(no_side_effects, {
  command = { 'ptymark', 'preview', '--source', '-' },
  launch_menu = false,
  key = false,
})
assert(no_side_effects.launch_menu == nil)
assert(no_side_effects.keys == nil)

local original_getenv = os.getenv
os.getenv = function(name)
  if name == 'PTYMARK_BINARY' then
    return '/home/user/.cargo/bin/ptymark'
  end
  if name == 'PTYMARK_CONFIG' then
    return '/home/user/.config/ptymark/config.toml'
  end
  if name == 'SHELL' then
    return '/bin/zsh'
  end
  return original_getenv(name)
end

local unix_example = dofile('examples/wezterm.lua')
os.getenv = original_getenv

assert(#unix_example.launch_menu == 1)
assert(unix_example.launch_menu[1].label == 'ptymark shell')
assert(unix_example.launch_menu[1].cwd == '/home/user')
assert(unix_example.launch_menu[1].args[1] == '/home/user/.cargo/bin/ptymark')
assert(unix_example.launch_menu[1].args[2] == '--config')
assert(unix_example.launch_menu[1].args[3] == '/home/user/.config/ptymark/config.toml')
assert(unix_example.launch_menu[1].args[4] == '--')
assert(unix_example.launch_menu[1].args[5] == '/bin/zsh')
assert(unix_example.launch_menu[1].args[6] == '-l')
assert(#unix_example.keys == 1)
assert(unix_example.keys[1].key == 'P')
assert(unix_example.keys[1].mods == 'CTRL|SHIFT')
assert(unix_example.keys[1].action.kind == 'SpawnCommandInNewTab')

wezterm_stub.home_dir = 'C:/Users/user'
wezterm_stub.target_triple = 'x86_64-pc-windows-msvc'
os.getenv = function(name)
  if name == 'APPDATA' then
    return 'C:/Users/user/AppData/Roaming'
  end
  if name == 'COMSPEC' then
    return 'C:/Windows/System32/cmd.exe'
  end
  return nil
end

local windows_example = dofile('examples/wezterm.lua')
os.getenv = original_getenv

assert(windows_example.launch_menu[1].args[1] == 'C:/Users/user/.cargo/bin/ptymark.exe')
assert(windows_example.launch_menu[1].args[2] == '--config')
assert(windows_example.launch_menu[1].args[3] == 'C:/Users/user/AppData/Roaming/ptymark/config.toml')
assert(windows_example.launch_menu[1].args[4] == '--')
assert(windows_example.launch_menu[1].args[5] == 'C:/Windows/System32/cmd.exe')
assert(windows_example.launch_menu[1].args[6] == nil)

print('ptymark WezTerm plugin and platform examples smoke: ok')
