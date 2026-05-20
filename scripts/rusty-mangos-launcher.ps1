param(
    [ValidateSet("InstallStart", "Install", "Configure", "Start", "Stop", "Restart", "Status")]
    [string]$Action = "InstallStart",
    [ValidateSet("Native", "Docker")]
    [string]$DatabaseMode = "Native",
    [string]$ClientDir,
    [string]$ClassicDbPath,
    [int]$DbPort = 3307,
    [int]$WorldPort = 18085,
    [int]$AuthPort = 13724,
    [int]$ReadyTimeoutSeconds = 120,
    [string]$MariaDbVersion = "11.4.8",
    [string]$MariaDbZipUrl,
    [switch]$SkipWorldImport,
    [switch]$ForceWorldImport,
    [switch]$NoClassicDbClone,
    [switch]$NoRealmlistUpdate,
    [switch]$ResetCharacters,
    [switch]$DebugBuild,
    [switch]$Help
)

$ErrorActionPreference = "Stop"

function Show-Usage {
    Write-Host "usage: .\scripts\rusty-mangos-launcher.cmd [Action] [options]"
    Write-Host ""
    Write-Host "Actions:"
    Write-Host "  InstallStart   Configure/install everything, then start the server. Default."
    Write-Host "  Install        Configure/install everything, but do not start auth/world."
    Write-Host "  Configure      Re-prompt for client/config and rewrite launcher config."
    Write-Host "  Start          Start MariaDB, authserver, and worldserver."
    Write-Host "  Stop           Stop launcher-managed MariaDB/auth/world processes."
    Write-Host "  Restart        Stop, then start."
    Write-Host "  Status         Print process and port status."
    Write-Host ""
    Write-Host "Options:"
    Write-Host "  -ClientDir <path>          WoW 1.12.1 client folder. Prompts when omitted."
    Write-Host "  -ClassicDbPath <path>      classic-db checkout. Default: target\classic-db"
    Write-Host "  -DbPort <port>             Local MariaDB port. Default: 3307"
    Write-Host "  -WorldPort <port>          Worldserver port. Default: 18085"
    Write-Host "  -AuthPort <port>           Authserver port. Default: 13724"
    Write-Host "  -SkipWorldImport           Do not clone/import ClassicDB."
    Write-Host "  -ForceWorldImport          Re-import ClassicDB even when world data exists."
    Write-Host "  -NoClassicDbClone          Require ClassicDB to already exist locally."
    Write-Host "  -NoRealmlistUpdate         Do not edit the client's realmlist.wtf."
    Write-Host "  -ResetCharacters           Reset only the seeded RUSTAUTH characters."
    Write-Host "  -DebugBuild                Build/run debug binaries instead of release."
    Write-Host ""
    Write-Host "Native portable MariaDB is the default backend. Docker is intentionally not"
    Write-Host "required for the normal player flow."
}

function Write-Step {
    param([Parameter(Mandatory = $true)][string]$Message)
    Write-Host ""
    Write-Host "==> $Message" -ForegroundColor Cyan
}

function Invoke-Checked {
    param(
        [Parameter(Mandatory = $true)][string]$Command,
        [string[]]$Arguments,
        [string]$WorkingDirectory
    )

    $oldLocation = Get-Location
    if ($WorkingDirectory) {
        Set-Location $WorkingDirectory
    }
    try {
        & $Command @Arguments
        if ($LASTEXITCODE -ne 0) {
            throw "$Command $($Arguments -join ' ') failed with exit code $LASTEXITCODE"
        }
    }
    finally {
        Set-Location $oldLocation
    }
}

function Quote-ProcessArgument {
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string]$Argument)

    if ($Argument -notmatch '[\s"]') {
        return $Argument
    }

    return '"' + ($Argument.Replace('"', '\"')) + '"'
}

function Join-ProcessArguments {
    param([string[]]$Arguments)

    return (($Arguments | ForEach-Object { Quote-ProcessArgument $_ }) -join " ")
}

function Require-Command {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][string]$InstallHint
    )

    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "$Name was not found on PATH. $InstallHint"
    }
}

function Select-FolderWithDialog {
    param([string]$Description)

    try {
        Add-Type -AssemblyName System.Windows.Forms
        $dialog = [System.Windows.Forms.FolderBrowserDialog]::new()
        $dialog.Description = $Description
        $dialog.ShowNewFolderButton = $false
        if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
            return $dialog.SelectedPath
        }
    }
    catch {
        Write-Verbose "Folder picker unavailable: $($_.Exception.Message)"
    }

    return $null
}

function Test-WowClientDir {
    param([string]$Path)

    return (-not [string]::IsNullOrWhiteSpace($Path)) -and
        (Test-Path -LiteralPath (Join-Path $Path "WoW.exe") -PathType Leaf) -and
        (Test-Path -LiteralPath (Join-Path $Path "Data") -PathType Container)
}

function Find-WowClientUnder {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [int]$Depth = 3,
        [ref]$Visited
    )

    if ($Visited.Value -gt 4000 -or $Depth -le 0 -or -not (Test-Path -LiteralPath $Path -PathType Container)) {
        return $null
    }
    $Visited.Value++

    if (Test-WowClientDir $Path) {
        return $Path
    }

    foreach ($child in Get-ChildItem -LiteralPath $Path -Directory -ErrorAction SilentlyContinue) {
        $found = Find-WowClientUnder $child.FullName ($Depth - 1) $Visited
        if ($found) {
            return $found
        }
    }

    return $null
}

