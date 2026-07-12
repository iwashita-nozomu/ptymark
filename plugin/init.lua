-- @dependency-start
-- contract implementation
-- responsibility Adds a thin WezTerm launcher and passes only ptymark config selectors to the native process.
-- upstream design ../documents/configuration.md defines the Rust-owned configuration schema and precedence.
-- upstream design ../documents/usage.md documents WezTerm integration.
-- downstream test ../tests/plugin_smoke.lua verifies command, environment, and append-only config behavior.
-- @dependency-end
local wezterm = require 'wezterm'

local M = {}

local function copy_array(values)
  local result = {}
  for index, value in ipairs(values) do
    result[index] = value
  end
  return result
end

local function copy_map(values)
  local result = {}
  if values == nil then
    return result
  end
  for key, value in pairs(values) do
    result[key] = value
  end
  return result
end

local function has_config_helpers(options)
  return options.config_file ~= nil
    or options.profile ~= nil
    or options.no_config ~= nil
    or options.private ~= nil
end

local function validate_config_helpers(options)
  if options.config_file ~= nil then
    assert(type(options.config_file) == 'string', 'ptymark option `config_file` must be a string')
    assert(options.config_file ~= '', 'ptymark option `config_file` must not be empty')
  end

  if options.profile ~= nil then
    assert(type(options.profile) == 'string', 'ptymark option `profile` must be a string')
    assert(options.profile ~= '', 'ptymark option `profile` must not be empty')
  end

  assert(
    not (options.private == true and options.profile ~= nil),
    'ptymark options `private` and `profile` cannot be combined; use profile = "private"'
  )
end

local function resolve_command(options)
  validate_config_helpers(options)

  if options.command ~= nil then
    assert(type(options.command) == 'table', 'ptymark option `command` must be an array')
    assert(#options.command > 0, 'ptymark option `command` must not be empty')
    assert(
      not has_config_helpers(options),
      'ptymark config helper options cannot be combined with an explicit `command`; put selectors in the command or environment explicitly'
    )
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

local function session_environment(options)
  validate_config_helpers(options)
  local environment = copy_map(options.set_environment_variables)

  if options.config_file ~= nil then
    environment.PTYMARK_CONFIG = options.config_file
  end
  if options.profile ~= nil then
    environment.PTYMARK_PROFILE = options.profile
  elseif options.private == true then
    environment.PTYMARK_PROFILE = 'private'
  end
  if options.no_config == true then
    environment.PTYMARK_NO_CONFIG = '1'
  end

  return environment
end

local function spawn_spec(command, options)
  local spec = {
    args = copy_array(command),
  }

  if options.cwd ~= nil then
    assert(type(options.cwd) == 'string', 'ptymark option `cwd` must be a string')
    spec.cwd = options.cwd
  end

  local environment = session_environment(options)
  if next(environment) ~= nil then
    spec.set_environment_variables = environment
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
