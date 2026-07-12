$ErrorActionPreference = 'Stop'
$PackageRoot = [System.IO.Path]::GetFullPath($PSScriptRoot)
$SourceBinary = Join-Path $PackageRoot 'bin\ptymark.exe'
$Installer = Join-Path $PackageRoot 'scripts\installer.ps1'

if (-not (Test-Path $SourceBinary -PathType Leaf)) {
  throw "Packaged ptymark binary was not found: $SourceBinary"
}
if (-not (Test-Path $Installer -PathType Leaf)) {
  throw "Packaged ptymark installer was not found: $Installer"
}

if ($env:PTYMARK_BINARY_DEST) {
  $BinaryDestination = $env:PTYMARK_BINARY_DEST
}
elseif ($env:CARGO_HOME) {
  $BinaryDestination = Join-Path $env:CARGO_HOME 'bin\ptymark.exe'
}
else {
  $BinaryDestination = Join-Path $HOME '.cargo\bin\ptymark.exe'
}

$ValueParameters = @{
  '-config' = 'Config'
  '--config' = 'Config'
  '-state' = 'State'
  '--state' = 'State'
  '-mermaid' = 'Mermaid'
  '--mermaid' = 'Mermaid'
  '-math' = 'Math'
  '--math' = 'Math'
  '-presenter' = 'Presenter'
  '--presenter' = 'Presenter'
  '-managed' = 'Managed'
  '--managed' = 'Managed'
  '-managedroot' = 'ManagedRoot'
  '--managed-root' = 'ManagedRoot'
  '-browser' = 'Browser'
  '--browser' = 'Browser'
}
$SwitchParameters = @{
  '-skipbrowserdownload' = 'SkipBrowserDownload'
  '--skip-browser-download' = 'SkipBrowserDownload'
  '-offline' = 'Offline'
  '--offline' = 'Offline'
  '-forcemanaged' = 'ForceManaged'
  '--force-managed' = 'ForceManaged'
  '-reprobe' = 'Reprobe'
  '--reprobe' = 'Reprobe'
  '-reset' = 'Reset'
  '--reset' = 'Reset'
  '-dryrun' = 'DryRun'
  '--dry-run' = 'DryRun'
  '-help' = 'Help'
  '--help' = 'Help'
  '-h' = 'Help'
}
$InvocationParameters = @{
  SkipCore = $true
}

for ($Index = 0; $Index -lt $args.Count; $Index += 1) {
  $Argument = [string]$args[$Index]
  $Key = $Argument.ToLowerInvariant()
  if ($Key -in @('-binarydestination', '--binary-destination')) {
    if ($Index + 1 -ge $args.Count) { throw "Missing value after $Argument" }
    $Index += 1
    $BinaryDestination = [string]$args[$Index]
    continue
  }
  if ($Key -in @('-binary', '--binary', '-skipcore', '--skip-core', '-root', '--root')) {
    throw "$Argument is owned by the packaged installer; use -BinaryDestination instead"
  }
  if ($ValueParameters.ContainsKey($Key)) {
    if ($Index + 1 -ge $args.Count) { throw "Missing value after $Argument" }
    $Index += 1
    $InvocationParameters[$ValueParameters[$Key]] = [string]$args[$Index]
    continue
  }
  if ($SwitchParameters.ContainsKey($Key)) {
    $InvocationParameters[$SwitchParameters[$Key]] = $true
    continue
  }
  throw "Unknown packaged installer option: $Argument"
}

$BinaryDestination = [System.IO.Path]::GetFullPath($BinaryDestination)
$DestinationDirectory = Split-Path -Parent $BinaryDestination
New-Item -ItemType Directory -Force -Path $DestinationDirectory | Out-Null
$TemporaryBinary = Join-Path $DestinationDirectory ('.ptymark-' + [guid]::NewGuid() + '.tmp.exe')
try {
  Copy-Item $SourceBinary $TemporaryBinary -Force
  Move-Item $TemporaryBinary $BinaryDestination -Force
}
finally {
  Remove-Item $TemporaryBinary -Force -ErrorAction SilentlyContinue
}

$InvocationParameters.Binary = $BinaryDestination
& $Installer @InvocationParameters
exit $LASTEXITCODE