function Find-WowClientDir {
    $candidates = [System.Collections.Generic.List[string]]::new()
    foreach ($name in @("RUSTY_MANGOS_WOW_DIR", "WOW_CLIENT_DIR", "ProgramFiles", "ProgramFiles(x86)", "USERPROFILE")) {
        $value = [Environment]::GetEnvironmentVariable($name)
        if (-not [string]::IsNullOrWhiteSpace($value)) {
            $candidates.Add($value)
        }
    }

    foreach ($base in @($candidates)) {
        foreach ($suffix in @(
                "World of Warcraft",
                "World of Warcraft 1.12.1",
                "WoW",
                "Vanilla WoW",
                "Games\World of Warcraft",
                "Downloads\World of Warcraft",
                "Desktop\World of Warcraft",
                "Documents\World of Warcraft"
            )) {
            $candidates.Add((Join-Path $base $suffix))
        }
    }

    foreach ($path in @(
            "C:\Games\World of Warcraft",
            "C:\Games\WoW",
            "C:\WoW",
            "C:\Vanilla WoW",
            "C:\World of Warcraft"
        )) {
        $candidates.Add($path)
    }

    foreach ($candidate in $candidates) {
        if (Test-WowClientDir $candidate) {
            return (Resolve-Path -LiteralPath $candidate).ProviderPath
        }
    }

    foreach ($candidate in $candidates) {
        $visited = 0
        $found = Find-WowClientUnder $candidate 3 ([ref]$visited)
        if ($found) {
            return (Resolve-Path -LiteralPath $found).ProviderPath
        }
    }

    return $null
}

function Resolve-WowClientDir {
    param([string]$Path)

    if ([string]::IsNullOrWhiteSpace($Path)) {
        $Path = Find-WowClientDir
        if (-not [string]::IsNullOrWhiteSpace($Path)) {
            Write-Host "Auto-detected WoW client: $Path"
        }
    }

    while ([string]::IsNullOrWhiteSpace($Path)) {
        $Path = Select-FolderWithDialog "Select your World of Warcraft 1.12.1 client folder"
        if ([string]::IsNullOrWhiteSpace($Path)) {
            $Path = Read-Host "Enter your WoW 1.12.1 client folder"
        }
    }

    $resolved = Resolve-Path -LiteralPath $Path -ErrorAction SilentlyContinue
    if (-not $resolved) {
        throw "Client directory does not exist: $Path"
    }

    $clientRoot = $resolved.ProviderPath
    if (-not (Test-WowClientDir $clientRoot)) {
        throw "Could not find WoW.exe and Data folder in $clientRoot. Pick the WoW 1.12.1 client root folder."
    }

    return $clientRoot
}

function Test-ExtractedServerData {
    param([Parameter(Mandatory = $true)][string]$Path)

    foreach ($name in @("dbc", "maps")) {
        if (-not (Test-Path -LiteralPath (Join-Path $Path $name) -PathType Container)) {
            return $false
        }
    }

    return $true
}

function Convert-ToTomlPath {
    param([Parameter(Mandatory = $true)][string]$Path)
    return (($Path -replace "\\", "/") -replace '"', '\"')
}

function Test-TcpPort {
    param(
        [Parameter(Mandatory = $true)][string]$HostName,
        [Parameter(Mandatory = $true)][int]$Port
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

function Get-LogTail {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [int]$LineCount = 40
    )

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        return ""
    }

    return ((Get-Content -LiteralPath $Path -Tail $LineCount -ErrorAction SilentlyContinue) -join "`n").Trim()
}

function Wait-ForTcpPort {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][int]$Port,
        [Parameter(Mandatory = $true)][int]$TimeoutSeconds,
        [System.Diagnostics.Process]$Process,
        [string]$ErrorLogPath
    )

    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    do {
        if (Test-TcpPort "127.0.0.1" $Port) {
            Write-Host "$Name is listening on 127.0.0.1:$Port"
            return
        }
        if ($Process -and $Process.HasExited) {
            $message = "$Name exited before listening on 127.0.0.1:$Port. Exit code: $($Process.ExitCode)."
            $tail = ""
            if ($ErrorLogPath) {
                $tail = Get-LogTail $ErrorLogPath
            }
            if (-not [string]::IsNullOrWhiteSpace($tail)) {
                $message += "`n`nLast lines from ${ErrorLogPath}:`n$tail"
            }
            throw $message
        }
        Start-Sleep -Milliseconds 500
    } while ((Get-Date) -lt $deadline)

    $message = "$Name did not start listening on 127.0.0.1:$Port within $TimeoutSeconds second(s)."
    $tail = ""
    if ($ErrorLogPath) {
        $tail = Get-LogTail $ErrorLogPath
    }
    if (-not [string]::IsNullOrWhiteSpace($tail)) {
        $message += "`n`nLast lines from ${ErrorLogPath}:`n$tail"
    }
    throw $message
}

function Read-Settings {
    param([Parameter(Mandatory = $true)][string]$Path)
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        return $null
    }
    return Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
}

function Write-Settings {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)]$Settings
    )

    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Path) | Out-Null
    $Settings | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $Path -Encoding ASCII
}

