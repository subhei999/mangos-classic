param(
    [int]$WorldPort = 18085,
    [int]$AuthPort = 13724,
    [int]$ReadyTimeoutSeconds = 90,
    [string]$WorldConfigPath = "config\\worldserver.local.toml",
    [switch]$ResetWorldDatabase,
    [switch]$ResetCharacters,
    [switch]$SeedLegacyRustFixtures,
    [switch]$EnablePlayerbots,
    [int]$PlayerbotRandomCount = -1,
    [switch]$Release,
    [switch]$Help
)

$ErrorActionPreference = "Stop"

function Show-Usage {
    Write-Host "usage: .\scripts\restart-game-stack.cmd [options]"
    Write-Host "       .\scripts\restart-game-stack.cmd -Release"
    Write-Host ""
    Write-Host "Restarts the local Rust authserver, worldserver, and worldserver observability dashboard."
    Write-Host ""
    Write-Host "Options:"
    Write-Host "  -WorldPort <port>             Worldserver port. Default: 18085"
    Write-Host "  -AuthPort <port>              Authserver port. Default: 13724"
    Write-Host "  -ReadyTimeoutSeconds <secs>   Time to wait for ports after launch. Default: 90"
    Write-Host "  -WorldConfigPath <path>       Worldserver config file. Default: config\\worldserver.local.toml"
    Write-Host "  -ResetWorldDatabase           Recreate the local mangos world database"
    Write-Host "  -ResetCharacters              Recreate the RUSTAUTH starter character"
    Write-Host "  -SeedLegacyRustFixtures       Add old Rust fixture NPCs for debugging"
    Write-Host "  -EnablePlayerbots             Opt into configured local playerbots"
    Write-Host "  -PlayerbotRandomCount <count> Enable generated random playerbots with this count"
    Write-Host "  -Release                      Build and launch target\\release binaries"
    Write-Host "                                cmd aliases: release, --release, /release"
    Write-Host "  -Help                         Show this help"
}

function Stop-GameStackProcesses {
    $processes = @(Get-Process authserver,worldserver -ErrorAction SilentlyContinue)
    if ($processes.Count -eq 0) {
        return
    }

    Write-Host "Stopping existing authserver/worldserver processes before build."
    $processes | Stop-Process -Force
    foreach ($process in $processes) {
        try {
            Wait-Process -Id $process.Id -Timeout 10 -ErrorAction SilentlyContinue
        }
        catch {
            Write-Warning "Timed out waiting for process $($process.Id) to exit."
        }
    }
}

function Wait-ForFileUnlock {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path,
        [int]$TimeoutSeconds = 30
    )

    if (-not (Test-Path $Path)) {
        return
    }

    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    do {
        try {
            $stream = [System.IO.File]::Open($Path, "Open", "ReadWrite", "None")
            $stream.Close()
            return
        }
        catch {
            Start-Sleep -Milliseconds 250
        }
    } while ((Get-Date) -lt $deadline)

    throw "Timed out waiting for $Path to be released. Close any terminal still running that binary and retry."
}

function Wait-ForStackBinaryUnlock {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Profile
    )

    Wait-ForFileUnlock (Join-Path $repoRoot "target\$Profile\authserver.exe")
    Wait-ForFileUnlock (Join-Path $repoRoot "target\$Profile\worldserver.exe")
}

function Test-TcpPort {
    param(
        [Parameter(Mandatory = $true)]
        [string]$HostName,
        [Parameter(Mandatory = $true)]
        [int]$Port
    )

    $client = [System.Net.Sockets.TcpClient]::new()
    try {
        $connect = $client.BeginConnect($HostName, $Port, $null, $null)
        if (-not $connect.AsyncWaitHandle.WaitOne(1000)) {
            return $false
        }
        $client.EndConnect($connect)
        return $true
    }
    catch {
        return $false
    }
    finally {
        $client.Close()
    }
}

function Wait-ForTcpPort {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Name,
        [Parameter(Mandatory = $true)]
        [int]$Port,
        [Parameter(Mandatory = $true)]
        [int]$TimeoutSeconds
    )

    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    do {
        if (Test-TcpPort "127.0.0.1" $Port) {
            Write-Host "$Name is listening on 127.0.0.1:$Port"
            return
        }
        Start-Sleep -Milliseconds 500
    } while ((Get-Date) -lt $deadline)

    throw "$Name did not start listening on 127.0.0.1:$Port within $TimeoutSeconds second(s)."
}

if ($Help) {
    Show-Usage
    exit 0
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$stackScript = Join-Path $PSScriptRoot "run-client-stack-18085.ps1"
$buildProfile = if ($Release) { "release" } else { "debug" }

Stop-GameStackProcesses
Wait-ForStackBinaryUnlock $buildProfile

$stackArgs = @(
    "-WorldPort", $WorldPort,
    "-AuthPort", $AuthPort,
    "-WorldConfigPath", $WorldConfigPath,
    "-NoAutoRestart"
)

if ($ResetWorldDatabase) {
    $stackArgs += "-ResetWorldDatabase"
}
if ($ResetCharacters) {
    $stackArgs += "-ResetCharacters"
}
if ($SeedLegacyRustFixtures) {
    $stackArgs += "-SeedLegacyRustFixtures"
}
if ($EnablePlayerbots) {
    $stackArgs += "-EnablePlayerbots"
}
if ($PlayerbotRandomCount -ge 0) {
    $stackArgs += @("-PlayerbotRandomCount", $PlayerbotRandomCount)
}
if ($Release) {
    $stackArgs += "-Release"
}

Write-Host "Restarting Rust game stack from $repoRoot"
& powershell -NoProfile -ExecutionPolicy Bypass -File $stackScript @stackArgs
if ($LASTEXITCODE -ne 0) {
    throw "run-client-stack-18085.ps1 failed with exit code $LASTEXITCODE"
}

Wait-ForTcpPort "Authserver" $AuthPort $ReadyTimeoutSeconds
Wait-ForTcpPort "Worldserver" $WorldPort $ReadyTimeoutSeconds
Wait-ForTcpPort "Observability dashboard" 9091 $ReadyTimeoutSeconds

Write-Host ""
Write-Host "Game stack is ready."
Write-Host "WoW realmlist.wtf: set realmlist 127.0.0.1:$AuthPort"
Write-Host "Worldserver: 127.0.0.1:$WorldPort"
Write-Host "Observability: http://127.0.0.1:9091/dashboard"
