param(
    [int]$ClientCount = 500,
    [int]$WorldPort = 18085,
    [int]$AuthPort = 13724,
    [int]$ReadyTimeoutSeconds = 180,
    [int]$HoldSeconds = 60,
    [int]$MoveIntervalMs = 500,
    [int]$LoginStaggerMs = 25,
    [int]$MaxAttempts = 3,
    [double]$CenterX = -8949.0,
    [double]$CenterY = -132.0,
    [double]$CenterZ = 83.5,
    [double]$Radius = 150.0,
    [double]$MoveRadius = 6.0,
    [switch]$SeedOnly
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$restartScript = Join-Path $PSScriptRoot "restart-game-stack.ps1"

Write-Host "Starting release auth/world stack for thin-client load test from $repoRoot"
& powershell -NoProfile -ExecutionPolicy Bypass -File $restartScript `
    -WorldPort $WorldPort `
    -AuthPort $AuthPort `
    -ReadyTimeoutSeconds $ReadyTimeoutSeconds `
    -WorldConfigPath "config\\worldserver.local.toml" `
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
    "--move-interval-ms", $MoveIntervalMs,
    "--login-stagger-ms", $LoginStaggerMs,
    "--max-attempts", $MaxAttempts,
    "--auth-addr", "127.0.0.1:$AuthPort",
    "--world-addr", "127.0.0.1:$WorldPort",
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
