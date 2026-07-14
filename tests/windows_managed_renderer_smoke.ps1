[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)]
  [string]$Binary,
  [Parameter(Mandatory = $true)]
  [string]$Browser,
  [Parameter(Mandatory = $true)]
  [string]$Root
)

$ErrorActionPreference = 'Stop'
$Binary = [System.IO.Path]::GetFullPath($Binary)
$Browser = [System.IO.Path]::GetFullPath($Browser)
$Root = [System.IO.Path]::GetFullPath($Root)
if (-not (Test-Path $Binary -PathType Leaf)) { throw "ptymark binary was not found: $Binary" }
if (-not (Test-Path $Browser -PathType Leaf)) { throw "browser was not found: $Browser" }

Remove-Item $Root -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $Root | Out-Null
$LogRoot = Join-Path $Root 'logs'
New-Item -ItemType Directory -Force -Path $LogRoot | Out-Null
$Bundle = Join-Path $Root 'bundle'
$Config = Join-Path $Root 'config.toml'
$StrictConfig = Join-Path $Root 'strict-config.toml'
$State = Join-Path $Root 'state.toml'

function Write-Utf8NoBom([string]$Path, [string]$Text) {
  [System.IO.File]::WriteAllText($Path, $Text, [System.Text.UTF8Encoding]::new($false))
}

function Invoke-NativeStage {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Name,
    [Parameter(Mandatory = $true)]
    [string]$FilePath,
    [string[]]$ArgumentList = @()
  )

  Write-Host "[ptymark-smoke] $Name"
  $StartInfo = [System.Diagnostics.ProcessStartInfo]::new()
  $StartInfo.FileName = $FilePath
  $StartInfo.UseShellExecute = $false
  $StartInfo.CreateNoWindow = $true
  $StartInfo.RedirectStandardOutput = $true
  $StartInfo.RedirectStandardError = $true
  foreach ($Argument in $ArgumentList) {
    $StartInfo.ArgumentList.Add([string]$Argument)
  }

  $Process = [System.Diagnostics.Process]::new()
  $Process.StartInfo = $StartInfo
  if (-not $Process.Start()) { throw "$Name could not be started" }
  $StdoutTask = $Process.StandardOutput.ReadToEndAsync()
  $StderrTask = $Process.StandardError.ReadToEndAsync()
  $Process.WaitForExit()
  $Stdout = $StdoutTask.GetAwaiter().GetResult()
  $Stderr = $StderrTask.GetAwaiter().GetResult()
  Write-Utf8NoBom (Join-Path $LogRoot "$Name.stdout.log") $Stdout
  Write-Utf8NoBom (Join-Path $LogRoot "$Name.stderr.log") $Stderr
  Write-Utf8NoBom (Join-Path $LogRoot "$Name.exit.txt") "$($Process.ExitCode)`n"
  if ($Stderr) { Write-Host $Stderr }
  if ($Process.ExitCode -ne 0) {
    throw "$Name failed with exit code $($Process.ExitCode); see $LogRoot"
  }
  return $Stdout
}

