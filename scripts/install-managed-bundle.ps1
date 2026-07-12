[CmdletBinding()]
param(
  [string]$Root,
  [Parameter(Mandatory = $true)]
  [string]$Launcher,
  [string]$Browser,
  [switch]$SkipBrowserDownload,
  [switch]$Offline,
  [switch]$Force
)

$ErrorActionPreference = 'Stop'
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$Launcher = [System.IO.Path]::GetFullPath($Launcher)
if (-not (Test-Path $Launcher -PathType Leaf)) { throw "Launcher was not found: $Launcher" }

$Versions = @{}
Get-Content (Join-Path $RepoRoot 'renderers/managed-bundle.env') | ForEach-Object {
  if ($_ -match '^([A-Z0-9_]+)=(.+)$') { $Versions[$Matches[1]] = $Matches[2] }
}
$BundleVersion = $Versions.PTYMARK_MANAGED_BUNDLE_VERSION
$NodeVersion = $Versions.PTYMARK_MANAGED_NODE_VERSION
$MermaidVersion = $Versions.PTYMARK_MANAGED_MERMAID_VERSION
$MathJaxVersion = $Versions.PTYMARK_MANAGED_MATHJAX_VERSION

$Architecture = switch ($env:PROCESSOR_ARCHITECTURE) {
  'AMD64' { 'x64' }
  'ARM64' { 'arm64' }
  default { throw "Unsupported Windows architecture: $($env:PROCESSOR_ARCHITECTURE)" }
}
$BundleId = "v$BundleVersion-node$NodeVersion-mermaid$MermaidVersion-mathjax$MathJaxVersion"
if (-not $Root) {
  if (-not $env:LOCALAPPDATA) { throw 'LOCALAPPDATA is required when -Root is omitted' }
  $Root = Join-Path $env:LOCALAPPDATA "ptymark\renderer-bundles\$BundleId"
}
$Root = [System.IO.Path]::GetFullPath($Root)
$RuntimeRoot = Join-Path $Root "runtime\node-v$NodeVersion-win-$Architecture"
$AppRoot = Join-Path $Root 'app'
$BinRoot = Join-Path $Root 'bin'
$CacheRoot = Join-Path $Root 'cache\puppeteer'
$StampPath = Join-Path $Root 'bundle.stamp'
$ManifestPath = Join-Path $Root 'bundle.toml'
New-Item -ItemType Directory -Force -Path $BinRoot, $CacheRoot | Out-Null

function Get-CommandPath([string]$Name) {
  $Command = Get-Command $Name -ErrorAction SilentlyContinue | Select-Object -First 1
  if ($Command) { return $Command.Source }
  return $null
}

function Invoke-Download([string]$Uri, [string]$Destination) {
  Invoke-WebRequest -UseBasicParsing -Uri $Uri -OutFile $Destination
}

$NodeCommand = Get-CommandPath 'node.exe'
$NpmCommand = Get-CommandPath 'npm.cmd'
$UseSystemNode = $false
if ($NodeCommand -and $NpmCommand) {
  $DetectedNode = (& $NodeCommand -p 'process.versions.node').Trim()
  $UseSystemNode = $DetectedNode -eq $NodeVersion
}
if (-not $UseSystemNode) {
  $NodeCommand = Join-Path $RuntimeRoot 'node.exe'
  $NpmCommand = Join-Path $RuntimeRoot 'npm.cmd'
  if (-not ((Test-Path $NodeCommand -PathType Leaf) -and (Test-Path $NpmCommand -PathType Leaf))) {
    if ($Offline) { throw "Offline mode: private Node runtime is missing at $RuntimeRoot" }
    $Temporary = Join-Path ([System.IO.Path]::GetTempPath()) ("ptymark-node-" + [guid]::NewGuid())
    New-Item -ItemType Directory -Force -Path $Temporary | Out-Null
    try {
      $Archive = "node-v$NodeVersion-win-$Architecture.zip"
      $BaseUri = "https://nodejs.org/dist/v$NodeVersion"
      $ArchivePath = Join-Path $Temporary $Archive
      $ChecksumsPath = Join-Path $Temporary 'SHASUMS256.txt'
      Invoke-Download "$BaseUri/$Archive" $ArchivePath
      Invoke-Download "$BaseUri/SHASUMS256.txt" $ChecksumsPath
      $Line = Get-Content $ChecksumsPath | Where-Object { $_ -match "^[0-9a-fA-F]{64}\s+$([regex]::Escape($Archive))$" } | Select-Object -First 1
      if (-not $Line) { throw "Node checksum entry is missing for $Archive" }
      $Expected = ($Line -split '\s+')[0].ToLowerInvariant()
      $Actual = (Get-FileHash -Algorithm SHA256 $ArchivePath).Hash.ToLowerInvariant()
      if ($Actual -ne $Expected) { throw "Node archive checksum mismatch for $Archive" }
      $Extracted = Join-Path $Temporary 'expanded'
      Expand-Archive -Path $ArchivePath -DestinationPath $Extracted -Force
      $SourceRoot = Get-ChildItem $Extracted -Directory | Select-Object -First 1
      if (-not $SourceRoot) { throw 'Node archive did not contain a root directory' }
      Remove-Item $RuntimeRoot -Recurse -Force -ErrorAction SilentlyContinue
      New-Item -ItemType Directory -Force -Path $RuntimeRoot | Out-Null
      Copy-Item (Join-Path $SourceRoot.FullName '*') $RuntimeRoot -Recurse -Force
    }
    finally {
      Remove-Item $Temporary -Recurse -Force -ErrorAction SilentlyContinue
    }
  }
}
$NodeCommand = [System.IO.Path]::GetFullPath($NodeCommand)

