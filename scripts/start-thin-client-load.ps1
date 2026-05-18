param(
    [int]$ClientCount = 500,
    [int]$WorldPort = 18085,
    [int]$AuthPort = 13724,
    [int]$ReadyTimeoutSeconds = 180,
    [int]$HoldSeconds = 60,
    [int]$LoginBootstrapTimeoutSeconds = 15,
    [int]$LoginReadyTimeoutSeconds = 30,
    [int]$MoveIntervalMs = 500,
    [int]$LoginStaggerMs = 25,
    [int]$MaxAttempts = 3,
    [double]$CenterX = -8949.0,
    [double]$CenterY = -132.0,
    [double]$CenterZ = 83.5,
    [double]$Radius = 150.0,
    [ValidateSet("local_radius", "creature_grid_scatter")]
    [string]$SpawnMode = "local_radius",
    [double]$MoveRadius = 6.0,
    [string]$WorldConfigPath = "config\\worldserver.local.toml",
    [switch]$EnableMovementActor,
    [int]$MovementActorQueueCapacity = 1024,
    [int]$MovementActorMaxBatchSize = 64,
    [switch]$SeedOnly
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$restartScript = Join-Path $PSScriptRoot "restart-game-stack.ps1"
$baseWorldConfigPath = Join-Path $repoRoot $WorldConfigPath

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
    "--login-stagger-ms", $LoginStaggerMs,
    "--max-attempts", $MaxAttempts,
    "--auth-addr", "127.0.0.1:$AuthPort",
    "--world-addr", "127.0.0.1:$WorldPort",
    "--spawn-mode", $SpawnMode,
    "--center-x", [string]$CenterX,
    "--center-y", [string]$CenterY,
    "--center-z", [string]$CenterZ,
    "--radius", [string]$Radius,
    "--move-radius", [string]$MoveRadius
)

if ($SeedOnly) {
    $cargoArgs += "--seed-only"
}

Write-Host "Launching thin-client load test: clients=$ClientCount hold=${HoldSeconds}s move_interval=${MoveIntervalMs}ms stagger=${LoginStaggerMs}ms"
& cargo @cargoArgs
