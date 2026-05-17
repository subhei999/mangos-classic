param(
    [int]$BotCount = 500,
    [int]$WorldPort = 18085,
    [int]$AuthPort = 13724,
    [int]$ReadyTimeoutSeconds = 180,
    [string]$Distribution = "radius",
    [double]$CenterX = -8949.0,
    [double]$CenterY = -132.0,
    [double]$CenterZ = 83.5,
    [double]$Radius = 1000.0,
    [switch]$ResetWorldDatabase,
    [switch]$ResetCharacters
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$restartScript = Join-Path $PSScriptRoot "restart-game-stack.ps1"

$savedEnv = @{
    "WORLD_PLAYERBOTS__COMBAT_ENABLED" = $env:WORLD_PLAYERBOTS__COMBAT_ENABLED
    "WORLD_PLAYERBOTS__LOCAL_ROAM_ONLY" = $env:WORLD_PLAYERBOTS__LOCAL_ROAM_ONLY
    "WORLD_PLAYERBOTS__FORCE_ACTIVE" = $env:WORLD_PLAYERBOTS__FORCE_ACTIVE
    "WORLD_PLAYERBOTS__RANDOM__DISTRIBUTION" = $env:WORLD_PLAYERBOTS__RANDOM__DISTRIBUTION
    "WORLD_PLAYERBOTS__RANDOM__NAME_PREFIX" = $env:WORLD_PLAYERBOTS__RANDOM__NAME_PREFIX
    "WORLD_PLAYERBOTS__RANDOM__MAP" = $env:WORLD_PLAYERBOTS__RANDOM__MAP
    "WORLD_PLAYERBOTS__RANDOM__CENTER_X" = $env:WORLD_PLAYERBOTS__RANDOM__CENTER_X
    "WORLD_PLAYERBOTS__RANDOM__CENTER_Y" = $env:WORLD_PLAYERBOTS__RANDOM__CENTER_Y
    "WORLD_PLAYERBOTS__RANDOM__CENTER_Z" = $env:WORLD_PLAYERBOTS__RANDOM__CENTER_Z
    "WORLD_PLAYERBOTS__RANDOM__SEED" = $env:WORLD_PLAYERBOTS__RANDOM__SEED
}

try {
    $env:WORLD_PLAYERBOTS__COMBAT_ENABLED = "false"
    $env:WORLD_PLAYERBOTS__LOCAL_ROAM_ONLY = "true"
    $env:WORLD_PLAYERBOTS__FORCE_ACTIVE = "true"
    $env:WORLD_PLAYERBOTS__RANDOM__DISTRIBUTION = $Distribution
    $env:WORLD_PLAYERBOTS__RANDOM__NAME_PREFIX = "Perfbot"
    $env:WORLD_PLAYERBOTS__RANDOM__MAP = "0"
    $env:WORLD_PLAYERBOTS__RANDOM__CENTER_X = [string]$CenterX
    $env:WORLD_PLAYERBOTS__RANDOM__CENTER_Y = [string]$CenterY
    $env:WORLD_PLAYERBOTS__RANDOM__CENTER_Z = [string]$CenterZ
    $env:WORLD_PLAYERBOTS__RANDOM__RADIUS = [string]$Radius
    $env:WORLD_PLAYERBOTS__RANDOM__SEED = "500"

    $restartArgs = @(
        "-WorldPort", $WorldPort,
        "-AuthPort", $AuthPort,
        "-ReadyTimeoutSeconds", $ReadyTimeoutSeconds,
        "-WorldConfigPath", "config\\worldserver.perf.toml",
        "-EnablePlayerbots",
        "-PlayerbotRandomCount", $BotCount,
        "-Release"
    )
    if ($ResetWorldDatabase) {
        $restartArgs += "-ResetWorldDatabase"
    }
    if ($ResetCharacters) {
        $restartArgs += "-ResetCharacters"
    }

    Write-Host "Starting release idle-playerbot perf stack from $repoRoot"
    Write-Host "Scenario: $BotCount force-active idle bots, map 0, distribution=$Distribution, center=($CenterX,$CenterY,$CenterZ), radius=$Radius, combat disabled"
    & powershell -NoProfile -ExecutionPolicy Bypass -File $restartScript @restartArgs
    if ($LASTEXITCODE -ne 0) {
        throw "restart-game-stack.ps1 failed with exit code $LASTEXITCODE"
    }
}
finally {
    foreach ($entry in $savedEnv.GetEnumerator()) {
        if ($null -eq $entry.Value) {
            Remove-Item "Env:$($entry.Key)" -ErrorAction SilentlyContinue
        } else {
            Set-Item "Env:$($entry.Key)" $entry.Value
        }
    }
}