function Write-GeneratedConfigs {
    param(
        [Parameter(Mandatory = $true)]$Settings,
        [Parameter(Mandatory = $true)][string]$LauncherDir
    )

    $clientTomlPath = Convert-ToTomlPath $Settings.clientDir
    $authConfig = @"
bind_address = "127.0.0.1"
bind_port = $($Settings.authPort)

[database]
host = "127.0.0.1"
port = $($Settings.dbPort)
user = "mangos"
password = "mangos"
database = "realmd"
"@

    $worldConfig = @"
bind_address = "127.0.0.1"
bind_port = $($Settings.worldPort)
data_dir = "$clientTomlPath"

[login_database]
host = "127.0.0.1"
port = $($Settings.dbPort)
user = "mangos"
password = "mangos"
database = "realmd"

[world_database]
host = "127.0.0.1"
port = $($Settings.dbPort)
user = "mangos"
password = "mangos"
database = "mangos"

[character_database]
host = "127.0.0.1"
port = $($Settings.dbPort)
user = "mangos"
password = "mangos"
database = "characters"

[world]
MapUpdateInterval = 100
char_delete_method = 0
char_delete_min_level = 0

[playerbots]
enabled = false
combat_enabled = true
force_active = false

[playerbots.random]
enabled = false
count = 0
start_guid = 9010000
name_prefix = "Loadbot"
race = 1
class = 1
gender = 0
level = 1
map = 0
center_x = -8949.0
center_y = -132.0
center_z = 83.5
radius = 80.0
distribution = "radius"
seed = 42

[playerbots.travel]
enabled = false
map = 0
x = -9095.620
y = 422.026
z = 92.0445
radius = 10.0

[observability]
enabled = true
bind_address = "127.0.0.1"
bind_port = 9091
"@

    $authPath = Join-Path $LauncherDir "authserver.launcher.toml"
    $worldPath = Join-Path $LauncherDir "worldserver.launcher.toml"
    Set-Content -LiteralPath $authPath -Value $authConfig -Encoding ASCII
    Set-Content -LiteralPath $worldPath -Value $worldConfig -Encoding ASCII
}

function Get-RealmListPaths {
    param([Parameter(Mandatory = $true)][string]$ClientRoot)

    $paths = New-Object System.Collections.Generic.List[string]
    $rootRealmList = Join-Path $ClientRoot "realmlist.wtf"
    if (Test-Path -LiteralPath $rootRealmList -PathType Leaf) {
        $paths.Add($rootRealmList)
    }

    $dataRoot = Join-Path $ClientRoot "Data"
    if (Test-Path -LiteralPath $dataRoot -PathType Container) {
        Get-ChildItem -LiteralPath $dataRoot -Directory -ErrorAction SilentlyContinue |
            ForEach-Object {
                $candidate = Join-Path $_.FullName "realmlist.wtf"
                if (Test-Path -LiteralPath $candidate -PathType Leaf) {
                    $paths.Add($candidate)
                }
            }
    }

    if ($paths.Count -eq 0) {
        $paths.Add($rootRealmList)
    }

    return $paths.ToArray()
}

function Update-Realmlist {
    param(
        [Parameter(Mandatory = $true)][string]$ClientRoot,
        [Parameter(Mandatory = $true)][int]$Port
    )

    $content = "set realmlist 127.0.0.1:$Port`r`n"
    foreach ($path in Get-RealmListPaths $ClientRoot) {
        if ((Test-Path -LiteralPath $path -PathType Leaf) -and -not (Test-Path -LiteralPath "$path.rusty-mangos.bak")) {
            Copy-Item -LiteralPath $path -Destination "$path.rusty-mangos.bak"
        }
        Set-Content -LiteralPath $path -Value $content -Encoding ASCII
        Write-Host "realmlist updated: $path"
    }
}

function Get-MariaDbRoot {
    param([Parameter(Mandatory = $true)][string]$InstallRoot)

    $direct = Join-Path $InstallRoot "bin\mariadbd.exe"
    if (Test-Path -LiteralPath $direct -PathType Leaf) {
        return $InstallRoot
    }

    $match = Get-ChildItem -LiteralPath $InstallRoot -Directory -ErrorAction SilentlyContinue |
        Where-Object { Test-Path -LiteralPath (Join-Path $_.FullName "bin\mariadbd.exe") -PathType Leaf } |
        Select-Object -First 1

    if ($match) {
        return $match.FullName
    }

    throw "Could not find bin\mariadbd.exe under $InstallRoot"
}

