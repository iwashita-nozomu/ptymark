[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)]
  [string]$Binary,
  [Parameter(Mandatory = $true)]
  [string]$Root
)

$ErrorActionPreference = 'Stop'
$Binary = [System.IO.Path]::GetFullPath($Binary)
$Root = [System.IO.Path]::GetFullPath($Root)
if (-not (Test-Path $Binary -PathType Leaf)) { throw "ptymark binary was not found: $Binary" }

Remove-Item $Root -Recurse -Force -ErrorAction SilentlyContinue
$HomeRoot = Join-Path $Root 'home'
$FishRoot = Join-Path $HomeRoot '.config\fish'
$NuRoot = Join-Path $HomeRoot '.config\nushell'
$PowerShellRoot = Join-Path $HomeRoot 'Documents\PowerShell'
New-Item -ItemType Directory -Force -Path $FishRoot, $NuRoot, $PowerShellRoot | Out-Null

$Profiles = [ordered]@{
  (Join-Path $HomeRoot '.bashrc') = 'source "$HOME/.bash_plugins"'
  (Join-Path $HomeRoot '.zshrc') = 'source "$ZDOTDIR/plugins.zsh"'
  (Join-Path $FishRoot 'config.fish') = 'source ~/.config/fish/plugins.fish'
  (Join-Path $NuRoot 'config.nu') = 'source ~/.config/nushell/plugins.nu'
  (Join-Path $PowerShellRoot 'Microsoft.PowerShell_profile.ps1') = 'Import-Module PSReadLine'
}
foreach ($Entry in $Profiles.GetEnumerator()) {
  [System.IO.File]::WriteAllText($Entry.Key, "$($Entry.Value)`n", [System.Text.UTF8Encoding]::new($false))
}

function Get-ProfileSnapshot {
  Get-ChildItem $HomeRoot -Recurse -File |
    Sort-Object FullName |
    ForEach-Object {
      $Relative = [System.IO.Path]::GetRelativePath($HomeRoot, $_.FullName)
      $Hash = (Get-FileHash -Algorithm SHA256 $_.FullName).Hash.ToLowerInvariant()
      "$Relative`t$Hash"
    }
}

$Before = @(Get-ProfileSnapshot)
$Config = Join-Path $Root 'config.toml'
$State = Join-Path $Root 'state.toml'
& (Join-Path $PSScriptRoot '..\scripts\installer.ps1') `
  -SkipCore `
  -Binary $Binary `
  -Managed never `
  -Mermaid preview `
  -Math preview `
  -Config $Config `
  -State $State
if ($LASTEXITCODE -ne 0) { throw "installer failed with exit code $LASTEXITCODE" }
$After = @(Get-ProfileSnapshot)
if (($Before -join "`n") -ne ($After -join "`n")) {
  throw 'installer modified one or more shell profile files'
}

$env:STARSHIP_SHELL = 'powershell'
$env:ATUIN_SESSION = 'ptymark-compat'
$env:ZDOTDIR = $HomeRoot
$env:FISH_CONFIG_DIR = $FishRoot
$env:NU_LIB_DIRS = $NuRoot
$Pwsh = (Get-Command pwsh.exe -ErrorAction Stop).Source
$BeginMarker = '__PTYMARK_ENV_BEGIN__'
$EndMarker = '__PTYMARK_ENV_END__'
$Command = "[Console]::Out.WriteLine('$BeginMarker' + [Environment]::GetEnvironmentVariable('STARSHIP_SHELL') + '|' + [Environment]::GetEnvironmentVariable('ATUIN_SESSION') + '|' + [Environment]::GetEnvironmentVariable('ZDOTDIR') + '|' + [Environment]::GetEnvironmentVariable('FISH_CONFIG_DIR') + '|' + [Environment]::GetEnvironmentVariable('NU_LIB_DIRS') + '$EndMarker')"
$ChildOutput = (& $Binary --config $Config -- $Pwsh -NoProfile -Command $Command) -join "`n"
if ($LASTEXITCODE -ne 0) { throw 'PowerShell child launch failed' }
$Expected = "powershell|ptymark-compat|$HomeRoot|$FishRoot|$NuRoot"
$PayloadPattern = [regex]::Escape($BeginMarker) + '(?<payload>.*?)' + [regex]::Escape($EndMarker)
$PayloadMatch = [regex]::Match($ChildOutput, $PayloadPattern, [System.Text.RegularExpressions.RegexOptions]::Singleline)
if (-not $PayloadMatch.Success) {
  throw "child environment marker was not found in PTY output: '$ChildOutput'"
}
$Observed = $PayloadMatch.Groups['payload'].Value
if ($Observed -ne $Expected) {
  throw "child environment changed: expected '$Expected', received '$Observed'"
}

& $Pwsh -NoProfile -Command 'Import-Module PSReadLine; if (-not (Get-Module PSReadLine)) { exit 1 }'
if ($LASTEXITCODE -ne 0) { throw 'PSReadLine module smoke failed' }

Write-Output 'ptymark Windows shell profile coexistence: ok'
