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
$Bundle = Join-Path $Root 'bundle'
$Config = Join-Path $Root 'config.toml'
$State = Join-Path $Root 'state.toml'

& (Join-Path $PSScriptRoot '..\scripts\installer.ps1') `
  -SkipCore `
  -Binary $Binary `
  -Managed always `
  -ManagedRoot $Bundle `
  -Config $Config `
  -State $State `
  -Browser $Browser `
  -SkipBrowserDownload
if ($LASTEXITCODE -ne 0) { throw "installer failed with exit code $LASTEXITCODE" }

& $Binary --config $Config config check
if ($LASTEXITCODE -ne 0) { throw 'config check failed' }
& $Binary --config $Config engine check
if ($LASTEXITCODE -ne 0) { throw 'engine check failed' }
& $Binary install status --state $State
if ($LASTEXITCODE -ne 0) { throw 'install status failed' }

$Mmdc = Join-Path $Bundle 'bin\mmdc.exe'
$Tex2Svg = Join-Path $Bundle 'bin\tex2svg.exe'
$Presenter = Join-Path $Bundle 'bin\chafa.exe'
foreach ($Path in @($Mmdc, $Tex2Svg, $Presenter)) {
  if (-not (Test-Path $Path -PathType Leaf)) { throw "managed launcher was not found: $Path" }
}

$DiagramBody = "flowchart LR`n  Install --> Resolve --> Render`n"
$DiagramInput = Join-Path $Root 'diagram.mmd'
$DiagramSvg = Join-Path $Root 'direct-mermaid.svg'
[System.IO.File]::WriteAllText($DiagramInput, $DiagramBody, [System.Text.UTF8Encoding]::new($false))
& $Mmdc --input $DiagramInput --output $DiagramSvg
if ($LASTEXITCODE -ne 0) { throw 'direct Mermaid renderer failed' }
if (-not (Test-Path $DiagramSvg -PathType Leaf)) { throw 'direct Mermaid renderer did not create SVG' }
if ((Get-Content $DiagramSvg -Raw) -notmatch '<svg') { throw 'direct Mermaid output is not SVG' }

$MathSvg = Join-Path $Root 'direct-math.svg'
& $Tex2Svg 'E = mc^2' | Set-Content -Encoding utf8NoBOM $MathSvg
if ($LASTEXITCODE -ne 0) { throw 'direct MathJax renderer failed' }
if ((Get-Content $MathSvg -Raw) -notmatch '<svg') { throw 'direct MathJax output is not SVG' }

$Presented = Join-Path $Root 'direct-presenter.txt'
& $Presenter `
  --format symbols `
  --probe off `
  --polite on `
  --relative off `
  --animate off `
  --colors none `
  --size 48x `
  $DiagramSvg | Set-Content -Encoding utf8NoBOM $Presented
if ($LASTEXITCODE -ne 0) { throw 'direct managed presenter failed' }
if (-not (Test-Path $Presented -PathType Leaf) -or (Get-Item $Presented).Length -eq 0) {
  throw 'direct managed presenter produced no output'
}

$MermaidInput = Join-Path $Root 'mermaid.md'
$Mermaid = (@(
  '```mermaid'
  'flowchart LR'
  '  Windows --> Installer --> Renderer'
  '```'
) -join "`n") + "`n"
[System.IO.File]::WriteAllText($MermaidInput, $Mermaid, [System.Text.UTF8Encoding]::new($false))
$MermaidOutput = (& $Binary --config $Config preview --strict --columns 48 $MermaidInput) -join "`n"
if ($LASTEXITCODE -ne 0) { throw 'strict Mermaid preview failed' }
if (-not $MermaidOutput -or $MermaidOutput.Contains('```mermaid')) {
  throw 'Mermaid source was not replaced by managed output'
}

$MathInput = Join-Path $Root 'math.md'
$Math = (@('$$', 'E = mc^2', '$$') -join "`n") + "`n"
[System.IO.File]::WriteAllText($MathInput, $Math, [System.Text.UTF8Encoding]::new($false))
$MathOutput = (& $Binary --config $Config preview --strict --columns 48 $MathInput) -join "`n"
if ($LASTEXITCODE -ne 0) { throw 'strict MathJax preview failed' }
if (-not $MathOutput -or $MathOutput.Contains('E = mc^2')) {
  throw 'math source was not replaced by managed output'
}

Write-Output 'ptymark Windows managed renderer smoke: ok'
