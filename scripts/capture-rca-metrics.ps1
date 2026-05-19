param(
    [string]$MetricsUrl = "http://127.0.0.1:9091/metrics",
    [string]$OutDir = "logs\perf-rca",
    [string]$Scenario = "manual",
    [switch]$OpenDashboard
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$outputRoot = Join-Path $repoRoot $OutDir
New-Item -ItemType Directory -Force -Path $outputRoot | Out-Null

$timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
$safeScenario = [System.Text.RegularExpressions.Regex]::Replace($Scenario, "[^A-Za-z0-9_.-]+", "-").Trim("-")
if ([string]::IsNullOrWhiteSpace($safeScenario)) {
    $safeScenario = "manual"
}

$baseName = "$timestamp-$safeScenario"
$rawPath = Join-Path $outputRoot "$baseName.metrics.prom"
$summaryPath = Join-Path $outputRoot "$baseName.summary.prom"
$metadataPath = Join-Path $outputRoot "$baseName.metadata.md"

Write-Host "Capturing metrics from $MetricsUrl"
$response = Invoke-WebRequest -Uri $MetricsUrl -UseBasicParsing
$metricsBody = [string]$response.Content
Set-Content -Path $rawPath -Value $metricsBody -Encoding utf8

$prefixes = @(
    "wow_world_sessions_connected",
    "wow_map_active_players",
    "wow_map_loaded_grids",
    "wow_map_tick_duration",
    "wow_map_tick_lag",
    "wow_map_phase_duration",
    "wow_map_tracked_idle_motion_creatures",
    "wow_map_tracked_idle_motion_start_candidates",
    "wow_world_packet_dispatch_delay",
    "wow_world_packet_handler_duration",
    "wow_world_packet_service_time",
    "wow_world_packet_outbound_queue_latency",
    "wow_world_packet_write_duration",
    "wow_world_packet_outbound_enqueued_bytes",
    "wow_world_packet_write_bytes",
    "wow_world_outbound_fanout",
    "wow_world_outbound_source",
    "wow_world_outbound_queue",
    "wow_world_position_status",
    "wow_world_geometry",
    "wow_world_session_loop_phase_duration",
    "wow_channel_queue_age",
    "wow_channel_queue_depth",
    "wow_channel_send_wait",
    "wow_tokio",
    "wow_db",
    "wow_movement_actor",
    "wow_movement_map_mutex",
    "wow_movement_apply",
    "wow_player_visibility_refresh",
    "wow_idle_motion"
)

$summaryLines = $metricsBody -split "`n" | Where-Object {
    $line = $_.Trim()
    if ($line.Length -eq 0 -or $line.StartsWith("#")) {
        return $false
    }
    foreach ($prefix in $prefixes) {
        if ($line.StartsWith($prefix)) {
            return $true
        }
    }
    return $false
}

Set-Content -Path $summaryPath -Value ($summaryLines -join [Environment]::NewLine) -Encoding utf8

Push-Location $repoRoot
try {
    $gitStatus = git status --short --branch
    $gitCommit = git rev-parse HEAD
}
finally {
    Pop-Location
}

$runtimeEnvironment = @(
    "RUSTFLAGS=$env:RUSTFLAGS",
    "RUST_LOG=$env:RUST_LOG",
    "PROCESSOR_IDENTIFIER=$env:PROCESSOR_IDENTIFIER",
    "NUMBER_OF_PROCESSORS=$env:NUMBER_OF_PROCESSORS",
    "PowerShell=$($PSVersionTable.PSVersion)"
)

try {
    $os = Get-CimInstance Win32_OperatingSystem
    $cpu = Get-CimInstance Win32_Processor | Select-Object -First 1
    $runtimeEnvironment += "OS=$($os.Caption) $($os.Version)"
    $runtimeEnvironment += "CPU=$($cpu.Name); cores=$($cpu.NumberOfCores); logical=$($cpu.NumberOfLogicalProcessors)"
}
catch {
    $runtimeEnvironment += "SystemInfoError=$($_.Exception.Message)"
}

$interestingProcesses = @()
try {
    $interestingProcesses = Get-CimInstance Win32_Process |
        Where-Object {
            $_.CommandLine -and
            ($_.CommandLine -match "authserver|worldserver|world-load-test|start-thin-client-load|cargo.*world-load-test")
        } |
        Sort-Object ProcessId |
        ForEach-Object {
            "PID=$($_.ProcessId) Name=$($_.Name) CommandLine=$($_.CommandLine)"
        }
}
catch {
    $interestingProcesses = @("ProcessCaptureError=$($_.Exception.Message)")
}
if ($interestingProcesses.Count -eq 0) {
    $interestingProcesses = @("(no matching process command lines found at capture time)")
}

$worldConfigSnippets = @()
try {
    $processRows = Get-CimInstance Win32_Process |
        Where-Object { $_.CommandLine -and $_.CommandLine -match "worldserver" }
    foreach ($processRow in $processRows) {
        if ($processRow.CommandLine -match '--config\s+(?:"(?<quoted>[^"]+)"|(?<plain>\S+))') {
            $configPath = if ($Matches["quoted"]) { $Matches["quoted"] } else { $Matches["plain"] }
            if ($configPath -and -not [System.IO.Path]::IsPathRooted($configPath)) {
                $configPath = Join-Path $repoRoot $configPath
            }
            if ($configPath -and (Test-Path $configPath)) {
                $worldConfigSnippets += "### $configPath"
                $worldConfigSnippets += (
                    Get-Content -Path $configPath |
                        Where-Object {
                            $_ -match '^\s*(\[|bind_|port|enabled|map_update_interval_ms|visibility|experimental_movement_actor|playerbot|random_|db_|log)'
                        }
                )
            }
        }
    }
}
catch {
    $worldConfigSnippets = @("WorldConfigCaptureError=$($_.Exception.Message)")
}
if ($worldConfigSnippets.Count -eq 0) {
    $worldConfigSnippets = @("(no worldserver --config file discovered at capture time)")
}

$quickMetricPrefixes = @(
    "wow_world_sessions_connected",
    "wow_map_active_players",
    "wow_map_tick_duration_latest_milliseconds",
    "wow_map_tick_lag_latest_milliseconds",
    "wow_tokio_runtime_workers",
    "wow_tokio_task_count",
    "wow_tokio_worker_busy_milliseconds",
    "wow_tokio_runtime_global_queue_depth",
    "wow_tokio_task_poll_duration_milliseconds",
    "wow_tokio_spawn_blocking_queue_depth"
)
$quickMetricLines = $metricsBody -split "`n" | Where-Object {
    $line = $_.Trim()
    if ($line.Length -eq 0 -or $line.StartsWith("#")) {
        return $false
    }
    foreach ($prefix in $quickMetricPrefixes) {
        if ($line.StartsWith($prefix)) {
            return $true
        }
    }
    return $false
}
if ($quickMetricLines.Count -eq 0) {
    $quickMetricLines = @("(no quick baseline metrics matched)")
}

$metadata = @(
    "# RCA Metrics Capture",
    "",
    "- Timestamp: $(Get-Date -Format o)",
    "- Scenario: $Scenario",
    "- Metrics URL: $MetricsUrl",
    "- Raw metrics: $rawPath",
    "- Summary metrics: $summaryPath",
    "- Git commit: $gitCommit",
    "",
    "## Git Status",
    "",
    '```text',
    ($gitStatus -join [Environment]::NewLine),
    '```',
    "",
    "## Runtime Environment",
    "",
    '```text',
    ($runtimeEnvironment -join [Environment]::NewLine),
    '```',
    "",
    "## Process Command Lines",
    "",
    '```text',
    ($interestingProcesses -join [Environment]::NewLine),
    '```',
    "",
    "## World Config Snippets",
    "",
    '```toml',
    ($worldConfigSnippets -join [Environment]::NewLine),
    '```',
    "",
    "## Quick Baseline Metrics",
    "",
    '```text',
    ($quickMetricLines -join [Environment]::NewLine),
    '```',
    "",
    "## Notes",
    "",
    "- Scenario naming should still include client count, spawn mode, movement interval, jitter, actor state, sentinel count, spell id, and scrape timing.",
    "- Runtime poll/blocking-pool metrics with `wow_tokio_task_*` and `wow_tokio_spawn_blocking_*` require the server to be built with `RUSTFLAGS=--cfg tokio_unstable`."
)
Set-Content -Path $metadataPath -Value ($metadata -join [Environment]::NewLine) -Encoding utf8

Write-Host "Raw metrics:     $rawPath"
Write-Host "Summary metrics: $summaryPath"
Write-Host "Metadata:        $metadataPath"

if ($OpenDashboard) {
    Start-Process "http://127.0.0.1:9091/dashboard"
}
