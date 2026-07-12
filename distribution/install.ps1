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

$Forward = [System.Collections.Generic.List[object]]::new()
for ($Index = 0; $Index -lt $args.Count; $Index += 1) {
  $Argument = [string]$args[$Index]
  switch -Regex ($Argument) {
    '^(-BinaryDestination|--binary-destination)$' {
      if ($Index + 1 -ge $args.Count) { throw "Missing value after $Argument" }
      $Index += 1
      $BinaryDestination = [string]$args[$Index]
      continue
    }
    '^(-Binary|--binary|-SkipCore|--skip-core|-Root|--root)$' {
      throw "$Argument is owned by the packaged installer; use -BinaryDestination instead"
    }
    default {
      $Forward.Add($args[$Index])
    }
  }
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

$ForwardArguments = $Forward.ToArray()
& $Installer -SkipCore -Binary $BinaryDestination @ForwardArguments
exit $LASTEXITCODE
