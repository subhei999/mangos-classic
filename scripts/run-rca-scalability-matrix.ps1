param(
    [ValidateSet("Smoke", "Core", "JitterOnly", "RateKnee", "Full")]
    [string]$Preset = "Core",
    [int[]]$PlayerCounts = @(50, 100, 250, 500),
    [int]$HoldSeconds = 60,
    [int]$LoginBootstrapTimeoutSeconds = 30,
    [int]$LoginReadyTimeoutSeconds = 60,
    [int]$MoveIntervalMs = 50,
    [int]$LoginStaggerMs = 1,
    [int]$ReadyTimeoutSeconds = 300,
    [int]$CaptureDelaySeconds = 10,
    [int]$SentinelCastClients = 5,
    [int]$SentinelCastSpellId = 168,
    [int]$SentinelCastIntervalMs = 5000,
    [int]$SentinelCastPhaseJitterMs = 5000,
    [string]$MetricsUrl = "http://127.0.0.1:9091/metrics",
    [string]$OutDir = "logs\perf-rca"
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$startScript = Join-Path $PSScriptRoot "start-thin-client-load.ps1"
$captureScript = Join-Path $PSScriptRoot "capture-rca-metrics.ps1"
$outputRoot = Join-Path $repoRoot $OutDir
$matrixId = Get-Date -Format "yyyyMMdd-HHmmss"
$matrixDir = Join-Path $outputRoot "matrix-$matrixId"
New-Item -ItemType Directory -Force -Path $matrixDir | Out-Null

function Get-ScenarioSet {
    param([string]$Name)

    $core = @(
        [pscustomobject]@{
            Name = "idle-same-grid"
            SpawnMode = "local_radius"
            MovePhaseJitterMs = 0
            DisableMovement = $true
            DisableSentinelMovement = $false
        },
        [pscustomobject]@{
            Name = "movement-same-grid-sync"
            SpawnMode = "local_radius"
            MovePhaseJitterMs = 0
            DisableMovement = $false
            DisableSentinelMovement = $true
        },
        [pscustomobject]@{
            Name = "movement-same-grid-jitter250"
            SpawnMode = "local_radius"
            MovePhaseJitterMs = 250
            DisableMovement = $false
            DisableSentinelMovement = $true
        },
        [pscustomobject]@{
            Name = "movement-spread-sync"
            SpawnMode = "creature_grid_scatter"
            MovePhaseJitterMs = 0
            DisableMovement = $false
            DisableSentinelMovement = $true
        }
    )

    switch ($Name) {
        "Smoke" { return @($core[1]) }
        "JitterOnly" { return @($core[1], $core[2]) }
        "RateKnee" {
            return @(
                [pscustomobject]@{
                    Name = "movement-same-grid-slow250"
                    SpawnMode = "local_radius"
                    MovePhaseJitterMs = 0
                    MoveIntervalMs = 250
                    DisableMovement = $false
                    DisableSentinelMovement = $true
                },
                [pscustomobject]@{
                    Name = "movement-same-grid-slow500"
                    SpawnMode = "local_radius"
                    MovePhaseJitterMs = 0
                    MoveIntervalMs = 500
                    DisableMovement = $false
                    DisableSentinelMovement = $true
                }
            )
        }
        "Core" { return $core }
        "Full" {
            return $core + @(
                [pscustomobject]@{
                    Name = "idle-spread"
                    SpawnMode = "creature_grid_scatter"
                    MovePhaseJitterMs = 0
                    DisableMovement = $true
                    DisableSentinelMovement = $false
                },
                [pscustomobject]@{
                    Name = "movement-spread-jitter250"
                    SpawnMode = "creature_grid_scatter"
                    MovePhaseJitterMs = 250
                    DisableMovement = $false
                    DisableSentinelMovement = $true
                },
                [pscustomobject]@{
                    Name = "movement-same-grid-slow250"
                    SpawnMode = "local_radius"
                    MovePhaseJitterMs = 0
                    MoveIntervalMs = 250
                    DisableMovement = $false
                    DisableSentinelMovement = $true
                }
            )
        }
    }
}

function Get-MetricSnapshot {
    param([string]$Url)

    $body = [string](Invoke-WebRequest -Uri $Url -UseBasicParsing -TimeoutSec 3).Content
    $sessionsMatch = [regex]::Match($body, '(?m)^wow_world_sessions_connected\s+(\d+)')
    $sessions = if ($sessionsMatch.Success) { [int]$sessionsMatch.Groups[1].Value } else { 0 }
    $players = 0
    foreach ($match in [regex]::Matches($body, '(?m)^wow_map_active_players\{[^\n]*\}\s+(\d+)')) {
        $players += [int]$match.Groups[1].Value
    }

    [pscustomobject]@{
        Sessions = $sessions
        ActivePlayers = $players
    }
}

function Wait-ForSteadyState {
    param(
        [int]$ClientCount,
        [string]$Url,
        [int]$TimeoutSeconds
    )

    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    $last = [pscustomobject]@{ Sessions = 0; ActivePlayers = 0 }
    while ((Get-Date) -lt $deadline) {
        try {
            $last = Get-MetricSnapshot -Url $Url
            if ($last.Sessions -ge $ClientCount -and $last.ActivePlayers -ge $ClientCount) {
                return [pscustomobject]@{
                    Ready = $true
                    Sessions = $last.Sessions
                    ActivePlayers = $last.ActivePlayers
                }
            }
        }
        catch {
            Write-Host "Metrics not ready yet: $($_.Exception.Message)"
        }
        Start-Sleep -Seconds 5
    }

    [pscustomobject]@{
        Ready = $false
        Sessions = $last.Sessions
        ActivePlayers = $last.ActivePlayers
    }
}

function ConvertTo-SwitchText {
    param(
        [string]$Name,
        [bool]$Enabled
    )

    if ($Enabled) {
        return " -$Name"
    }

    return ""
}

$scenarios = @(Get-ScenarioSet -Name $Preset)
if ($Preset -eq "Smoke" -and $PlayerCounts.Count -gt 1) {
    $PlayerCounts = @($PlayerCounts[-1])
}

$resultsPath = Join-Path $matrixDir "matrix-results.csv"
$summaryPath = Join-Path $matrixDir "matrix-summary.md"
$results = New-Object System.Collections.Generic.List[object]

Write-Host "RCA matrix id: $matrixId"
Write-Host "Preset: $Preset"
Write-Host "Player counts: $($PlayerCounts -join ', ')"
Write-Host "Scenarios: $($scenarios.Name -join ', ')"
Write-Host "Output: $matrixDir"

$totalRuns = $PlayerCounts.Count * $scenarios.Count
$runIndex = 0

foreach ($count in $PlayerCounts) {
    foreach ($scenario in $scenarios) {
        $runIndex += 1
        $scenarioMoveIntervalMs = if ($scenario.PSObject.Properties.Name -contains "MoveIntervalMs") {
            [int]$scenario.MoveIntervalMs
        }
        else {
            $MoveIntervalMs
        }
        $sentinelCount = [Math]::Min($SentinelCastClients, $count)
        $scenarioName = "matrix-$matrixId-$($scenario.Name)-players$count-move$scenarioMoveIntervalMs-jitter$($scenario.MovePhaseJitterMs)"
        $stdout = Join-Path $matrixDir "$scenarioName.stdout.log"
        $stderr = Join-Path $matrixDir "$scenarioName.stderr.log"
        $disableMovementText = ConvertTo-SwitchText -Name "DisableMovement" -Enabled ([bool]$scenario.DisableMovement)
        $disableSentinelMovementText = ConvertTo-SwitchText -Name "DisableSentinelMovement" -Enabled ([bool]$scenario.DisableSentinelMovement)

        $command = @"
& '$startScript' -ClientCount $count -SpawnMode $($scenario.SpawnMode) -MoveIntervalMs $scenarioMoveIntervalMs -MovePhaseJitterMs $($scenario.MovePhaseJitterMs) -LoginStaggerMs $LoginStaggerMs -HoldSeconds $HoldSeconds -LoginBootstrapTimeoutSeconds $LoginBootstrapTimeoutSeconds -LoginReadyTimeoutSeconds $LoginReadyTimeoutSeconds -CharacterClass 8 -Race 1 -SentinelCastClients $sentinelCount -SentinelCastSpellId $SentinelCastSpellId -SentinelCastIntervalMs $SentinelCastIntervalMs -SentinelCastPhaseJitterMs $SentinelCastPhaseJitterMs -EnableMovementActor -EnableTokioUnstableMetrics$disableMovementText$disableSentinelMovementText
"@

        Write-Host "[$runIndex/$totalRuns] Starting $scenarioName"
        $startedAt = Get-Date
        $process = Start-Process -FilePath "powershell" `
            -ArgumentList @("-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", $command) `
            -WorkingDirectory $repoRoot `
            -RedirectStandardOutput $stdout `
            -RedirectStandardError $stderr `
            -PassThru

        $steady = Wait-ForSteadyState -ClientCount $count -Url $MetricsUrl -TimeoutSeconds $ReadyTimeoutSeconds
        if ($steady.Ready) {
            Write-Host "[$runIndex/$totalRuns] Steady state reached: sessions=$($steady.Sessions), active_players=$($steady.ActivePlayers)"
            Start-Sleep -Seconds $CaptureDelaySeconds
        }
        else {
            Write-Host "[$runIndex/$totalRuns] Steady state timeout: sessions=$($steady.Sessions), active_players=$($steady.ActivePlayers)"
        }

        $captureOutput = @()
        $captureSucceeded = $false
        try {
            $captureOutput = & powershell -NoProfile -ExecutionPolicy Bypass -File $captureScript `
                -MetricsUrl $MetricsUrl `
                -OutDir $OutDir `
                -Scenario $scenarioName
            $captureSucceeded = $true
        }
        catch {
            $captureOutput = @("Capture failed: $($_.Exception.Message)")
        }
        $rawMetricsPath = ($captureOutput | Select-String -Pattern '^Raw metrics:\s+(.*)$' | Select-Object -First 1).Matches.Groups[1].Value
        $summaryMetricsPath = ($captureOutput | Select-String -Pattern '^Summary metrics:\s+(.*)$' | Select-Object -First 1).Matches.Groups[1].Value
        $metadataPath = ($captureOutput | Select-String -Pattern '^Metadata:\s+(.*)$' | Select-Object -First 1).Matches.Groups[1].Value

        Wait-Process -Id $process.Id
        $process.Refresh()
        $finishedAt = Get-Date
        $exitCode = $process.ExitCode
        $stdoutText = if (Test-Path $stdout) { Get-Content -Path $stdout -Raw } else { "" }
        $stderrText = if (Test-Path $stderr) { Get-Content -Path $stderr -Raw } else { "" }
        if ($null -eq $exitCode -or $exitCode -eq "") {
            $exitMatch = [regex]::Match($stderrText, 'exit code:\s+(\d+)')
            if ($exitMatch.Success) {
                $exitCode = [int]$exitMatch.Groups[1].Value
            }
            elseif ($stderrText -match 'Error:\s+\d+\s+client\(s\) failed') {
                $exitCode = 1
            }
            else {
                $exitCode = 0
            }
        }
        $finishedLine = ([regex]::Match($stdoutText, 'world-load-test finished[^\r\n]*')).Value
        $sentinelLine = ([regex]::Match($stdoutText, 'sentinel-cast summary:[^\r\n]*')).Value
        $clientFailureCount = ([regex]::Matches($stderrText, 'client failure:')).Count

        $result = [pscustomobject]@{
            MatrixId = $matrixId
            Scenario = $scenario.Name
            ScenarioName = $scenarioName
            ClientCount = $count
            SpawnMode = $scenario.SpawnMode
            MoveIntervalMs = $scenarioMoveIntervalMs
            MovePhaseJitterMs = $scenario.MovePhaseJitterMs
            DisableMovement = [bool]$scenario.DisableMovement
            DisableSentinelMovement = [bool]$scenario.DisableSentinelMovement
            Ready = [bool]$steady.Ready
            SessionsAtCapture = $steady.Sessions
            ActivePlayersAtCapture = $steady.ActivePlayers
            CaptureSucceeded = $captureSucceeded
            ExitCode = $exitCode
            ClientFailureCount = $clientFailureCount
            StartedAt = $startedAt.ToString("o")
            FinishedAt = $finishedAt.ToString("o")
            DurationSeconds = [Math]::Round(($finishedAt - $startedAt).TotalSeconds, 3)
            FinishedLine = $finishedLine
            SentinelLine = $sentinelLine
            RawMetricsPath = $rawMetricsPath
            SummaryMetricsPath = $summaryMetricsPath
            MetadataPath = $metadataPath
            StdoutPath = $stdout
            StderrPath = $stderr
        }
        $results.Add($result)
        $results | Export-Csv -Path $resultsPath -NoTypeInformation

        Write-Host "[$runIndex/$totalRuns] Finished exit=$exitCode failures=$clientFailureCount"
        if ($sentinelLine) {
            Write-Host "[$runIndex/$totalRuns] $sentinelLine"
        }
    }
}

$summary = @(
    "# RCA Scalability Matrix $matrixId",
    "",
    "- Preset: $Preset",
    "- Player counts: $($PlayerCounts -join ', ')",
    "- Hold seconds: $HoldSeconds",
    "- Move interval ms: $MoveIntervalMs",
    "- Results CSV: $resultsPath",
    "",
    "## Runs",
    ""
)

foreach ($result in $results) {
    $summary += "- ``$($result.ScenarioName)``: ready=$($result.Ready), exit=$($result.ExitCode), failures=$($result.ClientFailureCount), sentinel=``$($result.SentinelLine)``"
}

Set-Content -Path $summaryPath -Value ($summary -join [Environment]::NewLine) -Encoding utf8

Write-Host "Matrix results: $resultsPath"
Write-Host "Matrix summary: $summaryPath"