try {
  $InstallerLog = Join-Path $LogRoot 'installer.log'
  & (Join-Path $PSScriptRoot '..\scripts\installer.ps1') `
    -SkipCore `
    -Binary $Binary `
    -Managed always `
    -ManagedRoot $Bundle `
    -Config $Config `
    -State $State `
    -Browser $Browser `
    -SkipBrowserDownload *>&1 | Tee-Object -FilePath $InstallerLog
  if ($LASTEXITCODE -ne 0) { throw "installer failed with exit code $LASTEXITCODE" }

  Copy-Item $Config (Join-Path $LogRoot 'config.toml') -Force
  Copy-Item $State (Join-Path $LogRoot 'state.toml') -Force
  Copy-Item (Join-Path $Bundle 'bundle.toml') (Join-Path $LogRoot 'bundle.toml') -Force
  Copy-Item (Join-Path $Bundle 'puppeteer-config.json') (Join-Path $LogRoot 'puppeteer-config.json') -Force

  $null = Invoke-NativeStage 'config-check' $Binary @('--config', $Config, 'config', 'check')
  $null = Invoke-NativeStage 'engine-check' $Binary @('--config', $Config, 'engine', 'check')
  $null = Invoke-NativeStage 'install-status' $Binary @('install', 'status', '--state', $State)

  $Mmdc = Join-Path $Bundle 'bin\mmdc.exe'
  $Tex2Svg = Join-Path $Bundle 'bin\tex2svg.exe'
  $Presenter = Join-Path $Bundle 'bin\chafa.exe'
  foreach ($Path in @($Mmdc, $Tex2Svg, $Presenter)) {
    if (-not (Test-Path $Path -PathType Leaf)) { throw "managed launcher was not found: $Path" }
  }

  $DiagramBody = "flowchart LR`n  Install --> Resolve --> Render`n"
  $DiagramInput = Join-Path $Root 'diagram.mmd'
  $DiagramSvg = Join-Path $Root 'direct-mermaid.svg'
  Write-Utf8NoBom $DiagramInput $DiagramBody
  $null = Invoke-NativeStage 'direct-mermaid' $Mmdc @('--input', $DiagramInput, '--output', $DiagramSvg)
  if (-not (Test-Path $DiagramSvg -PathType Leaf)) { throw 'direct Mermaid renderer did not create SVG' }
  if ((Get-Content $DiagramSvg -Raw) -notmatch '<svg') { throw 'direct Mermaid output is not SVG' }

  $MathStdout = Invoke-NativeStage 'direct-math' $Tex2Svg @('E = mc^2')
  $MathSvg = Join-Path $Root 'direct-math.svg'
  Write-Utf8NoBom $MathSvg $MathStdout
  if ($MathStdout -notmatch '<svg') { throw 'direct MathJax output is not SVG' }

  $Presented = Invoke-NativeStage 'direct-presenter' $Presenter @(
    '--format', 'symbols',
    '--probe', 'off',
    '--polite', 'on',
    '--relative', 'off',
    '--animate', 'off',
    '--colors', 'none',
    '--size', '48x',
    $DiagramSvg
  )
  Write-Utf8NoBom (Join-Path $Root 'direct-presenter.txt') $Presented
  if (-not $Presented) { throw 'direct managed presenter produced no output' }

  $MermaidInput = Join-Path $Root 'mermaid.md'
  $Mermaid = (@(
    '```mermaid'
    'flowchart LR'
    '  Windows --> Installer --> Renderer'
    '```'
  ) -join "`n") + "`n"
  Write-Utf8NoBom $MermaidInput $Mermaid
  $MermaidOutput = Invoke-NativeStage 'strict-mermaid-preview' $Binary @(
    '--config', $Config,
    'preview', '--strict', '--columns', '48', $MermaidInput
  )
  if (-not $MermaidOutput -or $MermaidOutput.Contains('```mermaid')) {
    throw 'Mermaid source was not replaced by managed output'
  }

  $MathInput = Join-Path $Root 'math.md'
  $Math = (@('$$', 'E = mc^2', '$$') -join "`n") + "`n"
  Write-Utf8NoBom $MathInput $Math
  $MathOutput = Invoke-NativeStage 'strict-math-preview' $Binary @(
    '--config', $Config,
    'preview', '--strict', '--columns', '48', $MathInput
  )
  if (-not $MathOutput -or $MathOutput.Contains('E = mc^2')) {
    throw 'math source was not replaced by managed output'
  }

  $StrictText = (Get-Content $Config -Raw) -replace '(?m)^strict = false$', 'strict = true'
  Write-Utf8NoBom $StrictConfig $StrictText
  $Pwsh = (Get-Command pwsh.exe -ErrorAction Stop).Source
  $InteractiveScript = @'
[Console]::Out.Write((@('before','```mermaid','flowchart LR','  Interactive --> ConPTY --> Renderer','```','$$','E = mc^2','$$','after') -join "`n") + "`n")
'@
  $InteractiveOutput = Invoke-NativeStage 'strict-interactive-conpty' $Binary @(
    '--config', $StrictConfig,
    '--', $Pwsh,
    '-NoLogo', '-NoProfile', '-NonInteractive', '-Command', $InteractiveScript
  )
  if (-not $InteractiveOutput -or $InteractiveOutput.Contains('```mermaid') -or $InteractiveOutput.Contains('$$')) {
    throw 'interactive ConPTY path fell back to semantic source'
  }

  Write-Utf8NoBom (Join-Path $LogRoot 'result.txt') "success`n"
  Write-Output 'ptymark Windows managed renderer and real-ConPTY smoke: ok'
}
catch {
  Write-Utf8NoBom (Join-Path $LogRoot 'result.txt') "failure`n$($_ | Out-String)"
  throw
}
