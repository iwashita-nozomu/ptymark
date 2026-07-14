# @dependency-start
# contract release packaging
# responsibility Builds one versioned Windows release archive with package-local installers, renderer metadata, documentation, and checksums.
# upstream design documents/release.md defines release asset contents and naming.
# upstream configuration Cargo.toml owns the package version.
# downstream workflow .github/workflows/ptymark-release.yml validates and publishes this archive.
# @dependency-end

[CmdletBinding()]
param(
  [string]$Binary = (Join-Path $PSScriptRoot '..\target\release\ptymark.exe'),
  [string]$OutputDir = (Join-Path $PSScriptRoot '..\dist')
)

$ErrorActionPreference = 'Stop'
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$Binary = [System.IO.Path]::GetFullPath($Binary)
$OutputDir = [System.IO.Path]::GetFullPath($OutputDir)

if (-not (Test-Path $Binary -PathType Leaf)) {
  throw "Release binary was not found: $Binary"
}

$VersionOutput = (& $Binary --version).Trim()
if ($LASTEXITCODE -ne 0 -or $VersionOutput -notmatch '^ptymark\s+(.+)$') {
  throw "Unexpected version output: $VersionOutput"
}
$Version = $Matches[1]
$Architecture = switch ($env:PROCESSOR_ARCHITECTURE) {
  'AMD64' { 'x86_64' }
  'ARM64' { 'aarch64' }
  default { throw "Unsupported Windows release architecture: $($env:PROCESSOR_ARCHITECTURE)" }
}

$PackageName = "ptymark-$Version-windows-$Architecture"
$PackageRoot = Join-Path $OutputDir $PackageName
$Archive = Join-Path $OutputDir "$PackageName.zip"
$Checksum = "$Archive.sha256"

Remove-Item $PackageRoot -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path @(
  (Join-Path $PackageRoot 'bin'),
  (Join-Path $PackageRoot 'scripts'),
  (Join-Path $PackageRoot 'renderers\managed'),
  (Join-Path $PackageRoot 'plugin'),
  (Join-Path $PackageRoot 'examples'),
  (Join-Path $PackageRoot 'documents'),
  (Join-Path $PackageRoot 'compat\shell-integrations')
) | Out-Null

function Copy-Required([string]$Source, [string]$Destination) {
  if (-not (Test-Path $Source -PathType Leaf)) { throw "Required package file was not found: $Source" }
  Copy-Item $Source $Destination -Force
}

Copy-Required $Binary (Join-Path $PackageRoot 'bin\ptymark.exe')
Copy-Required (Join-Path $RepoRoot 'distribution\install.ps1') (Join-Path $PackageRoot 'install.ps1')
Copy-Required (Join-Path $RepoRoot 'distribution\install.cmd') (Join-Path $PackageRoot 'install.cmd')
Copy-Required (Join-Path $RepoRoot 'distribution\install.sh') (Join-Path $PackageRoot 'install.sh')
Copy-Required (Join-Path $RepoRoot 'scripts\installer.ps1') (Join-Path $PackageRoot 'scripts\installer.ps1')
Copy-Required (Join-Path $RepoRoot 'scripts\install-managed-bundle.ps1') (Join-Path $PackageRoot 'scripts\install-managed-bundle.ps1')
Copy-Required (Join-Path $RepoRoot 'scripts\installer.sh') (Join-Path $PackageRoot 'scripts\installer.sh')
Copy-Required (Join-Path $RepoRoot 'scripts\install-managed-bundle.sh') (Join-Path $PackageRoot 'scripts\install-managed-bundle.sh')
Copy-Required (Join-Path $RepoRoot 'renderers\package.json') (Join-Path $PackageRoot 'renderers\package.json')
Copy-Required (Join-Path $RepoRoot 'renderers\package-lock.json') (Join-Path $PackageRoot 'renderers\package-lock.json')
Copy-Required (Join-Path $RepoRoot 'renderers\managed-bundle.env') (Join-Path $PackageRoot 'renderers\managed-bundle.env')
Copy-Required (Join-Path $RepoRoot 'renderers\managed\mathjax-cli.mjs') (Join-Path $PackageRoot 'renderers\managed\mathjax-cli.mjs')
Copy-Required (Join-Path $RepoRoot 'renderers\managed\ansi-presenter.mjs') (Join-Path $PackageRoot 'renderers\managed\ansi-presenter.mjs')
Copy-Required (Join-Path $RepoRoot 'plugin\init.lua') (Join-Path $PackageRoot 'plugin\init.lua')
Copy-Required (Join-Path $RepoRoot 'examples\ptymark.toml') (Join-Path $PackageRoot 'examples\ptymark.toml')
Copy-Required (Join-Path $RepoRoot 'examples\wezterm.lua') (Join-Path $PackageRoot 'examples\wezterm.lua')
Copy-Required (Join-Path $RepoRoot 'README.md') (Join-Path $PackageRoot 'README.md')
Copy-Required (Join-Path $RepoRoot 'CHANGELOG.md') (Join-Path $PackageRoot 'CHANGELOG.md')
Copy-Required (Join-Path $RepoRoot 'SECURITY.md') (Join-Path $PackageRoot 'SECURITY.md')
Copy-Required (Join-Path $RepoRoot 'LICENSE') (Join-Path $PackageRoot 'LICENSE')
Copy-Required (Join-Path $RepoRoot 'documents\ptymark-design.md') (Join-Path $PackageRoot 'documents\ptymark-design.md')
Copy-Required (Join-Path $RepoRoot 'documents\interactive-session.md') (Join-Path $PackageRoot 'documents\interactive-session.md')
Copy-Required (Join-Path $RepoRoot 'documents\filtered-command.md') (Join-Path $PackageRoot 'documents\filtered-command.md')
Copy-Required (Join-Path $RepoRoot 'documents\release.md') (Join-Path $PackageRoot 'documents\release.md')
Copy-Required (Join-Path $RepoRoot 'documents\ptymark-installer.md') (Join-Path $PackageRoot 'documents\ptymark-installer.md')
Copy-Required (Join-Path $RepoRoot 'documents\shell-plugin-compatibility.md') (Join-Path $PackageRoot 'documents\shell-plugin-compatibility.md')
foreach ($Inventory in @('bash', 'zsh', 'fish', 'powershell', 'nushell')) {
  Copy-Required `
    (Join-Path $RepoRoot "compat\shell-integrations\$Inventory.tsv") `
    (Join-Path $PackageRoot "compat\shell-integrations\$Inventory.tsv")
}

