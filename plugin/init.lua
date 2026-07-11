local wezterm = require 'wezterm'

local M = {}

local function copy_array(values)
  local result = {}
  for index, value in ipairs(values) do
    result[index] = value
  end
  return result
end

local function append_config_options(command, options)
  if options.config_file ~= nil then
    assert(type(options.config_file) == 'string', 'ptymark option `config_file` must be a string')
    assert(options.config_file ~= '', 'ptymark option `config_file` must not be empty')
    table.insert(command, '--config')
    table.insert(command, options.config_file)
  end

  if options.profile ~= nil then
    assert(type(options.profile) == 'string', 'ptymark option `profile` must be a string')
    assert(options.profile ~= '', 'ptymark option `profile` must not be empty')
    table.insert(command, '--profile')
    table.insert(command, options.profile)
  end

  if options.no_config == true then
    table.insert(command, '--no-config')
  end
  if options.private == true then
    table.insert(command, '--private')
  end
end

local function resolve_command(options)
  if options.command ~= nil then
    assert(type(options.command) == 'table', 'ptymark option `command` must be an array')
    assert(#options.command > 0, 'ptymark option `command` must not be empty')
    assert(
      options.config_file == nil
        and options.profile == nil
        and options.no_config == nil
        and options.private == nil,
      'ptymark config helper options cannot be combined with an explicit `command`; put selectors in the command array'
    )
    return copy_array(options.command)
  end

  local command = {
    options.binary or 'ptymark',
  }
  append_config_options(command, options)
  table.insert(command, '--')
  table.insert(command, options.shell or os.getenv('SHELL') or '/bin/sh')

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
