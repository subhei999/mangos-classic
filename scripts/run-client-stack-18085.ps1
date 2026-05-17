param(
    [int]$WorldPort = 18085,
    [int]$AuthPort = 13724,
    [string]$WorldConfigPath = "config\\worldserver.local.toml",
    [string]$WorldSqlPath = $env:CMANGOS_WORLD_SQL,
    [switch]$ResetWorldDatabase,
    [switch]$ResetCharacters,
    [switch]$NoAutoRestart,
    [switch]$SeedLegacyRustFixtures,
    [switch]$EnablePlayerbots,
    [int]$PlayerbotRandomCount = -1,
    [switch]$Release,
    [int]$RestartDelaySeconds = 2
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

function Start-StackProcess {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Name,
        [Parameter(Mandatory = $true)]
        [string]$ProcessName,
        [Parameter(Mandatory = $true)]
        [string]$Command,
        [Parameter(Mandatory = $true)]
        [string]$LogPath
    )

    $timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    Add-Content -Path $LogPath -Value ""
    Add-Content -Path $LogPath -Value "[$timestamp] starting $Name"
    $launcher = Start-Process -FilePath "cmd.exe" -ArgumentList "/c", $Command -WorkingDirectory $repoRoot -WindowStyle Hidden -PassThru

    $deadline = (Get-Date).AddSeconds(10)
    do {
        $process = Get-Process $ProcessName -ErrorAction SilentlyContinue |
            Where-Object { $_.StartTime -ge $launcher.StartTime } |
            Sort-Object StartTime -Descending |
            Select-Object -First 1
        if ($process) {
            return $process
        }
        Start-Sleep -Milliseconds 200
    } while ((Get-Date) -lt $deadline)

    return $launcher
}

