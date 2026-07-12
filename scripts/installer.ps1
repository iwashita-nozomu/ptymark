[CmdletBinding()]
param(
  [string]$Root,
  [string]$Binary,
  [switch]$SkipCore,
  [string]$Config,
  [string]$State,
  [string]$Mermaid,
  [string]$Math,
  [string]$Presenter,
  [ValidateSet('auto', 'always', 'never')]
  [string]$Managed = 'auto',
  [string]$ManagedRoot,
  [string]$Browser,
  [switch]$SkipBrowserDownload,
  [switch]$Offline,
  [switch]$ForceManaged,
  [switch]$Reprobe,
  [switch]$Reset,
  [switch]$DryRun,
  [Alias('h')]
  [switch]$Help
)

$ErrorActionPreference = 'Stop'

function Show-Usage {
  @'
Usage: pwsh -File scripts/installer.ps1 [OPTIONS]

Install the Windows-native ptymark binary, prefer user-installed renderer
commands, and provision missing default roles in a versioned LOCALAPPDATA
bundle. No global npm package or PATH entry is created.

Options:
  -Root DIR
  -Binary PATH
  -SkipCore
  -Config PATH
  -State PATH
  -Mermaid auto|keep|preview|source|EXECUTABLE
  -Math auto|keep|preview|source|EXECUTABLE
  -Presenter auto|keep|EXECUTABLE
  -Managed auto|always|never
  -ManagedRoot DIR
  -Browser PATH
  -SkipBrowserDownload
  -Offline
  -ForceManaged
  -Reprobe
  -Reset
  -DryRun
  -Help

Git Bash, MSYS2, and Cygwin users may run scripts/installer.sh; it converts
POSIX paths and delegates to this Windows-native installer.
'@ | Write-Output
}

if ($Help) {
  Show-Usage
  exit 0
}
if ($env:OS -ne 'Windows_NT') {
  throw 'scripts/installer.ps1 is the Windows-native installer; use scripts/installer.sh on Linux, macOS, or WSL'
}

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$Versions = @{}
Get-Content (Join-Path $RepoRoot 'renderers/managed-bundle.env') | ForEach-Object {
  if ($_ -match '^([A-Z0-9_]+)=(.+)$') { $Versions[$Matches[1]] = $Matches[2] }
}
$BundleId = "v$($Versions.PTYMARK_MANAGED_BUNDLE_VERSION)-node$($Versions.PTYMARK_MANAGED_NODE_VERSION)-mermaid$($Versions.PTYMARK_MANAGED_MERMAID_VERSION)-mathjax$($Versions.PTYMARK_MANAGED_MATHJAX_VERSION)"

if (-not $SkipCore) {
  if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw 'cargo is required to install the ptymark core binary'
  }
  $CargoArgs = @('install', '--locked', '--force', '--path', $RepoRoot)
  if ($Root) { $CargoArgs += @('--root', [System.IO.Path]::GetFullPath($Root)) }
  & cargo @CargoArgs
  if ($LASTEXITCODE -ne 0) { throw "cargo install failed with exit code $LASTEXITCODE" }
}

if (-not $Binary) {
  if ($Root) { $Binary = Join-Path $Root 'bin\ptymark.exe' }
  elseif ($env:CARGO_INSTALL_ROOT) { $Binary = Join-Path $env:CARGO_INSTALL_ROOT 'bin\ptymark.exe' }
  elseif ($env:CARGO_HOME) { $Binary = Join-Path $env:CARGO_HOME 'bin\ptymark.exe' }
  else { $Binary = Join-Path $HOME '.cargo\bin\ptymark.exe' }
}
$Binary = [System.IO.Path]::GetFullPath($Binary)
if (-not (Test-Path $Binary -PathType Leaf)) { throw "ptymark binary was not found: $Binary" }

if (-not $Config) {
  if ($env:PTYMARK_CONFIG) { $Config = $env:PTYMARK_CONFIG }
  elseif ($env:APPDATA) { $Config = Join-Path $env:APPDATA 'ptymark\config.toml' }
  else { $Config = Join-Path $HOME '.config\ptymark\config.toml' }
}
$Config = [System.IO.Path]::GetFullPath($Config)
if ($State) { $State = [System.IO.Path]::GetFullPath($State) }

if (-not $ManagedRoot) {
  if (-not $env:LOCALAPPDATA) { throw 'LOCALAPPDATA is required when -ManagedRoot is omitted' }
  $ManagedRoot = Join-Path $env:LOCALAPPDATA "ptymark\renderer-bundles\$BundleId"
}
$ManagedRoot = [System.IO.Path]::GetFullPath($ManagedRoot)
$ManagedMermaid = Join-Path $ManagedRoot 'bin\mmdc.exe'
$ManagedMath = Join-Path $ManagedRoot 'bin\tex2svg.exe'
$ManagedPresenter = Join-Path $ManagedRoot 'bin\chafa.exe'

function Get-ProgramPath([string[]]$Names) {
  foreach ($Name in $Names) {
    $Command = Get-Command $Name -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($Command) { return [System.IO.Path]::GetFullPath($Command.Source) }
  }
  return $null
}