function Ensure-NativeMariaDb {
    param(
        [Parameter(Mandatory = $true)]$Settings,
        [Parameter(Mandatory = $true)][string]$LauncherDir
    )

    $installRoot = Join-Path $LauncherDir "mariadb"
    $downloadDir = Join-Path $LauncherDir "downloads"
    New-Item -ItemType Directory -Force -Path $installRoot, $downloadDir | Out-Null

    try {
        $root = Get-MariaDbRoot $installRoot
        return $root
    }
    catch {
        Write-Host "Portable MariaDB is not installed yet."
    }

    if ([string]::IsNullOrWhiteSpace($MariaDbZipUrl)) {
        $MariaDbZipUrl = "https://archive.mariadb.org/mariadb-$MariaDbVersion/winx64-packages/mariadb-$MariaDbVersion-winx64.zip"
    }

    $zipPath = Join-Path $downloadDir ("mariadb-$MariaDbVersion-winx64.zip")
    if (-not (Test-Path -LiteralPath $zipPath -PathType Leaf)) {
        Write-Host "Downloading MariaDB $MariaDbVersion portable ZIP..."
        [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
        Invoke-WebRequest -Uri $MariaDbZipUrl -OutFile $zipPath
    }

    Write-Host "Extracting MariaDB to $installRoot"
    Expand-Archive -LiteralPath $zipPath -DestinationPath $installRoot -Force
    return Get-MariaDbRoot $installRoot
}

function Initialize-NativeMariaDbData {
    param(
        [Parameter(Mandatory = $true)][string]$MariaRoot,
        [Parameter(Mandatory = $true)][string]$DataDir
    )

    if (Test-Path -LiteralPath (Join-Path $DataDir "mysql") -PathType Container) {
        return
    }

    New-Item -ItemType Directory -Force -Path $DataDir | Out-Null
    $installDb = Join-Path $MariaRoot "bin\mariadb-install-db.exe"
    if (-not (Test-Path -LiteralPath $installDb -PathType Leaf)) {
        $installDb = Join-Path $MariaRoot "bin\mysql_install_db.exe"
    }
    if (-not (Test-Path -LiteralPath $installDb -PathType Leaf)) {
        throw "Could not find mariadb-install-db.exe or mysql_install_db.exe under $MariaRoot\bin"
    }

    Write-Host "Initializing local MariaDB data directory."
    Invoke-Checked $installDb @("--datadir=$DataDir", "--password=root")
}

function Start-NativeMariaDb {
    param(
        [Parameter(Mandatory = $true)][string]$MariaRoot,
        [Parameter(Mandatory = $true)][string]$DataDir,
        [Parameter(Mandatory = $true)][string]$PidPath,
        [Parameter(Mandatory = $true)][string]$LogDir,
        [Parameter(Mandatory = $true)][int]$Port
    )

    if (Test-TcpPort "127.0.0.1" $Port) {
        Write-Host "MariaDB is already listening on 127.0.0.1:$Port"
        return
    }

    $server = Join-Path $MariaRoot "bin\mariadbd.exe"
    $stdout = Join-Path $LogDir "mariadb.log"
    $stderr = Join-Path $LogDir "mariadb.err.log"
    $args = @(
        "--no-defaults",
        "--datadir=$DataDir",
        "--port=$Port",
        "--bind-address=127.0.0.1",
        "--console"
    )

    $process = Start-Process -FilePath $server -ArgumentList (Join-ProcessArguments $args) -WindowStyle Hidden -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
    Set-Content -LiteralPath $PidPath -Value $process.Id -Encoding ASCII
    Wait-ForTcpPort "MariaDB" $Port 60 -Process $process -ErrorLogPath $stderr
}

function Invoke-MariaDbSql {
    param(
        [Parameter(Mandatory = $true)][string]$MariaRoot,
        [Parameter(Mandatory = $true)][int]$Port,
        [string]$Database,
        [Parameter(Mandatory = $true)][string]$Sql,
        [switch]$AsMangosUser
    )

    $client = Join-Path $MariaRoot "bin\mariadb.exe"
    if (-not (Test-Path -LiteralPath $client -PathType Leaf)) {
        $client = Join-Path $MariaRoot "bin\mysql.exe"
    }

    $args = @("--protocol=tcp", "-h127.0.0.1", "-P$Port", "--default-character-set=utf8")
    if ($AsMangosUser) {
        $args += @("-umangos", "-pmangos")
    }
    else {
        $args += @("-uroot", "-proot")
    }
    if ($Database) {
        $args += $Database
    }
    $args += @("-e", $Sql)

    Invoke-Checked $client $args
}

function Invoke-MariaDbScalar {
    param(
        [Parameter(Mandatory = $true)][string]$MariaRoot,
        [Parameter(Mandatory = $true)][int]$Port,
        [Parameter(Mandatory = $true)][string]$Sql
    )

    $client = Join-Path $MariaRoot "bin\mariadb.exe"
    if (-not (Test-Path -LiteralPath $client -PathType Leaf)) {
        $client = Join-Path $MariaRoot "bin\mysql.exe"
    }
    $args = @("--protocol=tcp", "-h127.0.0.1", "-P$Port", "-uroot", "-proot", "-N", "-B", "-e", $Sql)
    $result = & $client @args
    if ($LASTEXITCODE -ne 0) {
        return $null
    }
    return ($result | Select-Object -First 1)
}

function Import-MariaDbSqlFile {
    param(
        [Parameter(Mandatory = $true)][string]$MariaRoot,
        [Parameter(Mandatory = $true)][int]$Port,
        [Parameter(Mandatory = $true)][string]$Database,
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Description
    )

    $client = Join-Path $MariaRoot "bin\mariadb.exe"
    if (-not (Test-Path -LiteralPath $client -PathType Leaf)) {
        $client = Join-Path $MariaRoot "bin\mysql.exe"
    }

    $resolved = Resolve-Path -LiteralPath $Path
    Write-Host "Importing $Description from $resolved"
    $processInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $processInfo.FileName = $client
    $processInfo.Arguments = "--protocol=tcp -h127.0.0.1 -P$Port -uroot -proot --default-character-set=utf8 $Database"
    $processInfo.RedirectStandardInput = $true
    $processInfo.RedirectStandardOutput = $true
    $processInfo.RedirectStandardError = $true
    $processInfo.UseShellExecute = $false

    $process = [System.Diagnostics.Process]::Start($processInfo)
    $input = [System.IO.File]::OpenRead($resolved.ProviderPath)
    $sqlStream = $input
    if ($resolved.ProviderPath.EndsWith(".gz", [System.StringComparison]::OrdinalIgnoreCase)) {
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

function Ensure-BaseDatabases {
    param(
        [Parameter(Mandatory = $true)][string]$MariaRoot,
        [Parameter(Mandatory = $true)][int]$Port,
        [Parameter(Mandatory = $true)][string]$RepoRoot
    )

    Invoke-MariaDbSql $MariaRoot $Port "" "CREATE DATABASE IF NOT EXISTS realmd DEFAULT CHARACTER SET utf8 COLLATE utf8_general_ci;"
    Invoke-MariaDbSql $MariaRoot $Port "" "CREATE DATABASE IF NOT EXISTS characters DEFAULT CHARACTER SET utf8 COLLATE utf8_general_ci;"
    Invoke-MariaDbSql $MariaRoot $Port "" "CREATE DATABASE IF NOT EXISTS mangos DEFAULT CHARACTER SET utf8 COLLATE utf8_general_ci;"
    Invoke-MariaDbSql $MariaRoot $Port "" "CREATE USER IF NOT EXISTS 'mangos'@'localhost' IDENTIFIED BY 'mangos'; CREATE USER IF NOT EXISTS 'mangos'@'%' IDENTIFIED BY 'mangos'; GRANT ALL PRIVILEGES ON realmd.* TO 'mangos'@'localhost'; GRANT ALL PRIVILEGES ON realmd.* TO 'mangos'@'%'; GRANT ALL PRIVILEGES ON characters.* TO 'mangos'@'localhost'; GRANT ALL PRIVILEGES ON characters.* TO 'mangos'@'%'; GRANT ALL PRIVILEGES ON mangos.* TO 'mangos'@'localhost'; GRANT ALL PRIVILEGES ON mangos.* TO 'mangos'@'%'; FLUSH PRIVILEGES;"

    $realmdTables = Invoke-MariaDbScalar $MariaRoot $Port "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema='realmd' AND table_name='account';"
    if (($realmdTables -as [int]) -eq 0) {
        Import-MariaDbSqlFile $MariaRoot $Port "realmd" (Join-Path $RepoRoot "sql\base\realmd.sql") "sql/base/realmd.sql"
    }

    $characterTables = Invoke-MariaDbScalar $MariaRoot $Port "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema='characters' AND table_name='characters';"
    if (($characterTables -as [int]) -eq 0) {
        Import-MariaDbSqlFile $MariaRoot $Port "characters" (Join-Path $RepoRoot "sql\base\characters.sql") "sql/base/characters.sql"
    }

    $worldTables = Invoke-MariaDbScalar $MariaRoot $Port "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema='mangos' AND table_name='playercreateinfo';"
    if (($worldTables -as [int]) -eq 0) {
        Import-MariaDbSqlFile $MariaRoot $Port "mangos" (Join-Path $RepoRoot "sql\base\mangos.sql") "sql/base/mangos.sql"
    }
}

function Get-WorldContentCount {
    param(
        [Parameter(Mandatory = $true)][string]$MariaRoot,
        [Parameter(Mandatory = $true)][int]$Port
    )

    $sql = "SELECT COALESCE((SELECT COUNT(*) FROM mangos.creature), 0) + COALESCE((SELECT COUNT(*) FROM mangos.gameobject), 0) + COALESCE((SELECT COUNT(*) FROM mangos.quest_template), 0);"
    $line = Invoke-MariaDbScalar $MariaRoot $Port $sql
    $count = 0
    if ([int]::TryParse((([string]$line).Trim()), [ref]$count)) {
        return $count
    }
    return 0
}

function Ensure-ClassicDbCheckout {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [bool]$AllowClone
    )

    $fullDb = Join-Path $Path "Full_DB\ClassicDB_1_12_1_z2815.sql.gz"
    if (Test-Path -LiteralPath $fullDb -PathType Leaf) {
        return
    }

    if (-not $AllowClone) {
        throw "ClassicDB full database was not found at $fullDb."
    }

    Require-Command "git" "Install Git or pass -ClassicDbPath pointing at an existing classic-db checkout."
    if (Test-Path -LiteralPath $Path) {
        Invoke-Checked git @("-C", $Path, "pull", "--ff-only")
    }
    else {
        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Path) | Out-Null
        Invoke-Checked git @("clone", "--depth", "1", "https://github.com/cmangos/classic-db", $Path)
    }
}

function Import-ClassicDbWorld {
    param(
        [Parameter(Mandatory = $true)][string]$MariaRoot,
        [Parameter(Mandatory = $true)][int]$Port,
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][string]$ClassicDbRoot
    )

    $fullDb = Join-Path $ClassicDbRoot "Full_DB\ClassicDB_1_12_1_z2815.sql.gz"
    Invoke-MariaDbSql $MariaRoot $Port "" "DROP DATABASE IF EXISTS mangos; CREATE DATABASE mangos DEFAULT CHARACTER SET utf8 COLLATE utf8_general_ci; GRANT ALL PRIVILEGES ON mangos.* TO 'mangos'@'localhost'; GRANT ALL PRIVILEGES ON mangos.* TO 'mangos'@'%'; FLUSH PRIVILEGES;"
    Import-MariaDbSqlFile $MariaRoot $Port "mangos" $fullDb "ClassicDB full world z2815"

    $contentUpdates = Get-ChildItem (Join-Path $ClassicDbRoot "Updates") -File -Filter "*.sql" | Sort-Object Name
    foreach ($file in $contentUpdates) {
        Import-MariaDbSqlFile $MariaRoot $Port "mangos" $file.FullName "ClassicDB update $($file.Name)"
    }

    $instancePath = Join-Path $ClassicDbRoot "Updates\Instances"
    if (Test-Path -LiteralPath $instancePath -PathType Container) {
        $instanceUpdates = Get-ChildItem $instancePath -File -Filter "*.sql" | Sort-Object Name
        foreach ($file in $instanceUpdates) {
            Import-MariaDbSqlFile $MariaRoot $Port "mangos" $file.FullName "ClassicDB instance update $($file.Name)"
        }
    }

    $coreUpdates = Get-ChildItem (Join-Path $RepoRoot "sql\updates\mangos") -File -Filter "*.sql" |
        Where-Object { $_.Name -match '^z(\d+)_' -and [int]$Matches[1] -gt 2829 } |
        Sort-Object Name
    foreach ($file in $coreUpdates) {
        Import-MariaDbSqlFile $MariaRoot $Port "mangos" $file.FullName "remaining core world update $($file.Name)"
    }
}