function Stop-StackProcess {
    param(
        [AllowNull()]
        [System.Diagnostics.Process]$Process,
        [string[]]$ProcessNames = @("authserver", "worldserver")
    )

    if ($Process -and -not $Process.HasExited) {
        Stop-Process -Id $Process.Id -Force -ErrorAction SilentlyContinue
    }

    Get-Process $ProcessNames -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
Set-Location $repoRoot

Write-Host "Stopping any running Rust authserver/worldserver before restart."
Stop-StackProcess -Process $null
Start-Sleep -Milliseconds 500

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

if ($ResetCharacters) {
    Write-Host "Resetting RUSTAUTH characters and recreating Rustone."
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
SELECT
    CASE WHEN MAX(CASE WHEN c.guid = 1 THEN 1 ELSE 0 END) = 0 THEN 1 ELSE COALESCE(MAX(c.guid), 0) + 1 END,
    a.id, 'Rustone', 1, 1, 0, 1, 12, 0, -8949.95, -132.493, 83.5312, 0, 0, ''
FROM realmd.account a
LEFT JOIN characters.characters c ON TRUE
WHERE a.username = 'RUSTAUTH'
GROUP BY a.id;

DROP TEMPORARY TABLE rust_client_account_chars;
"@
    Invoke-MariaDb "characters" $seedCharacterSql
}
else {
    Write-Host "Preserving RUSTAUTH characters; seeding Rustone only if the account is empty."
    $seedCharacterSql = @"
INSERT INTO characters.characters
    (guid, account, name, race, class, gender, level, zone, map, position_x, position_y, position_z, playerBytes, playerBytes2, equipmentCache)
SELECT
    CASE WHEN MAX(CASE WHEN c.guid = 1 THEN 1 ELSE 0 END) = 0 THEN 1 ELSE COALESCE(MAX(c.guid), 0) + 1 END,
    a.id, 'Rustone', 1, 1, 0, 1, 12, 0, -8949.95, -132.493, 83.5312, 0, 0, ''
FROM realmd.account a
LEFT JOIN characters.characters c ON TRUE
WHERE a.username = 'RUSTAUTH'
  AND NOT EXISTS (
      SELECT 1
      FROM characters.characters account_characters
      WHERE account_characters.account = a.id
  )
GROUP BY a.id;
"@
    Invoke-MariaDb "characters" $seedCharacterSql
}

$realmCharacterCountSql = @"
INSERT INTO realmd.realmcharacters (realmid, acctid, numchars)
SELECT 1, a.id, COUNT(c.guid)
FROM realmd.account a
LEFT JOIN characters.characters c ON c.account = a.id
WHERE a.username = 'RUSTAUTH'
GROUP BY a.id
ON DUPLICATE KEY UPDATE numchars = VALUES(numchars);
"@
Invoke-MariaDb "characters" $realmCharacterCountSql

$backfillStarterSkillsSql = @"
INSERT IGNORE INTO characters.character_skills (guid, skill, value, max)
SELECT
    c.guid,
    pcs.skill,
    CASE
        WHEN pcs.note LIKE 'Language:%' THEN 300
        ELSE 1
    END AS value,
    CASE
        WHEN pcs.note LIKE 'Language:%' THEN 300
        WHEN pcs.note LIKE 'Misc: GENERIC%' THEN 1
        WHEN pcs.note LIKE 'Armor:%' THEN 1
        WHEN pcs.note LIKE 'Racial:%' THEN 1
        ELSE 5
    END AS max
FROM characters.characters c
JOIN realmd.account a ON a.id = c.account
JOIN mangos.playercreateinfo_skills pcs
  ON (pcs.raceMask = 0 OR (pcs.raceMask & (1 << (c.race - 1))) <> 0)
 AND (pcs.classMask = 0 OR (pcs.classMask & (1 << (c.class - 1))) <> 0)
WHERE a.username = 'RUSTAUTH';
"@
Invoke-MariaDb "" $backfillStarterSkillsSql

if ($SeedLegacyRustFixtures) {
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
FROM characters.characters c
WHERE c.account = (SELECT id FROM realmd.account WHERE username = 'RUSTAUTH')
ORDER BY CASE WHEN c.name = 'Rustone' THEN 0 ELSE 1 END, c.guid
LIMIT 1;
DROP TEMPORARY TABLE rust_client_creature_template;
"@
    Invoke-MariaDb "mangos" $seedCreatureSql
} else {
    $removeLegacyCreatureSql = @"
DELETE FROM mangos.creature WHERE guid = 900010;
DELETE FROM mangos.npc_vendor WHERE entry = 900010 AND item IN (117, 2102);
DELETE FROM mangos.creature_template WHERE Entry = 900010;
"@
    Invoke-MariaDb "mangos" $removeLegacyCreatureSql
}

$removeStaleHarnessCreatureSql = @"
DELETE FROM mangos.creature WHERE guid IN (96001, 910907);
DELETE FROM mangos.creature_template WHERE Entry = 910007;
"@
Invoke-MariaDb "mangos" $removeStaleHarnessCreatureSql

$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
$cargoBuildArgs = @("build")
if ($Release) {
    $cargoBuildArgs += "--release"
}
Invoke-Checked cargo ($cargoBuildArgs + @("-p", "authserver"))
Invoke-Checked cargo ($cargoBuildArgs + @("-p", "worldserver"))

Get-Process authserver,worldserver -ErrorAction SilentlyContinue | Stop-Process -Force

$authLog = Join-Path $repoRoot "auth-client-$AuthPort.log"
$worldLog = Join-Path $repoRoot "world-client-$WorldPort.log"
Remove-Item $authLog, $worldLog -ErrorAction SilentlyContinue

$buildProfile = if ($Release) { "release" } else { "debug" }
$resolvedWorldConfigPath = Resolve-Path $WorldConfigPath
$authCmd = "set `"RUST_LOG=info`" && target\$buildProfile\authserver.exe --config config\authserver.local.toml >> `"$authLog`" 2>&1"
$worldCmd = "set `"RUST_LOG=info`" && set `"WORLD_BIND_PORT=$WorldPort`""
if ($EnablePlayerbots -or $PlayerbotRandomCount -ge 0) {
    $worldCmd += " && set `"WORLD_PLAYERBOTS__ENABLED=true`""
    if ($PlayerbotRandomCount -ge 0) {
        $worldCmd += " && set `"WORLD_PLAYERBOTS__RANDOM__ENABLED=true`""
        $worldCmd += " && set `"WORLD_PLAYERBOTS__RANDOM__COUNT=$PlayerbotRandomCount`""
    }
} else {
    $worldCmd += " && set `"WORLD_PLAYERBOTS__ENABLED=false`""
    $worldCmd += " && set `"WORLD_PLAYERBOTS__RANDOM__ENABLED=false`""
    $worldCmd += " && set `"WORLD_PLAYERBOTS__RANDOM__COUNT=0`""
}
if ($SeedLegacyRustFixtures) {
    $worldCmd += " && set `"WORLD_ENABLE_LEGACY_FIXTURE_NPCS=1`""
}
$worldCmd += " && target\$buildProfile\worldserver.exe --config `"$resolvedWorldConfigPath`" >> `"$worldLog`" 2>&1"

$auth = Start-StackProcess "authserver" "authserver" $authCmd $authLog
$world = Start-StackProcess "worldserver" "worldserver" $worldCmd $worldLog

Start-Sleep -Seconds 2

Write-Host "Authserver process: $($auth.Id), log: $authLog"
Write-Host "Worldserver process: $($world.Id), log: $worldLog"
Write-Host "WoW realmlist.wtf: set realmlist 127.0.0.1:$AuthPort"
Write-Host "Realm row points to 127.0.0.1:$WorldPort"

if ($NoAutoRestart) {
    Write-Host "Auto-restart disabled; leaving started processes running."
    return
}

Write-Host "Auto-restart supervisor is running. Press Ctrl+C to stop both servers."

try {
    while ($true) {
        if ($auth.HasExited) {
            Write-Host "Authserver exited with code $($auth.ExitCode); restarting in $RestartDelaySeconds second(s)."
            Start-Sleep -Seconds $RestartDelaySeconds
            $auth = Start-StackProcess "authserver" "authserver" $authCmd $authLog
            Write-Host "Authserver restarted as process $($auth.Id), log: $authLog"
        }

        if ($world.HasExited) {
            Write-Host "Worldserver exited with code $($world.ExitCode); restarting in $RestartDelaySeconds second(s)."
            Start-Sleep -Seconds $RestartDelaySeconds
            $world = Start-StackProcess "worldserver" "worldserver" $worldCmd $worldLog
            Write-Host "Worldserver restarted as process $($world.Id), log: $worldLog"
        }

        Start-Sleep -Seconds 1
    }
}
finally {
    Write-Host "Stopping client stack processes."
    Stop-StackProcess $auth
    Stop-StackProcess $world
}
