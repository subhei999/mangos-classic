param(
    [string]$ClassicDbPath = (Join-Path (Join-Path $PSScriptRoot "..") "target\classic-db")
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

function Invoke-MariaDbFile {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path,
        [Parameter(Mandatory = $true)]
        [string]$Description
    )

    $resolved = Resolve-Path $Path
    Write-Host "Applying $Description"
    & cmd.exe /c "docker exec -i cmangos-rust-realmd mariadb -uroot -proot mangos < `"$resolved`""
    if ($LASTEXITCODE -ne 0) {
        throw "failed applying $Description from $resolved"
    }
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$classicDbRoot = Resolve-Path $ClassicDbPath
$fullDb = Join-Path $classicDbRoot "Full_DB\ClassicDB_1_12_1_z2815.sql.gz"
if (-not (Test-Path $fullDb)) {
    throw "ClassicDB full DB file was not found at $fullDb. Clone https://github.com/cmangos/classic-db to $classicDbRoot first."
}

Set-Location $repoRoot
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

Invoke-Checked docker @(
    "exec", "cmangos-rust-realmd", "mariadb", "-uroot", "-proot", "-e",
    "DROP DATABASE IF EXISTS mangos; CREATE DATABASE mangos DEFAULT CHARACTER SET utf8 COLLATE utf8_general_ci; GRANT ALL PRIVILEGES ON mangos.* TO 'mangos'@'%'; FLUSH PRIVILEGES;"
)

Import-MariaDbSqlFile "mangos" $fullDb "ClassicDB full world z2815"

$contentUpdates = Get-ChildItem (Join-Path $classicDbRoot "Updates") -File -Filter "*.sql" | Sort-Object Name
foreach ($file in $contentUpdates) {
    Invoke-MariaDbFile $file.FullName "ClassicDB update $($file.Name)"
}

$instanceUpdates = Get-ChildItem (Join-Path $classicDbRoot "Updates\Instances") -File -Filter "*.sql" | Sort-Object Name
foreach ($file in $instanceUpdates) {
    Invoke-MariaDbFile $file.FullName "ClassicDB instance update $($file.Name)"
}

$coreUpdates = Get-ChildItem "sql\updates\mangos" -File -Filter "*.sql" |
    Where-Object { $_.Name -match '^z(\d+)_' -and [int]$Matches[1] -gt 2829 } |
    Sort-Object Name
foreach ($file in $coreUpdates) {
    Invoke-MariaDbFile $file.FullName "remaining core world update $($file.Name)"
}

Write-Host "ClassicDB world import complete."