$Manifest = @(
  "name=$PackageName",
  "version=$Version",
  'platform=windows',
  "architecture=$Architecture",
  'binary=bin/ptymark.exe',
  'installer=install.ps1'
) -join "`n"
[System.IO.File]::WriteAllText(
  (Join-Path $PackageRoot 'PACKAGE-MANIFEST.txt'),
  "$Manifest`n",
  [System.Text.UTF8Encoding]::new($false)
)

$SmokeRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("ptymark-package-" + [guid]::NewGuid())
$HomeRoot = Join-Path $SmokeRoot 'home'
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
  foreach ($ProfilePath in $Profiles.Keys) {
    if (-not (Test-Path $ProfilePath -PathType Leaf)) {
      throw "Shell profile disappeared during package smoke: $ProfilePath"
    }
    $Relative = [System.IO.Path]::GetRelativePath($HomeRoot, $ProfilePath)
    $Hash = (Get-FileHash -Algorithm SHA256 $ProfilePath).Hash.ToLowerInvariant()
    "$Relative`t$Hash"
  }
}

$OriginalHome = $env:HOME
$OriginalUserProfile = $env:USERPROFILE
$OriginalAppData = $env:APPDATA
$OriginalLocalAppData = $env:LOCALAPPDATA
try {
  $env:HOME = $HomeRoot
  $env:USERPROFILE = $HomeRoot
  $env:APPDATA = Join-Path $SmokeRoot 'appdata'
  $env:LOCALAPPDATA = Join-Path $SmokeRoot 'localappdata'
  New-Item -ItemType Directory -Force -Path $env:APPDATA, $env:LOCALAPPDATA | Out-Null

  $BeforeProfiles = @(Get-ProfileSnapshot)
  $Config = Join-Path $SmokeRoot 'config.toml'
  $State = Join-Path $SmokeRoot 'state.toml'
  & (Join-Path $PackageRoot 'install.ps1') `
    -Managed never `
    -Mermaid preview `
    -Math preview `
    -Config $Config `
    -State $State
  if ($LASTEXITCODE -ne 0) { throw "Packaged installer failed with exit code $LASTEXITCODE" }

  $AfterProfiles = @(Get-ProfileSnapshot)
  if (($BeforeProfiles -join "`n") -ne ($AfterProfiles -join "`n")) {
    throw 'Packaged installer modified one or more shell profile files'
  }

  $PackagedBinary = Join-Path $PackageRoot 'bin\ptymark.exe'
  & $PackagedBinary --version | Out-Null
  if ($LASTEXITCODE -ne 0) { throw 'Packaged binary version smoke failed' }
  & $PackagedBinary --config $Config config check | Out-Null
  if ($LASTEXITCODE -ne 0) { throw 'Packaged configuration smoke failed' }

  $Preview = (@('$$', 'E = mc^2', '$$') -join "`n") + "`n"
  $PreviewOutput = ($Preview | & $PackagedBinary --config $Config preview -) -join "`n"
  if ($LASTEXITCODE -ne 0 -or $PreviewOutput -notmatch 'ptymark math') {
    throw 'Packaged preview smoke failed'
  }
}
finally {
  $env:HOME = $OriginalHome
  $env:USERPROFILE = $OriginalUserProfile
  $env:APPDATA = $OriginalAppData
  $env:LOCALAPPDATA = $OriginalLocalAppData
  Remove-Item $SmokeRoot -Recurse -Force -ErrorAction SilentlyContinue
}

New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null
Remove-Item $Archive, $Checksum -Force -ErrorAction SilentlyContinue
Compress-Archive -Path $PackageRoot -DestinationPath $Archive -CompressionLevel Optimal
$Hash = (Get-FileHash -Algorithm SHA256 $Archive).Hash.ToLowerInvariant()
[System.IO.File]::WriteAllText(
  $Checksum,
  "$Hash  $([System.IO.Path]::GetFileName($Archive))`n",
  [System.Text.UTF8Encoding]::new($false)
)

"package`t$PackageRoot"
"archive`t$Archive"
"checksum`t$Checksum"
