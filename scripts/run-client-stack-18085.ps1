param(
    [int]$WorldPort = 18085,
    [int]$AuthPort = 13724,
    [string]$WorldSqlPath = $env:CMANGOS_WORLD_SQL,
    [switch]$ResetWorldDatabase
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

function Import-MariaDbSqlFile {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Database,
        [Parameter(Mandatory = $true)]
        [string]$Path,
        [Parameter(Mandatory = $true)]
        [string]$Description
    )

    $resolved = Resolve-Path $Path
    Write-Host "Importing $Description from $resolved"
    $processInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $processInfo.FileName = "docker"
    $processInfo.Arguments = "exec -i cmangos-rust-realmd mariadb -uroot -proot $Database"
    $processInfo.RedirectStandardInput = $true
    $processInfo.RedirectStandardOutput = $true
    $processInfo.RedirectStandardError = $true
    $processInfo.UseShellExecute = $false

    $process = [System.Diagnostics.Process]::Start($processInfo)
    $input = [System.IO.File]::OpenRead($resolved)
    $sqlStream = $input
    if ($resolved.Path.EndsWith(".gz", [System.StringComparison]::OrdinalIgnoreCase)) {
        $sqlStream = [System.IO.Compression.GZipStream]::new($input, [System.IO.Compression.CompressionMode]::Decompress)
    }

    try {
        $sqlStream.CopyTo($process.StandardInput.BaseStream)
        $process.StandardInput.Close()
        $stdout = $process.StandardOutput.ReadToEnd()
        $stderr = $process.StandardError.ReadToEnd()
        $process.WaitForExit()
        if ($process.ExitCode -ne 0) {
            if ($stdout) { Write-Host $stdout }
            if ($stderr) { Write-Host $stderr }
            throw "failed to import $Description from $resolved"
        }
    }
    finally {
        $sqlStream.Dispose()
        if (-not [object]::ReferenceEquals($sqlStream, $input)) {
            $input.Dispose()
        }
    }
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

if ($ResetWorldDatabase) {
    Invoke-MariaDb "" "DROP DATABASE IF EXISTS mangos; CREATE DATABASE mangos DEFAULT CHARACTER SET utf8 COLLATE utf8_general_ci; GRANT ALL PRIVILEGES ON mangos.* TO 'mangos'@'%'; FLUSH PRIVILEGES;"
}

$characterTableCount = docker exec cmangos-rust-realmd mariadb -uroot -proot -N -B -e "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema='characters' AND table_name='characters';"
if ($LASTEXITCODE -ne 0) {
    throw "failed to check characters schema"
}

if (($characterTableCount | Select-Object -First 1).Trim() -eq "0") {
    $charactersSql = Join-Path $repoRoot "sql\base\characters.sql"
    Import-MariaDbSqlFile "characters" $charactersSql "sql/base/characters.sql"
}

$worldTableCount = docker exec cmangos-rust-realmd mariadb -uroot -proot -N -B -e "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema='mangos' AND table_name='playercreateinfo';"
if ($LASTEXITCODE -ne 0) {
    throw "failed to check mangos schema"
}

if (($worldTableCount | Select-Object -First 1).Trim() -eq "0") {
    $mangosSql = Join-Path $repoRoot "sql\base\mangos.sql"
    Import-MariaDbSqlFile "mangos" $mangosSql "sql/base/mangos.sql"
}

if ($WorldSqlPath) {
    Import-MariaDbSqlFile "mangos" $WorldSqlPath "full CMaNGOS world SQL"
}

$sql = "UPDATE realmlist SET address='127.0.0.1', port=$WorldPort WHERE id=1;"
Invoke-Checked docker @("exec", "cmangos-rust-realmd", "mariadb", "-umangos", "-pmangos", "realmd", "-e", $sql)

$seedCharacterSql = @"
DROP TEMPORARY TABLE IF EXISTS rust_client_account_chars;
CREATE TEMPORARY TABLE rust_client_account_chars
    SELECT guid
    FROM characters.characters
    WHERE account = (SELECT id FROM realmd.account WHERE username = 'RUSTAUTH');

DELETE FROM characters.character_account_data WHERE guid IN (SELECT guid FROM rust_client_account_chars);
DELETE FROM characters.character_action WHERE guid IN (SELECT guid FROM rust_client_account_chars);
DELETE FROM characters.character_aura WHERE guid IN (SELECT guid FROM rust_client_account_chars);
DELETE FROM characters.character_battleground_data WHERE guid IN (SELECT guid FROM rust_client_account_chars);
DELETE FROM characters.character_gifts WHERE guid IN (SELECT guid FROM rust_client_account_chars);
DELETE FROM characters.character_homebind WHERE guid IN (SELECT guid FROM rust_client_account_chars);
DELETE FROM characters.character_honor_cp WHERE guid IN (SELECT guid FROM rust_client_account_chars);
DELETE FROM characters.character_instance WHERE guid IN (SELECT guid FROM rust_client_account_chars);
DELETE FROM characters.character_inventory WHERE guid IN (SELECT guid FROM rust_client_account_chars);
DELETE FROM characters.character_pet WHERE owner IN (SELECT guid FROM rust_client_account_chars);
DELETE FROM characters.character_queststatus WHERE guid IN (SELECT guid FROM rust_client_account_chars);
DELETE FROM characters.character_queststatus_weekly WHERE guid IN (SELECT guid FROM rust_client_account_chars);
DELETE FROM characters.character_reputation WHERE guid IN (SELECT guid FROM rust_client_account_chars);
DELETE FROM characters.character_skills WHERE guid IN (SELECT guid FROM rust_client_account_chars);
DELETE FROM characters.character_social WHERE guid IN (SELECT guid FROM rust_client_account_chars);
DELETE FROM characters.character_spell WHERE guid IN (SELECT guid FROM rust_client_account_chars);
DELETE FROM characters.character_spell_cooldown WHERE guid IN (SELECT guid FROM rust_client_account_chars);
DELETE FROM characters.character_stats WHERE guid IN (SELECT guid FROM rust_client_account_chars);
DELETE FROM characters.character_tutorial WHERE account = (SELECT id FROM realmd.account WHERE username = 'RUSTAUTH');
DELETE FROM characters.mail_items WHERE receiver IN (SELECT guid FROM rust_client_account_chars);
DELETE FROM characters.mail WHERE receiver IN (SELECT guid FROM rust_client_account_chars);
DELETE FROM characters.item_instance WHERE owner_guid IN (SELECT guid FROM rust_client_account_chars);
DELETE FROM characters.characters WHERE guid IN (SELECT guid FROM rust_client_account_chars);

INSERT INTO characters.characters
    (guid, account, name, race, class, gender, level, zone, map, position_x, position_y, position_z, playerBytes, playerBytes2, equipmentCache)
SELECT 1, id, 'Rustone', 1, 1, 0, 1, 12, 0, -8949.95, -132.493, 83.5312, 0, 0, ''
FROM realmd.account
WHERE username = 'RUSTAUTH'
ON DUPLICATE KEY UPDATE account = VALUES(account), name = VALUES(name);

INSERT INTO realmd.realmcharacters (realmid, acctid, numchars)
SELECT 1, id, 1 FROM realmd.account WHERE username = 'RUSTAUTH'
ON DUPLICATE KEY UPDATE numchars = VALUES(numchars);

DROP TEMPORARY TABLE rust_client_account_chars;
"@
Invoke-MariaDb "characters" $seedCharacterSql

$seedCreatureSql = @"
DROP TEMPORARY TABLE IF EXISTS rust_client_creature_template;
CREATE TEMPORARY TABLE rust_client_creature_template LIKE mangos.creature_template;
INSERT INTO rust_client_creature_template
SELECT * FROM mangos.creature_template WHERE Entry = 1 LIMIT 1;
UPDATE rust_client_creature_template
SET Entry = 900010,
    Name = 'Rust DB Guide',
    SubName = 'DB Spawn',
    MinLevel = 1,
    MaxLevel = 1,
    DisplayId1 = 49,
    DisplayId2 = 0,
    DisplayId3 = 0,
    DisplayId4 = 0,
    Faction = 35,
    Scale = 1,
    NpcFlags = 5,
    UnitFlags = 0,
    DynamicFlags = 0,
    MinLevelHealth = 42,
    MaxLevelHealth = 42,
    MinMeleeDmg = 1,
    MaxMeleeDmg = 2,
    MeleeBaseAttackTime = 2000,
    RangedBaseAttackTime = 2000;
DELETE FROM mangos.creature WHERE guid = 900010;
DELETE FROM mangos.creature_template WHERE Entry = 900010;
DELETE FROM mangos.npc_vendor WHERE entry = 900010 AND item IN (117, 2102);
INSERT INTO mangos.creature_template SELECT * FROM rust_client_creature_template;
INSERT INTO mangos.npc_vendor (entry, item, maxcount, incrtime, slot, condition_id)
VALUES (900010, 117, 0, 0, 1, 0);
INSERT INTO mangos.creature
    (guid, id, map, spawnMask, position_x, position_y, position_z, orientation,
     spawntimesecsmin, spawntimesecsmax, spawndist, MovementType)
SELECT 900010, 900010, map, 1, position_x + 6, position_y - 2, position_z, orientation,
       120, 120, 0, 0
FROM characters.characters
WHERE name = 'Rustone'
LIMIT 1;
DROP TEMPORARY TABLE rust_client_creature_template;
"@
Invoke-MariaDb "mangos" $seedCreatureSql

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
