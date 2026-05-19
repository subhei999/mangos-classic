param(
    [int]$ClientCount = 500,
    [int]$WorldPort = 18085,
    [int]$AuthPort = 13724,
    [int]$ReadyTimeoutSeconds = 180,
    [int]$HoldSeconds = 60,
    [int]$LoginBootstrapTimeoutSeconds = 15,
    [int]$LoginReadyTimeoutSeconds = 30,
    [int]$MoveIntervalMs = 500,
    [int]$MovePhaseJitterMs = 0,
    [int]$LoginStaggerMs = 25,
    [int]$MaxAttempts = 3,
    [double]$CenterX = -8949.0,
    [double]$CenterY = -132.0,
    [double]$CenterZ = 83.5,
    [double]$Radius = 150.0,
    [ValidateSet("local_radius", "creature_grid_scatter")]
    [string]$SpawnMode = "local_radius",
    [double]$MoveRadius = 6.0,
    [int]$Race = 1,
    [int]$CharacterClass = 1,
    [int]$Gender = 0,
    [int]$SentinelCastClients = 0,
    [int]$SentinelCastSpellId = 168,
    [int]$SentinelCastIntervalMs = 5000,
    [int]$SentinelCastPhaseJitterMs = 0,
    [int]$ClientThreadStackKb = 1024,
    [string]$WorldConfigPath = "config\\worldserver.local.toml",
    [switch]$EnableMovementActor,
    [switch]$DisableMovement,
    [switch]$DisableSentinelMovement,
    [switch]$EnableTokioUnstableMetrics,
    [int]$MovementActorQueueCapacity = 1024,
    [int]$MovementActorMaxBatchSize = 64,
    [switch]$SeedOnly
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$restartScript = Join-Path $PSScriptRoot "restart-game-stack.ps1"
$baseWorldConfigPath = Join-Path $repoRoot $WorldConfigPath

if ($EnableTokioUnstableMetrics) {
    $existingRustFlags = $env:RUSTFLAGS
    if ([string]::IsNullOrWhiteSpace($existingRustFlags)) {
        $env:RUSTFLAGS = "--cfg tokio_unstable"
    }
    elseif ($existingRustFlags -notmatch "(^|\s)--cfg\s+tokio_unstable(\s|$)") {
        $env:RUSTFLAGS = "$existingRustFlags --cfg tokio_unstable"
    }
    Write-Host "Tokio unstable runtime metrics enabled for this run with RUSTFLAGS=$env:RUSTFLAGS"
}

function New-BenchmarkWorldConfig {
    param(
        [Parameter(Mandatory = $true)]
        [string]$BasePath,
        [Parameter(Mandatory = $true)]
        [bool]$MovementActorEnabled,
        [Parameter(Mandatory = $true)]
        [int]$QueueCapacity,
        [Parameter(Mandatory = $true)]
        [int]$MaxBatchSize
    )

    $content = Get-Content -Path $BasePath -Raw
    $content = [System.Text.RegularExpressions.Regex]::Replace(
        $content,
        "(?m)^experimental_movement_actor\s*=.*\r?\n?",
        ""
    )
    $content = [System.Text.RegularExpressions.Regex]::Replace(
        $content,
        "(?m)^experimental_movement_actor_queue_capacity\s*=.*\r?\n?",
        ""
    )
    $content = [System.Text.RegularExpressions.Regex]::Replace(
        $content,
        "(?m)^experimental_movement_actor_max_batch_size\s*=.*\r?\n?",
        ""
    )

    $worldHeader = "[world]"
    $worldHeaderIndex = $content.IndexOf($worldHeader)
    if ($worldHeaderIndex -lt 0) {
        throw "Failed to find [world] section in $BasePath"
    }
    $insertIndex = $worldHeaderIndex + $worldHeader.Length
    $movementActorSettings = @"

experimental_movement_actor = $($MovementActorEnabled.ToString().ToLowerInvariant())
experimental_movement_actor_queue_capacity = $QueueCapacity
experimental_movement_actor_max_batch_size = $MaxBatchSize
"@
    $content = $content.Insert($insertIndex, $movementActorSettings)

    $generatedPath = Join-Path $env:TEMP ("worldserver.thin-client." + [Guid]::NewGuid().ToString("N") + ".toml")
    Set-Content -Path $generatedPath -Value $content
    return $generatedPath
}

$effectiveWorldConfigPath = New-BenchmarkWorldConfig `
    -BasePath $baseWorldConfigPath `
    -MovementActorEnabled:$EnableMovementActor `
    -QueueCapacity $MovementActorQueueCapacity `
    -MaxBatchSize $MovementActorMaxBatchSize

Write-Host "Starting release auth/world stack for thin-client load test from $repoRoot"
& powershell -NoProfile -ExecutionPolicy Bypass -File $restartScript `
    -WorldPort $WorldPort `
    -AuthPort $AuthPort `
    -ReadyTimeoutSeconds $ReadyTimeoutSeconds `
    -WorldConfigPath $effectiveWorldConfigPath `
    -Release

if ($LASTEXITCODE -ne 0) {
    throw "restart-game-stack.ps1 failed with exit code $LASTEXITCODE"
}

$cargoArgs = @(
    "run",
    "--release",
    "-p",
    "world-load-test",
    "--",
    "--client-count", $ClientCount,
    "--hold-seconds", $HoldSeconds,
    "--login-bootstrap-timeout-secs", $LoginBootstrapTimeoutSeconds,
    "--login-ready-timeout-secs", $LoginReadyTimeoutSeconds,
    "--move-interval-ms", $MoveIntervalMs,
    "--move-phase-jitter-ms", $MovePhaseJitterMs,
    "--login-stagger-ms", $LoginStaggerMs,
    "--max-attempts", $MaxAttempts,
    "--auth-addr", "127.0.0.1:$AuthPort",
    "--world-addr", "127.0.0.1:$WorldPort",
    "--spawn-mode", $SpawnMode,
    "--center-x", [string]$CenterX,
    "--center-y", [string]$CenterY,
    "--center-z", [string]$CenterZ,
    "--radius", [string]$Radius,
    "--move-radius", [string]$MoveRadius,
    "--race", $Race,
    "--class", $CharacterClass,
    "--gender", $Gender,
    "--sentinel-cast-clients", $SentinelCastClients,
    "--sentinel-cast-spell-id", $SentinelCastSpellId,
    "--sentinel-cast-interval-ms", $SentinelCastIntervalMs,
    "--sentinel-cast-phase-jitter-ms", $SentinelCastPhaseJitterMs,
    "--client-thread-stack-kb", $ClientThreadStackKb
)

if ($SeedOnly) {
    $cargoArgs += "--seed-only"
}

if ($DisableMovement) {
    $cargoArgs += "--disable-movement"
}

if ($DisableSentinelMovement) {
    $cargoArgs += "--disable-sentinel-movement"
}

Write-Host "Launching thin-client load test: clients=$ClientCount hold=${HoldSeconds}s move_interval=${MoveIntervalMs}ms phase_jitter=${MovePhaseJitterMs}ms stagger=${LoginStaggerMs}ms sentinel_cast_clients=$SentinelCastClients sentinel_spell=$SentinelCastSpellId sentinel_phase_jitter=${SentinelCastPhaseJitterMs}ms client_thread_stack_kb=$ClientThreadStackKb disable_movement=$DisableMovement disable_sentinel_movement=$DisableSentinelMovement tokio_unstable_metrics=$EnableTokioUnstableMetrics"
& cargo @cargoArgs
