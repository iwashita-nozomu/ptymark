$ErrorActionPreference = 'Stop'
$PackageRoot = [System.IO.Path]::GetFullPath($PSScriptRoot)
$Binary = Join-Path $PackageRoot 'bin\ptymark.exe'
$Installer = Join-Path $PackageRoot 'scripts\installer.ps1'

if (-not (Test-Path $Binary -PathType Leaf)) {
  throw "Packaged ptymark binary was not found: $Binary"
}
if (-not (Test-Path $Installer -PathType Leaf)) {
  throw "Packaged ptymark installer was not found: $Installer"
}

& $Installer -SkipCore -Binary $Binary @args
exit $LASTEXITCODE