function Get-LauncherBuildId {
    param([Parameter(Mandatory = $true)][string]$RepoRoot)

    $buildInfoPath = Join-Path $RepoRoot "BUILD_INFO.txt"
    if (Test-Path -LiteralPath $buildInfoPath -PathType Leaf) {
        $line = Get-Content -LiteralPath $buildInfoPath |
            Where-Object { $_ -match '^Source commit:\s*([0-9a-fA-F]{8,})' } |
            Select-Object -First 1
        if ($line -and $line -match '^Source commit:\s*([0-9a-fA-F]{8,})') {
            return $Matches[1].Substring(0, 8)
        }
    }

    try {
        $commit = ((& git -C $RepoRoot rev-parse --short=8 HEAD 2>$null) -join "").Trim()
        if (-not [string]::IsNullOrWhiteSpace($commit)) {
            return $commit
        }
    }
    catch {
    }

    return "local"
}

function Seed-PlayAccount {
    param(
        [Parameter(Mandatory = $true)][string]$MariaRoot,
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][int]$Port,
        [Parameter(Mandatory = $true)][int]$WorldPort,
        [bool]$Reset
    )

    $buildId = Get-LauncherBuildId $RepoRoot
    $realmName = "Pre-alpha Test Realm $buildId"
    Invoke-MariaDbSql $MariaRoot $Port "realmd" "UPDATE realmlist SET name='$realmName', address='127.0.0.1', port=$WorldPort WHERE id=1;"
    Write-Host "Realm name: $realmName"

    $rustAuthVerifier = "171c640a3ed8fa4a187d99ce40b5ca1a62f39bc15dbbe74482edaa9a4eafc42f"
    $rustAuthSalt = "49212faef52cbb62fd06a55599e9ed118cd3155ed8766ec1132a39a12acc9681"
    $seedAccountSql = @"
