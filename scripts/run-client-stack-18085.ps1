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

$sql = "UPDATE realmlist SET address='127.0.0.1', port=$WorldPort WHERE id=1;"
Invoke-Checked docker @("exec", "cmangos-rust-realmd", "mariadb", "-umangos", "-pmangos", "realmd", "-e", $sql)

$seedCharacterSql = @"
INSERT INTO characters.characters
    (guid, account, name, race, class, gender, level, zone, map, position_x, position_y, position_z, playerBytes, playerBytes2, equipmentCache)
SELECT 1, id, 'Rustone', 1, 1, 0, 1, 12, 0, -8949.95, -132.493, 83.5312, 0, 0, ''
FROM realmd.account
WHERE username = 'RUSTAUTH'
ON DUPLICATE KEY UPDATE account = VALUES(account), name = VALUES(name);

INSERT INTO realmd.realmcharacters (realmid, acctid, numchars)
SELECT 1, id, 1 FROM realmd.account WHERE username = 'RUSTAUTH'
ON DUPLICATE KEY UPDATE numchars = VALUES(numchars);
"@
Invoke-MariaDb "" $seedCharacterSql

$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
Invoke-Checked cargo @("build", "-p", "authserver")
Invoke-Checked cargo @("build", "-p", "worldserver")

Get-Process authserver,worldserver -ErrorAction SilentlyContinue | Stop-Process -Force

$authLog = Join-Path $repoRoot "auth-client-$AuthPort.log"
$worldLog = Join-Path $repoRoot "world-client-$WorldPort.log"

$authCmd = "set `"RUST_LOG=info`" && target\debug\authserver.exe --config config\authserver.local.toml > `"$authLog`" 2>&1"
$worldCmd = "set `"RUST_LOG=info`" && set `"WORLD_BIND_PORT=$WorldPort`" && target\debug\worldserver.exe --config config\worldserver.local.toml > `"$worldLog`" 2>&1"

$auth = Start-Process -FilePath "cmd.exe" -ArgumentList "/c", $authCmd -WorkingDirectory $repoRoot -WindowStyle Hidden -PassThru
$world = Start-Process -FilePath "cmd.exe" -ArgumentList "/c", $worldCmd -WorkingDirectory $repoRoot -WindowStyle Hidden -PassThru

Start-Sleep -Seconds 2

Write-Host "Authserver process: $($auth.Id), log: $authLog"
Write-Host "Worldserver process: $($world.Id), log: $worldLog"
Write-Host "WoW realmlist.wtf: set realmlist 127.0.0.1:$AuthPort"
Write-Host "Realm row points to 127.0.0.1:$WorldPort"
