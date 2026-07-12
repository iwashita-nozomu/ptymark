package.preload['wezterm'] = function()
  return {
    action = {
      SpawnCommandInNewTab = function(spec)
        return { kind = 'SpawnCommandInNewTab', spec = spec }
      end,
    },
  }
end

local ptymark = dofile('plugin/init.lua')

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

print('ptymark WezTerm plugin smoke: ok')