INSERT INTO account (username, gmlevel, sessionkey, v, s, email, locked, expansion, locale, os)
VALUES ('RUSTAUTH', 3, '', '$rustAuthVerifier', '$rustAuthSalt', '', 0, 0, '', 'Win')
ON DUPLICATE KEY UPDATE
    gmlevel = 3,
    sessionkey = '',
    v = VALUES(v),
    s = VALUES(s),
    locked = 0,
    os = 'Win';
"@
    Invoke-MariaDbSql $MariaRoot $Port "realmd" $seedAccountSql

    if ($Reset) {
        $deleteSql = @"
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
DROP TEMPORARY TABLE rust_client_account_chars;
"@
        Invoke-MariaDbSql $MariaRoot $Port "characters" $deleteSql
    }

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
    Invoke-MariaDbSql $MariaRoot $Port "characters" $seedCharacterSql

    $realmCharacterCountSql = @"
INSERT INTO realmd.realmcharacters (realmid, acctid, numchars)
SELECT 1, a.id, COUNT(c.guid)
FROM realmd.account a
LEFT JOIN characters.characters c ON c.account = a.id
WHERE a.username = 'RUSTAUTH'
GROUP BY a.id
ON DUPLICATE KEY UPDATE numchars = VALUES(numchars);
"@
    Invoke-MariaDbSql $MariaRoot $Port "characters" $realmCharacterCountSql

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
    Invoke-MariaDbSql $MariaRoot $Port "" $backfillStarterSkillsSql
}

function Stop-ByPidFile {
    param([Parameter(Mandatory = $true)][string]$Path)

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        return
    }

    $pidText = (Get-Content -LiteralPath $Path -Raw).Trim()
    $pidValue = 0
    if ([int]::TryParse($pidText, [ref]$pidValue)) {
        Stop-Process -Id $pidValue -Force -ErrorAction SilentlyContinue
    }
    Remove-Item -LiteralPath $Path -Force -ErrorAction SilentlyContinue
}

function Stop-LauncherStack {
    param([Parameter(Mandatory = $true)][string]$PidDir)

    Write-Step "Stopping Rusty MaNGOS"
    Stop-ByPidFile (Join-Path $PidDir "worldserver.pid")
    Stop-ByPidFile (Join-Path $PidDir "authserver.pid")
    Stop-ByPidFile (Join-Path $PidDir "mariadb.pid")
    Get-Process authserver,worldserver -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
}