function Get-BrowserPath {
  $Candidates = [System.Collections.Generic.List[string]]::new()
  if (${env:ProgramFiles(x86)}) {
    $Candidates.Add((Join-Path ${env:ProgramFiles(x86)} 'Microsoft\Edge\Application\msedge.exe'))
    $Candidates.Add((Join-Path ${env:ProgramFiles(x86)} 'Google\Chrome\Application\chrome.exe'))
  }
  if ($env:ProgramFiles) {
    $Candidates.Add((Join-Path $env:ProgramFiles 'Microsoft\Edge\Application\msedge.exe'))
    $Candidates.Add((Join-Path $env:ProgramFiles 'Google\Chrome\Application\chrome.exe'))
  }
  return $Candidates | Where-Object { Test-Path $_ -PathType Leaf } | Select-Object -First 1
}

function Test-BuiltinChoice([string]$Value) {
  return $Value -eq 'preview' -or $Value -eq 'source'
}

$ResolveDefaults = (-not (Test-Path $Config -PathType Leaf)) -or $Reset -or $Reprobe -or
  [bool]$Mermaid -or [bool]$Math -or [bool]$Presenter

if ($ResolveDefaults) {
  $SystemMermaid = Get-ProgramPath @('mmdc.exe', 'mmdc.cmd', 'mmdc')
  $SystemMath = Get-ProgramPath @('tex2svg.exe', 'tex2svg.cmd', 'tex2svg')
  $SystemPresenter = Get-ProgramPath @('chafa.exe', 'chafa.cmd', 'chafa')

  if ($Mermaid -eq 'auto') { $Mermaid = $null }
  if ($Math -eq 'auto') { $Math = $null }
  if ($Presenter -eq 'auto') { $Presenter = $null }

  $NeedManaged = $Managed -eq 'always'
  if ($Managed -eq 'auto') {
    if (-not $Mermaid -and -not $SystemMermaid) { $NeedManaged = $true }
    if (-not $Math -and -not $SystemMath) { $NeedManaged = $true }
    if (-not $Presenter -and -not $SystemPresenter) { $NeedManaged = $true }
  }

  $ManagedReady = (Test-Path $ManagedMermaid -PathType Leaf) -and
    (Test-Path $ManagedMath -PathType Leaf) -and
    (Test-Path $ManagedPresenter -PathType Leaf)

  if ($NeedManaged -and -not $ManagedReady -and -not $DryRun) {
    if (-not $Browser) { $Browser = Get-BrowserPath }
    if ($Browser) { $SkipBrowserDownload = $true }
    $BundleParameters = @{
      Root = $ManagedRoot
      Launcher = $Binary
    }
    if ($Browser) { $BundleParameters.Browser = [System.IO.Path]::GetFullPath($Browser) }
    if ($SkipBrowserDownload) { $BundleParameters.SkipBrowserDownload = $true }
    if ($Offline) { $BundleParameters.Offline = $true }
    if ($ForceManaged) { $BundleParameters.Force = $true }
    & (Join-Path $RepoRoot 'scripts\install-managed-bundle.ps1') @BundleParameters
    if ($LASTEXITCODE -ne 0) { throw "managed bundle installation failed with exit code $LASTEXITCODE" }
    $ManagedReady = (Test-Path $ManagedMermaid -PathType Leaf) -and
      (Test-Path $ManagedMath -PathType Leaf) -and
      (Test-Path $ManagedPresenter -PathType Leaf)
    if (-not $ManagedReady) { throw "managed bundle did not create the expected launchers under $ManagedRoot" }
  }

  if (-not $Mermaid) {
    if ($Managed -eq 'always' -and $ManagedReady) { $Mermaid = $ManagedMermaid }
    elseif ($SystemMermaid) { $Mermaid = $SystemMermaid }
    elseif ($ManagedReady) { $Mermaid = $ManagedMermaid }
    else { $Mermaid = 'preview' }
  }
  if (-not $Math) {
    if ($Managed -eq 'always' -and $ManagedReady) { $Math = $ManagedMath }
    elseif ($SystemMath) { $Math = $SystemMath }
    elseif ($ManagedReady) { $Math = $ManagedMath }
    else { $Math = 'preview' }
  }

  $ExternalSelected = ((-not (Test-BuiltinChoice $Mermaid)) -and $Mermaid -ne 'keep') -or
    ((-not (Test-BuiltinChoice $Math)) -and $Math -ne 'keep')
  if (-not $Presenter -and $ExternalSelected) {
    if ($Managed -eq 'always' -and $ManagedReady) { $Presenter = $ManagedPresenter }
    elseif ($SystemPresenter) { $Presenter = $SystemPresenter }
    elseif ($ManagedReady) { $Presenter = $ManagedPresenter }
    else { throw 'No terminal presenter is available; select preview/source or allow the managed bundle' }
  }
}

$ResolveArgs = @('install', 'resolve', '--config', $Config)
if ($State) { $ResolveArgs += @('--state', $State) }
if ($Mermaid) { $ResolveArgs += @('--mermaid', $Mermaid) }
if ($Math) { $ResolveArgs += @('--math', $Math) }
if ($Presenter) { $ResolveArgs += @('--presenter', $Presenter) }
if ($Reset) { $ResolveArgs += '--reset' }
if ($DryRun) { $ResolveArgs += '--dry-run' }
& $Binary @ResolveArgs
if ($LASTEXITCODE -ne 0) { throw "ptymark install resolve failed with exit code $LASTEXITCODE" }

if (-not $DryRun) {
  $StatusArgs = @('install', 'status')
  if ($State) { $StatusArgs += @('--state', $State) }
  & $Binary @StatusArgs
  if ($LASTEXITCODE -ne 0) { throw "ptymark install status failed with exit code $LASTEXITCODE" }
}
