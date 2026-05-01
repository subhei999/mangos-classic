param(
    [int]$WorldPort = 18085,
    [int]$AuthPort = 13724
)

$ErrorActionPreference = "Stop"

function Invoke-Checked {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Command,
        [string[]]$Arguments
    )

    & $Command @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "$Command $($Arguments -join ' ') failed with exit code $LASTEXITCODE"
    }
}

function Invoke-MariaDb {
    param(
        [string]$Database,
        [Parameter(Mandatory = $true)]
        [string]$Sql
    )

    $arguments = @("exec", "cmangos-rust-realmd", "mariadb", "-uroot", "-proot")
    if ($Database) {
        $arguments += $Database
    }
    $arguments += @("-e", $Sql)
    Invoke-Checked docker $arguments
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
Set-Location $repoRoot

if (-not (Get-Command docker -ErrorAction SilentlyContinue)) {
    throw "docker was not found on PATH. Install/start Docker Desktop first."
}

$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "cargo was not found on PATH. Install Rust with rustup, then reopen this terminal."
}

Invoke-Checked docker @("compose", "-f", "docker-compose.local.yml", "up", "-d", "realmd")

$deadline = (Get-Date).AddMinutes(2)
do {
    $status = docker inspect --format "{{.State.Health.Status}}" cmangos-rust-realmd 2>$null
    if ($status -eq "healthy") {
        break
    }
    Start-Sleep -Seconds 3
} while ((Get-Date) -lt $deadline)

if ($status -ne "healthy") {
    throw "MariaDB did not become healthy in time. Run 'docker logs cmangos-rust-realmd' for details."
}

Invoke-MariaDb "" "CREATE DATABASE IF NOT EXISTS characters DEFAULT CHARACTER SET utf8 COLLATE utf8_general_ci;"
Invoke-MariaDb "" "CREATE DATABASE IF NOT EXISTS mangos DEFAULT CHARACTER SET utf8 COLLATE utf8_general_ci;"
Invoke-MariaDb "" "GRANT ALL PRIVILEGES ON characters.* TO 'mangos'@'%'; FLUSH PRIVILEGES;"
Invoke-MariaDb "" "GRANT ALL PRIVILEGES ON mangos.* TO 'mangos'@'%'; FLUSH PRIVILEGES;"

$characterTableCount = docker exec cmangos-rust-realmd mariadb -uroot -proot -N -B -e "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema='characters' AND table_name='characters';"
if ($LASTEXITCODE -ne 0) {
    throw "failed to check characters schema"
}

if (($characterTableCount | Select-Object -First 1).Trim() -eq "0") {
    $charactersSql = Join-Path $repoRoot "sql\base\characters.sql"
    & cmd.exe /c "docker exec -i cmangos-rust-realmd mariadb -uroot -proot characters < `"$charactersSql`""
    if ($LASTEXITCODE -ne 0) {
        throw "failed to import sql/base/characters.sql"
    }
}

$worldTableCount = docker exec cmangos-rust-realmd mariadb -uroot -proot -N -B -e "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema='mangos' AND table_name='playercreateinfo';"
if ($LASTEXITCODE -ne 0) {
    throw "failed to check mangos schema"
}

if (($worldTableCount | Select-Object -First 1).Trim() -eq "0") {
    $mangosSql = Join-Path $repoRoot "sql\base\mangos.sql"
    & cmd.exe /c "docker exec -i cmangos-rust-realmd mariadb -uroot -proot mangos < `"$mangosSql`""
    if ($LASTEXITCODE -ne 0) {
        throw "failed to import sql/base/mangos.sql"
    }
}

Invoke-MariaDb "realmd" "UPDATE realmlist SET address='127.0.0.1', port=$WorldPort WHERE id=1;"

Invoke-Checked cargo @("build", "-p", "authserver")
Invoke-Checked cargo @("build", "-p", "worldserver")
Invoke-Checked cargo @("build", "-p", "world-flow-test")

Get-Process authserver,worldserver -ErrorAction SilentlyContinue | Stop-Process -Force

$authLog = Join-Path $repoRoot "auth-world-flow-$AuthPort.log"
$worldLog = Join-Path $repoRoot "world-flow-$WorldPort.log"

$authCmd = "set `"RUST_LOG=info`" && target\debug\authserver.exe --config config\authserver.local.toml > `"$authLog`" 2>&1"
$worldCmd = "set `"RUST_LOG=info`" && set `"WORLD_BIND_PORT=$WorldPort`" && set `"WORLD_ENABLE_LEGACY_FIXTURE_NPCS=1`" && target\debug\worldserver.exe --config config\worldserver.local.toml > `"$worldLog`" 2>&1"

$auth = Start-Process -FilePath "cmd.exe" -ArgumentList "/c", $authCmd -WorkingDirectory $repoRoot -WindowStyle Hidden -PassThru
$world = Start-Process -FilePath "cmd.exe" -ArgumentList "/c", $worldCmd -WorkingDirectory $repoRoot -WindowStyle Hidden -PassThru

try {
    Start-Sleep -Seconds 2

    if ($auth.HasExited) {
        throw "authserver exited during startup with code $($auth.ExitCode); see $authLog"
    }
    if ($world.HasExited) {
        throw "worldserver exited during startup with code $($world.ExitCode); see $worldLog"
    }

    Invoke-Checked cargo @("run", "-p", "world-flow-test")
}
finally {
    Get-Process -Id $auth.Id,$world.Id -ErrorAction SilentlyContinue | Stop-Process -Force
    Get-Process authserver,worldserver -ErrorAction SilentlyContinue | Stop-Process -Force
}
