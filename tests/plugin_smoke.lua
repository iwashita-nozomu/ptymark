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
  binary = '/opt/ptymark/bin/ptymark',
  shell = '/bin/zsh',
})
assert(#command == 4)
assert(command[1] == '/opt/ptymark/bin/ptymark')
assert(command[2] == '--')
assert(command[3] == '/bin/zsh')
assert(command[4] == '-l')

local configured = ptymark.command({
  binary = '/opt/ptymark/bin/ptymark',
  config_file = '/home/user/.config/ptymark/config.toml',
  profile = 'interactive',
  private = true,
  shell = '/bin/zsh',
})
assert(#configured == 9)
assert(configured[1] == '/opt/ptymark/bin/ptymark')
assert(configured[2] == '--config')
assert(configured[3] == '/home/user/.config/ptymark/config.toml')
assert(configured[4] == '--profile')
assert(configured[5] == 'interactive')
assert(configured[6] == '--private')
assert(configured[7] == '--')
assert(configured[8] == '/bin/zsh')
assert(configured[9] == '-l')

local explicit = ptymark.command({
  command = { 'ptymark', 'preview', '--source', '-' },
})
assert(#explicit == 4)
assert(explicit[2] == 'preview')

local explicit_with_helpers_ok = pcall(function()
  ptymark.command({
    command = { 'ptymark', '--', 'zsh' },
    profile = 'interactive',
  })
end)
assert(not explicit_with_helpers_ok)

local config = {
  launch_menu = {
    { label = 'existing', args = { 'sh' } },
  },
  keys = {
    { key = 'E', mods = 'CTRL' },
  },
}

ptymark.apply_to_config(config, {
  binary = '/usr/local/bin/ptymark',
  shell = '/bin/fish',
  profile = 'private',
  cwd = '/workspace',
  set_environment_variables = { PTYMARK_LOG = 'debug' },
  label = 'Rendered shell',
  key = { key = 'R', mods = 'CTRL|SHIFT' },
})

assert(#config.launch_menu == 2)
assert(config.launch_menu[1].label == 'existing')
assert(config.launch_menu[2].label == 'Rendered shell')
assert(config.launch_menu[2].args[1] == '/usr/local/bin/ptymark')
assert(config.launch_menu[2].args[2] == '--profile')
assert(config.launch_menu[2].args[3] == 'private')
assert(config.launch_menu[2].args[4] == '--')
assert(config.launch_menu[2].args[5] == '/bin/fish')
assert(config.launch_menu[2].cwd == '/workspace')
assert(config.launch_menu[2].set_environment_variables.PTYMARK_LOG == 'debug')

assert(#config.keys == 2)
assert(config.keys[1].key == 'E')
assert(config.keys[2].key == 'R')
assert(config.keys[2].mods == 'CTRL|SHIFT')
assert(config.keys[2].action.kind == 'SpawnCommandInNewTab')
assert(config.keys[2].action.spec.args[1] == '/usr/local/bin/ptymark')
assert(config.keys[2].action.spec.args[2] == '--profile')

local no_side_effects = {}
ptymark.apply_to_config(no_side_effects, {
  command = { 'ptymark', 'demo' },
  launch_menu = false,
  key = false,
})
assert(no_side_effects.launch_menu == nil)
assert(no_side_effects.keys == nil)

print('ptymark WezTerm plugin smoke: ok')
