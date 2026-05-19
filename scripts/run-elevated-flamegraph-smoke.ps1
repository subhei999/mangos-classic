param(
    [string]$OutputDir = "logs\perf-rca"
)

$ErrorActionPreference = "Stop"

$workspace = Split-Path -Parent (Split-Path -Parent $PSCommandPath)
Set-Location $workspace

$resolvedOutputDir = Join-Path $workspace $OutputDir
New-Item -ItemType Directory -Force -Path $resolvedOutputDir | Out-Null

$timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
$logPath = Join-Path $resolvedOutputDir "$timestamp-elevated-flamegraph-smoke.log"
$svgPath = Join-Path $resolvedOutputDir "$timestamp-elevated-flamegraph-smoke.svg"
$donePath = Join-Path $resolvedOutputDir "$timestamp-elevated-flamegraph-smoke.done.txt"

$principal = New-Object Security.Principal.WindowsPrincipal(
    [Security.Principal.WindowsIdentity]::GetCurrent()
)
$isAdmin = $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)

try {
    @(
        "timestamp=$timestamp"
        "workspace=$workspace"
        "is_admin=$isAdmin"
        "svg=$svgPath"
        "log=$logPath"
        "flamegraph_path=$((Get-Command flamegraph -ErrorAction SilentlyContinue).Source)"
        ""
    ) | Set-Content -Path $logPath -Encoding UTF8

    & flamegraph `
        --verbose `
        --output $svgPath `
        --title "elevated flamegraph smoke" `
        -- powershell.exe -NoProfile -Command "Start-Sleep -Seconds 3" `
        *>> $logPath

    $exitCode = $LASTEXITCODE
    $svgExists = Test-Path $svgPath
    $svgLength = if ($svgExists) { (Get-Item $svgPath).Length } else { 0 }

    @(
        "exit_code=$exitCode"
        "svg_exists=$svgExists"
        "svg_length=$svgLength"
    ) | Set-Content -Path $donePath -Encoding UTF8

    exit $exitCode
}
catch {
    @(
        "exit_code=exception"
        "error=$($_.Exception.Message)"
    ) | Set-Content -Path $donePath -Encoding UTF8
    $_ | Out-String | Add-Content -Path $logPath -Encoding UTF8
    exit 1
}
