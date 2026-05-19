param(
    [Parameter(Mandatory = $true)]
    [int]$PidToProfile,

    [string]$OutputDir = "logs\perf-rca",

    [string]$Scenario = "worldserver-pid-attach"
)

$ErrorActionPreference = "Stop"

$workspace = Split-Path -Parent (Split-Path -Parent $PSCommandPath)
Set-Location $workspace

$resolvedOutputDir = Join-Path $workspace $OutputDir
New-Item -ItemType Directory -Force -Path $resolvedOutputDir | Out-Null

$timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
$safeScenario = $Scenario -replace '[^A-Za-z0-9_.-]', '-'
$prefix = "$timestamp-$safeScenario-pid-$PidToProfile"
$logPath = Join-Path $resolvedOutputDir "$prefix.log"
$svgPath = Join-Path $resolvedOutputDir "$prefix.svg"
$donePath = Join-Path $resolvedOutputDir "$prefix.done.txt"

$principal = New-Object Security.Principal.WindowsPrincipal(
    [Security.Principal.WindowsIdentity]::GetCurrent()
)
$isAdmin = $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)

try {
    @(
        "timestamp=$timestamp"
        "workspace=$workspace"
        "is_admin=$isAdmin"
        "pid=$PidToProfile"
        "scenario=$Scenario"
        "svg=$svgPath"
        "log=$logPath"
        "flamegraph_path=$((Get-Command flamegraph -ErrorAction SilentlyContinue).Source)"
        ""
    ) | Set-Content -Path $logPath -Encoding UTF8

    & flamegraph `
        --verbose `
        --pid $PidToProfile `
        --output $svgPath `
        --title "$Scenario pid $PidToProfile" `
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
