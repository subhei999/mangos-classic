param(
    [int]$DurationSeconds = 150,

    [string]$OutputDir = "logs\perf-rca",

    [string]$Scenario = "worldserver-wpr-cpu"
)

$ErrorActionPreference = "Stop"

$workspace = Split-Path -Parent (Split-Path -Parent $PSCommandPath)
Set-Location $workspace

$resolvedOutputDir = Join-Path $workspace $OutputDir
New-Item -ItemType Directory -Force -Path $resolvedOutputDir | Out-Null

$timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
$safeScenario = $Scenario -replace '[^A-Za-z0-9_.-]', '-'
$prefix = "$timestamp-$safeScenario"
$logPath = Join-Path $resolvedOutputDir "$prefix.wpr.log"
$etlPath = Join-Path $resolvedOutputDir "$prefix.etl"
$startedPath = Join-Path $resolvedOutputDir "$prefix.started.txt"
$donePath = Join-Path $resolvedOutputDir "$prefix.done.txt"

$principal = New-Object Security.Principal.WindowsPrincipal(
    [Security.Principal.WindowsIdentity]::GetCurrent()
)
$isAdmin = $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)

function Write-LogLine {
    param([string]$Line)
    $Line | Add-Content -Path $logPath -Encoding UTF8
}

try {
    @(
        "timestamp=$timestamp"
        "workspace=$workspace"
        "is_admin=$isAdmin"
        "duration_seconds=$DurationSeconds"
        "scenario=$Scenario"
        "etl=$etlPath"
        "log=$logPath"
        "wpr_path=$((Get-Command wpr -ErrorAction SilentlyContinue).Source)"
        ""
    ) | Set-Content -Path $logPath -Encoding UTF8

    Write-LogLine "starting WPR CPU filemode capture"
    & wpr -start CPU -filemode *>> $logPath
    $startExit = $LASTEXITCODE
    if ($startExit -ne 0) {
        throw "wpr -start CPU -filemode failed with exit code $startExit"
    }

    @(
        "started_at=$(Get-Date -Format o)"
        "etl=$etlPath"
        "log=$logPath"
        "duration_seconds=$DurationSeconds"
    ) | Set-Content -Path $startedPath -Encoding UTF8

    Start-Sleep -Seconds $DurationSeconds

    Write-LogLine "stopping WPR capture"
    & wpr -stop $etlPath *>> $logPath
    $stopExit = $LASTEXITCODE

    $etlExists = Test-Path $etlPath
    $etlLength = if ($etlExists) { (Get-Item $etlPath).Length } else { 0 }

    @(
        "exit_code=$stopExit"
        "etl_exists=$etlExists"
        "etl_length=$etlLength"
        "etl=$etlPath"
        "log=$logPath"
    ) | Set-Content -Path $donePath -Encoding UTF8

    exit $stopExit
}
catch {
    $message = $_.Exception.Message
    Write-LogLine "error=$message"

    try {
        Write-LogLine "attempting WPR cancel"
        & wpr -cancel *>> $logPath
    }
    catch {
        Write-LogLine "wpr cancel failed: $($_.Exception.Message)"
    }

    @(
        "exit_code=exception"
        "error=$message"
        "etl=$etlPath"
        "log=$logPath"
    ) | Set-Content -Path $donePath -Encoding UTF8

    exit 1
}
