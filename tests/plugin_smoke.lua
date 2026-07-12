local ptymark_module

local wezterm_stub = {
  home_dir = '/home/user',
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

local example = dofile('examples/wezterm.lua')
os.getenv = original_getenv

assert(#example.launch_menu == 1)
assert(example.launch_menu[1].label == 'ptymark shell')
assert(example.launch_menu[1].cwd == '/home/user')
assert(example.launch_menu[1].args[1] == '/home/user/.cargo/bin/ptymark')
assert(example.launch_menu[1].args[2] == '--config')
assert(example.launch_menu[1].args[3] == '/home/user/.config/ptymark/config.toml')
assert(example.launch_menu[1].args[4] == '--')
assert(example.launch_menu[1].args[5] == '/bin/zsh')
assert(example.launch_menu[1].args[6] == '-l')
assert(#example.keys == 1)
assert(example.keys[1].key == 'P')
assert(example.keys[1].mods == 'CTRL|SHIFT')
assert(example.keys[1].action.kind == 'SpawnCommandInNewTab')

print('ptymark WezTerm plugin and example smoke: ok')
