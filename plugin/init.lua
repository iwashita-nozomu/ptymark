local wezterm = require 'wezterm'

local M = {}

local function copy_array(values)
  local result = {}
  for index, value in ipairs(values) do
    result[index] = value
  end
  return result
end

local function resolve_command(options)
  if options.command ~= nil then
    assert(type(options.command) == 'table', 'ptymark option `command` must be an array')
    assert(#options.command > 0, 'ptymark option `command` must not be empty')
    return copy_array(options.command)
  end

  local command = {
    options.binary or 'ptymark',
    '--',
    options.shell or os.getenv('SHELL') or '/bin/sh',
  }

  if options.login_shell ~= false then
    table.insert(command, '-l')
  end

  return command
end

local function spawn_spec(command, options)
  local spec = {
    args = copy_array(command),
  }

  if options.cwd ~= nil then
    spec.cwd = options.cwd
  end

  if options.set_environment_variables ~= nil then
    spec.set_environment_variables = options.set_environment_variables
  end

  return spec
end

function M.command(options)
  return resolve_command(options or {})
end

function M.apply_to_config(config, options)
  local config_type = type(config)
  assert(
    config_type == 'table' or config_type == 'userdata',
    'ptymark requires a WezTerm config builder'
  )

  options = options or {}
  local command = resolve_command(options)
  local label = options.label or 'ptymark shell'

  if options.launch_menu ~= false then
    config.launch_menu = config.launch_menu or {}
    local entry = spawn_spec(command, options)
    entry.label = label
    table.insert(config.launch_menu, entry)
  end

  if options.key ~= false then
    local key = options.key or {
      key = 'P',
      mods = 'CTRL|SHIFT',
    }

    assert(type(key) == 'table', 'ptymark option `key` must be a table or false')
    assert(type(key.key) == 'string', 'ptymark key binding requires `key`')

    config.keys = config.keys or {}
    table.insert(config.keys, {
      key = key.key,
      mods = key.mods or 'NONE',
      action = wezterm.action.SpawnCommandInNewTab(spawn_spec(command, options)),
    })
  end
end

return M
