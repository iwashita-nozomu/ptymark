# Compatibility entrypoint. New documentation uses scripts/installer.ps1.
& (Join-Path $PSScriptRoot 'installer.ps1') @args
exit $LASTEXITCODE