if ($Browser) {
  $Browser = [System.IO.Path]::GetFullPath($Browser)
  if (-not (Test-Path $Browser -PathType Leaf)) { throw "Browser was not found: $Browser" }
}
elseif ($SkipBrowserDownload) {
  $Candidates = [System.Collections.Generic.List[string]]::new()
  if (${env:ProgramFiles(x86)}) {
    $Candidates.Add((Join-Path ${env:ProgramFiles(x86)} 'Microsoft\Edge\Application\msedge.exe'))
    $Candidates.Add((Join-Path ${env:ProgramFiles(x86)} 'Google\Chrome\Application\chrome.exe'))
  }
  if ($env:ProgramFiles) {
    $Candidates.Add((Join-Path $env:ProgramFiles 'Microsoft\Edge\Application\msedge.exe'))
    $Candidates.Add((Join-Path $env:ProgramFiles 'Google\Chrome\Application\chrome.exe'))
  }
  $Browser = $Candidates | Where-Object { Test-Path $_ -PathType Leaf } | Select-Object -First 1
  if (-not $Browser) { throw 'No Chromium-compatible browser was found while -SkipBrowserDownload is active' }
}

$LockPath = Join-Path $RepoRoot 'renderers/package-lock.json'
$LockSha = (Get-FileHash -Algorithm SHA256 $LockPath).Hash.ToLowerInvariant()
$LauncherSha = (Get-FileHash -Algorithm SHA256 $Launcher).Hash.ToLowerInvariant()
$BrowserIdentity = if ($Browser) { $Browser } else { 'puppeteer-managed' }
$ExpectedStamp = @(
  "bundle=$BundleId"
  "lock_sha=$LockSha"
  "launcher_sha=$LauncherSha"
  "node=$NodeCommand"
  "browser=$BrowserIdentity"
) -join "`n"
$Installed = (-not $Force) -and (Test-Path $StampPath -PathType Leaf) -and
  ((Get-Content $StampPath -Raw).TrimEnd("`r", "`n") -eq $ExpectedStamp) -and
  (Test-Path (Join-Path $AppRoot 'node_modules') -PathType Container)

if (-not $Installed) {
  if ($Offline) { throw 'Offline mode: managed renderer app is incomplete' }
  Remove-Item $AppRoot -Recurse -Force -ErrorAction SilentlyContinue
  New-Item -ItemType Directory -Force -Path (Join-Path $AppRoot 'managed') | Out-Null
  Copy-Item (Join-Path $RepoRoot 'renderers/package.json') (Join-Path $AppRoot 'package.json')
  Copy-Item $LockPath (Join-Path $AppRoot 'package-lock.json')
  Copy-Item (Join-Path $RepoRoot 'renderers/managed/mathjax-cli.mjs') (Join-Path $AppRoot 'managed/mathjax-cli.mjs')
  Copy-Item (Join-Path $RepoRoot 'renderers/managed/ansi-presenter.mjs') (Join-Path $AppRoot 'managed/ansi-presenter.mjs')
  $env:PUPPETEER_CACHE_DIR = $CacheRoot
  $env:npm_config_cache = Join-Path $Root 'cache\npm'
  if ($SkipBrowserDownload) { $env:PUPPETEER_SKIP_DOWNLOAD = 'true' }
  else { Remove-Item Env:PUPPETEER_SKIP_DOWNLOAD -ErrorAction SilentlyContinue }
  & $NpmCommand ci --prefix $AppRoot --omit=dev --no-audit --no-fund
  if ($LASTEXITCODE -ne 0) { throw "npm ci failed with exit code $LASTEXITCODE" }
  Set-Content -Path $StampPath -Value $ExpectedStamp -NoNewline -Encoding UTF8
}

function Install-NativeAlias([string]$Destination) {
  Remove-Item $Destination -Force -ErrorAction SilentlyContinue
  try {
    New-Item -ItemType HardLink -Path $Destination -Target $Launcher -ErrorAction Stop | Out-Null
  }
  catch {
    Copy-Item $Launcher $Destination -Force
  }
}

$MermaidAlias = Join-Path $BinRoot 'mmdc.exe'
$MathAlias = Join-Path $BinRoot 'tex2svg.exe'
$PresenterAlias = Join-Path $BinRoot 'chafa.exe'
Install-NativeAlias $MermaidAlias
Install-NativeAlias $MathAlias
Install-NativeAlias $PresenterAlias

function Convert-ToTomlString([string]$Value) {
  return '"' + $Value.Replace('\', '\\').Replace('"', '\"') + '"'
}
$Manifest = @(
  'schema_version = 1'
  "node_path = $(Convert-ToTomlString $NodeCommand)"
  "app_root = $(Convert-ToTomlString $AppRoot)"
  "cache_root = $(Convert-ToTomlString $CacheRoot)"
)
if ($Browser) { $Manifest += "browser_path = $(Convert-ToTomlString $Browser)" }
$Manifest += 'browser_no_sandbox = false'
Set-Content -Path $ManifestPath -Value $Manifest -Encoding UTF8

"root`t$Root"
"node`t$NodeCommand"
"mermaid`t$MermaidAlias"
"math`t$MathAlias"
"presenter`t$PresenterAlias"
"browser`t$BrowserIdentity"
