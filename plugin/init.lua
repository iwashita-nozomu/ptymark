local wezterm = require 'wezterm'

local M = {}

local function copy_array(values)
  local result = {}
  for index, value in ipairs(values or {}) do
    result[index] = value
  end
  return result
end

local function copy_map(values)
  local result = {}
  for key, value in pairs(values or {}) do
    result[key] = value
  end
  return result
end

local function session_mode_flag(mode)
  if mode == nil then
    return nil
  end
  assert(type(mode) == 'string',
    'ptymark option `mode` must be one of: source, safe, private')
  local flags = {
    source = '--source',
    safe = '--safe',
    private = '--private',
  }
  assert(flags[mode] ~= nil,
    'ptymark option `mode` must be one of: source, safe, private')
  return flags[mode]
end

function M.command(options)
  options = options or {}

  if options.command ~= nil then
    assert(type(options.command) == 'table' and #options.command > 0,
      'ptymark option `command` must be a non-empty array')
    assert(options.config_file == nil and options.shell == nil and options.mode == nil,
      'ptymark `command` cannot be combined with `config_file`, `shell`, or `mode`')
    return copy_array(options.command)
  end

  local args = { options.binary or 'ptymark' }
  if options.config_file ~= nil then
    assert(type(options.config_file) == 'string' and options.config_file ~= '',
      'ptymark option `config_file` must be a non-empty string')
    table.insert(args, '--config')
    table.insert(args, options.config_file)
  end
  local mode_flag = session_mode_flag(options.mode)
  if mode_flag ~= nil then
    table.insert(args, mode_flag)
  end
  table.insert(args, '--')
  table.insert(args, options.shell or os.getenv('SHELL') or '/bin/sh')
  if options.login_shell ~= false then
    table.insert(args, '-l')
  end
  return args
end

local function spawn_spec(options)
  local spec = {
    args = M.command(options),
  }
  if options.cwd ~= nil then
    spec.cwd = options.cwd
  end
  local environment = copy_map(options.set_environment_variables)
  if next(environment) ~= nil then
    spec.set_environment_variables = environment
  end
  return spec
end

function M.apply_to_config(config, options)
  assert(type(config) == 'table' or type(config) == 'userdata',
    'ptymark requires a WezTerm config builder')
  options = options or {}

  if options.launch_menu ~= false then
    config.launch_menu = config.launch_menu or {}
    local entry = spawn_spec(options)
    entry.label = options.label or 'ptymark shell'
    table.insert(config.launch_menu, entry)
  end

  if options.key ~= false then
    local key = options.key or { key = 'P', mods = 'CTRL|SHIFT' }
    assert(type(key) == 'table' and type(key.key) == 'string',
      'ptymark option `key` must contain a string `key`')
    config.keys = config.keys or {}
    table.insert(config.keys, {
      key = key.key,
      mods = key.mods or 'NONE',
      action = wezterm.action.SpawnCommandInNewTab(spawn_spec(options)),
    })
  end
end

return M
