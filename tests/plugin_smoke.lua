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

local explicit = ptymark.command({
  command = { 'ptymark', 'preview', '--source', '-' },
})
assert(#explicit == 4)
assert(explicit[2] == 'preview')

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
  cwd = '/workspace',
  set_environment_variables = { PTYMARK_LOG = 'debug' },
  label = 'Rendered shell',
  key = { key = 'R', mods = 'CTRL|SHIFT' },
})

assert(#config.launch_menu == 2)
assert(config.launch_menu[1].label == 'existing')
assert(config.launch_menu[2].label == 'Rendered shell')
assert(config.launch_menu[2].args[1] == '/usr/local/bin/ptymark')
assert(config.launch_menu[2].args[3] == '/bin/fish')
assert(config.launch_menu[2].cwd == '/workspace')
assert(config.launch_menu[2].set_environment_variables.PTYMARK_LOG == 'debug')

assert(#config.keys == 2)
assert(config.keys[1].key == 'E')
assert(config.keys[2].key == 'R')
assert(config.keys[2].mods == 'CTRL|SHIFT')
assert(config.keys[2].action.kind == 'SpawnCommandInNewTab')
assert(config.keys[2].action.spec.args[1] == '/usr/local/bin/ptymark')

local no_side_effects = {}
ptymark.apply_to_config(no_side_effects, {
  command = { 'ptymark', 'demo' },
  launch_menu = false,
  key = false,
})
assert(no_side_effects.launch_menu == nil)
assert(no_side_effects.keys == nil)

print('ptymark WezTerm plugin smoke: ok')