function Start-RustServers {
    param(
        [Parameter(Mandatory = $true)]$Settings,
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][string]$LauncherDir,
        [Parameter(Mandatory = $true)][string]$LogDir,
        [Parameter(Mandatory = $true)][string]$PidDir
    )

    $authExe = Join-Path $RepoRoot "server\authserver.exe"
    $worldExe = Join-Path $RepoRoot "server\worldserver.exe"

    if (-not (Test-Path -LiteralPath $authExe -PathType Leaf) -or -not (Test-Path -LiteralPath $worldExe -PathType Leaf)) {
        $profile = "release"
        if ($Settings.debugBuild) {
            $profile = "debug"
        }
        $authExe = Join-Path $RepoRoot "target\$profile\authserver.exe"
        $worldExe = Join-Path $RepoRoot "target\$profile\worldserver.exe"
    }

    if (-not (Test-Path -LiteralPath $authExe -PathType Leaf) -or -not (Test-Path -LiteralPath $worldExe -PathType Leaf)) {
        Write-Step "Building Rust servers"
        Require-Command "cargo" "Install Rust from https://rustup.rs/ and reopen this terminal, or install a packaged Rusty MaNGOS build with bundled server binaries."
        $args = @("build")
        if (-not $Settings.debugBuild) {
            $args += "--release"
        }
        Invoke-Checked cargo ($args + @("-p", "authserver")) $RepoRoot
        Invoke-Checked cargo ($args + @("-p", "worldserver")) $RepoRoot

        $profile = "release"
        if ($Settings.debugBuild) {
            $profile = "debug"
        }
        $authExe = Join-Path $RepoRoot "target\$profile\authserver.exe"
        $worldExe = Join-Path $RepoRoot "target\$profile\worldserver.exe"
    }

    Get-Process authserver,worldserver -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue

    $authLog = Join-Path $LogDir "authserver.log"
    $authErr = Join-Path $LogDir "authserver.err.log"
    $worldLog = Join-Path $LogDir "worldserver.log"
    $worldErr = Join-Path $LogDir "worldserver.err.log"
    $authConfig = Join-Path $LauncherDir "authserver.launcher.toml"
    $worldConfig = Join-Path $LauncherDir "worldserver.launcher.toml"

    $auth = Start-Process -FilePath $authExe -ArgumentList (Join-ProcessArguments @("--config", $authConfig)) -WorkingDirectory $RepoRoot -WindowStyle Hidden -PassThru -RedirectStandardOutput $authLog -RedirectStandardError $authErr
    Set-Content -LiteralPath (Join-Path $PidDir "authserver.pid") -Value $auth.Id -Encoding ASCII

    $world = Start-Process -FilePath $worldExe -ArgumentList (Join-ProcessArguments @("--config", $worldConfig)) -WorkingDirectory $RepoRoot -WindowStyle Hidden -PassThru -RedirectStandardOutput $worldLog -RedirectStandardError $worldErr
    Set-Content -LiteralPath (Join-Path $PidDir "worldserver.pid") -Value $world.Id -Encoding ASCII

    Wait-ForTcpPort "Authserver" $Settings.authPort $ReadyTimeoutSeconds -Process $auth -ErrorLogPath $authErr
    Wait-ForTcpPort "Worldserver" $Settings.worldPort $ReadyTimeoutSeconds -Process $world -ErrorLogPath $worldErr
    Wait-ForTcpPort "Observability dashboard" 9091 $ReadyTimeoutSeconds
}

function Show-Status {
    param(
        [Parameter(Mandatory = $true)]$Settings,
        [Parameter(Mandatory = $true)][string]$PidDir
    )

    Write-Step "Status"
    foreach ($name in @("mariadb", "authserver", "worldserver")) {
        $pidFile = Join-Path $PidDir "$name.pid"
        $status = "not launcher-managed"
        if (Test-Path -LiteralPath $pidFile -PathType Leaf) {
            $pidText = (Get-Content -LiteralPath $pidFile -Raw).Trim()
            $pidValue = 0
            if ([int]::TryParse($pidText, [ref]$pidValue)) {
                $process = Get-Process -Id $pidValue -ErrorAction SilentlyContinue
                if ($process) {
                    $status = "running pid $pidValue"
                }
                else {
                    $status = "pid file exists, process not running"
                }
            }
        }
        Write-Host "${name}: $status"
    }

    $dbPortStatus = "closed"
    if (Test-TcpPort "127.0.0.1" $Settings.dbPort) { $dbPortStatus = "open" }
    $authPortStatus = "closed"
    if (Test-TcpPort "127.0.0.1" $Settings.authPort) { $authPortStatus = "open" }
    $worldPortStatus = "closed"
    if (Test-TcpPort "127.0.0.1" $Settings.worldPort) { $worldPortStatus = "open" }

    Write-Host "MariaDB port $($Settings.dbPort): $dbPortStatus"
    Write-Host "Auth port $($Settings.authPort): $authPortStatus"
    Write-Host "World port $($Settings.worldPort): $worldPortStatus"
    Write-Host "Dashboard: http://127.0.0.1:9091/dashboard"
}

function Write-LauncherShortcuts {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][string]$LauncherDir
    )

    $launcher = Join-Path $RepoRoot "scripts\rusty-mangos-launcher.cmd"
    $commands = @{
        "Start Rusty MaNGOS.cmd" = "Start"
        "Stop Rusty MaNGOS.cmd" = "Stop"
        "Restart Rusty MaNGOS.cmd" = "Restart"
        "Rusty MaNGOS Status.cmd" = "Status"
        "Configure Rusty MaNGOS.cmd" = "Configure"
    }

    foreach ($entry in $commands.GetEnumerator()) {
        $path = Join-Path $LauncherDir $entry.Key
        $content = "@echo off`r`n`"$launcher`" $($entry.Value)`r`npause`r`n"
        Set-Content -LiteralPath $path -Value $content -Encoding ASCII
    }
}

function Test-BundledServers {
    param([Parameter(Mandatory = $true)][string]$Root)

    return (Test-Path -LiteralPath (Join-Path $Root "server\authserver.exe") -PathType Leaf) -and
        (Test-Path -LiteralPath (Join-Path $Root "server\worldserver.exe") -PathType Leaf)
}

if ($Help) {
    Show-Usage
    exit 0
}

if ($DatabaseMode -eq "Docker") {
    throw "Docker mode is intentionally no longer the default launcher path. Use scripts\restart-game-stack.cmd directly for the existing Docker-backed dev stack."
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).ProviderPath
$launcherDir = Join-Path $repoRoot "target\launcher"
$logDir = Join-Path $launcherDir "logs"
$pidDir = Join-Path $launcherDir "pids"
$settingsPath = Join-Path $launcherDir "rusty-mangos.settings.json"
New-Item -ItemType Directory -Force -Path $launcherDir, $logDir, $pidDir | Out-Null

if ([string]::IsNullOrWhiteSpace($ClassicDbPath)) {
    $ClassicDbPath = Join-Path $repoRoot "target\classic-db"
}

$settings = Read-Settings $settingsPath

if ($Action -eq "Stop") {
    Stop-LauncherStack $pidDir
    exit 0
}

if ($Action -eq "Restart") {
    Stop-LauncherStack $pidDir
    $Action = "Start"
}

if ($Action -eq "Status") {
    if (-not $settings) {
        throw "Rusty MaNGOS is not configured yet. Run .\scripts\rusty-mangos-launcher.cmd Install first."
    }
    Show-Status $settings $pidDir
    exit 0
}

$needsConfigure = $false
if ($Action -eq "InstallStart" -or $Action -eq "Install" -or $Action -eq "Configure") {
    $needsConfigure = $true
}
if (-not $settings) {
    $needsConfigure = $true
}

if ($needsConfigure) {
    Write-Step "Configuring Rusty MaNGOS"
    if ($Action -ne "Configure" -and -not (Test-BundledServers $repoRoot)) {
        Require-Command "cargo" "Install Rust from https://rustup.rs/ and reopen this terminal."
    }
    if (-not $SkipWorldImport -and -not $NoClassicDbClone) {
        Require-Command "git" "Install Git, or rerun with -NoClassicDbClone and -ClassicDbPath."
    }

    $clientRoot = Resolve-WowClientDir $ClientDir
    if (-not (Test-ExtractedServerData $clientRoot)) {
        Write-Warning "The selected client folder does not contain extracted server data folders 'dbc' and 'maps'."
        Write-Warning "The server can start, but DBC/map/vmap/mmap parity will be limited until extracted CMaNGOS data is placed there."
    }

    $debug = $false
    if ($DebugBuild) {
        $debug = $true
    }

    $settings = [pscustomobject]@{
        databaseMode = "Native"
        clientDir = $clientRoot
        classicDbPath = $ClassicDbPath
        dbPort = $DbPort
        worldPort = $WorldPort
        authPort = $AuthPort
        debugBuild = $debug
        mariaDbVersion = $MariaDbVersion
    }

    Write-Settings $settingsPath $settings
    Write-GeneratedConfigs $settings $launcherDir
    Write-LauncherShortcuts $repoRoot $launcherDir

    if (-not $NoRealmlistUpdate) {
        Update-Realmlist $clientRoot $AuthPort
    }

    if ($Action -eq "Configure") {
        Write-Host ""
        Write-Host "Rusty MaNGOS configuration was updated." -ForegroundColor Green
        Write-Host "Launcher commands are in: $launcherDir"
        exit 0
    }
}
else {
    Write-GeneratedConfigs $settings $launcherDir
}

Write-Step "Preparing native MariaDB"
$mariaRoot = Ensure-NativeMariaDb $settings $launcherDir
$mariaData = Join-Path $launcherDir "mariadb-data"
Initialize-NativeMariaDbData $mariaRoot $mariaData
Start-NativeMariaDb $mariaRoot $mariaData (Join-Path $pidDir "mariadb.pid") $logDir $settings.dbPort

Write-Step "Preparing databases"
Ensure-BaseDatabases $mariaRoot $settings.dbPort $repoRoot

if (-not $SkipWorldImport) {
    $contentCount = Get-WorldContentCount $mariaRoot $settings.dbPort
    if ($contentCount -eq 0 -or $ForceWorldImport) {
        Ensure-ClassicDbCheckout $settings.classicDbPath (-not $NoClassicDbClone)
        Import-ClassicDbWorld $mariaRoot $settings.dbPort $repoRoot $settings.classicDbPath
    }
    else {
        Write-Host "World database already has content rows ($contentCount); skipping ClassicDB import."
    }
}

Seed-PlayAccount $mariaRoot $repoRoot $settings.dbPort $settings.worldPort $ResetCharacters

if ($Action -eq "Install") {
    Write-Host ""
    Write-Host "Rusty MaNGOS is installed/configured." -ForegroundColor Green
    Write-Host "Launcher commands were written to: $launcherDir"
    Write-Host "Run: .\scripts\rusty-mangos-launcher.cmd Start"
    exit 0
}

Write-Step "Starting Rusty MaNGOS"
Start-RustServers $settings $repoRoot $launcherDir $logDir $pidDir

Write-Host ""
Write-Host "Rusty MaNGOS is ready." -ForegroundColor Green
Write-Host "Client realmlist: set realmlist 127.0.0.1:$($settings.authPort)"
Write-Host "Login account: RUSTAUTH"
Write-Host "Login password: RUSTPASS"
Write-Host "Dashboard: http://127.0.0.1:9091/dashboard"
Write-Host "Launcher commands: $launcherDir"
