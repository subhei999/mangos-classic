param(
    [switch]$SkipRustBuild,
    [switch]$SkipInstaller,
    [string]$InnoSetupUrl = "https://jrsoftware.org/download.php/is.exe"
)

$ErrorActionPreference = "Stop"

function Invoke-Checked {
    param(
        [Parameter(Mandatory = $true)][string]$Command,
        [string[]]$Arguments
    )

    & $Command @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "$Command $($Arguments -join ' ') failed with exit code $LASTEXITCODE"
    }
}

function Get-InnoCompiler {
    param([Parameter(Mandatory = $true)][string]$RepoRoot)

    $pathCompiler = Get-Command iscc -ErrorAction SilentlyContinue
    if ($pathCompiler) {
        return $pathCompiler.Source
    }

    $toolRoot = Join-Path $RepoRoot "target\tooling"
    $innoRoot = Join-Path $toolRoot "inno-setup"
    $localCompiler = Join-Path $innoRoot "ISCC.exe"
    if (Test-Path -LiteralPath $localCompiler -PathType Leaf) {
        return $localCompiler
    }

    New-Item -ItemType Directory -Force -Path $toolRoot, $innoRoot | Out-Null
    $downloadPath = Join-Path $toolRoot "innosetup.exe"

    if (-not (Test-Path -LiteralPath $downloadPath -PathType Leaf)) {
        Write-Host "Downloading Inno Setup compiler from $InnoSetupUrl"
        [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
        Invoke-WebRequest -Uri $InnoSetupUrl -OutFile $downloadPath
    }

    Write-Host "Installing Inno Setup compiler into $innoRoot"
    $arguments = @(
        "/VERYSILENT",
        "/SUPPRESSMSGBOXES",
        "/CURRENTUSER",
        "/NOICONS",
        "/NORESTART",
        "/DIR=`"$innoRoot`""
    )
    $process = Start-Process -FilePath $downloadPath -ArgumentList $arguments -Wait -PassThru
    if ($process.ExitCode -ne 0) {
        throw "Inno Setup installer failed with exit code $($process.ExitCode)"
    }

    if (-not (Test-Path -LiteralPath $localCompiler -PathType Leaf)) {
        throw "Inno Setup installed, but ISCC.exe was not found at $localCompiler"
    }

    return $localCompiler
}

function Get-ExtractorSource {
    param([Parameter(Mandatory = $true)][string]$RepoRoot)

    $existing = Join-Path $RepoRoot "build-cmangos-tools\bin\x64_Release\Extractors"
    if (Test-Path -LiteralPath (Join-Path $existing "ad.exe") -PathType Leaf) {
        return $existing
    }

    $buildRoot = Join-Path $RepoRoot "target\launcher-extractor-build"
    $binaryRoot = Join-Path $buildRoot "bin\x64_Release\Extractors"
    if (-not (Test-Path -LiteralPath (Join-Path $binaryRoot "ad.exe") -PathType Leaf)) {
        Invoke-Checked cmake @(
            "-S", $RepoRoot,
            "-B", $buildRoot,
            "-G", "Visual Studio 17 2022",
            "-A", "x64",
            "-DBUILD_GAME_SERVER=OFF",
            "-DBUILD_LOGIN_SERVER=OFF",
            "-DBUILD_SCRIPTDEV=OFF",
            "-DBUILD_EXTRACTORS=ON",
            "-DPCH=OFF",
            "-DDEV_BINARY_DIR=$buildRoot\bin"
        )
        Invoke-Checked cmake @(
            "--build", $buildRoot,
            "--config", "Release",
            "--target", "ad"
        )
    }

    if (-not (Test-Path -LiteralPath (Join-Path $binaryRoot "ad.exe") -PathType Leaf)) {
        throw "CMaNGOS ad.exe extractor was not found or built at $binaryRoot"
    }

    return $binaryRoot
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).ProviderPath
Set-Location $repoRoot

$packageRoot = Join-Path $repoRoot "target\launcher-package"
$appRoot = Join-Path $packageRoot "app"

Remove-Item -LiteralPath $appRoot -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $appRoot | Out-Null

if (-not $SkipRustBuild) {
    Invoke-Checked cargo @("build", "--release", "-p", "authserver")
    Invoke-Checked cargo @("build", "--release", "-p", "worldserver")
}

Invoke-Checked cargo @("build", "--release", "-p", "rusty-mangos-launcher")

Copy-Item -LiteralPath "target\release\rusty-mangos-launcher.exe" -Destination (Join-Path $appRoot "RustyMangosLauncher.exe") -Force

New-Item -ItemType Directory -Force -Path (Join-Path $appRoot "server") | Out-Null
Copy-Item -LiteralPath "target\release\authserver.exe" -Destination (Join-Path $appRoot "server\authserver.exe") -Force
Copy-Item -LiteralPath "target\release\worldserver.exe" -Destination (Join-Path $appRoot "server\worldserver.exe") -Force

$commit = (& git rev-parse HEAD).Trim()
$branch = (& git rev-parse --abbrev-ref HEAD).Trim()
$dirty = ((& git status --short) -join "`n").Trim()
$dirtyState = "clean"
if (-not [string]::IsNullOrWhiteSpace($dirty)) {
    $dirtyState = "dirty"
}
$buildInfo = @"
Rusty MaNGOS launcher package

Source branch: $branch
Source commit: $commit
Working tree: $dirtyState
Built at UTC: $((Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ"))

Included binaries:
- server\authserver.exe
- server\worldserver.exe
- RustyMangosLauncher.exe
"@
Set-Content -LiteralPath (Join-Path $appRoot "BUILD_INFO.txt") -Value $buildInfo -Encoding ASCII

New-Item -ItemType Directory -Force -Path (Join-Path $appRoot "scripts") | Out-Null
Copy-Item -LiteralPath "scripts\rusty-mangos-launcher.ps1" -Destination (Join-Path $appRoot "scripts\rusty-mangos-launcher.ps1") -Force
Copy-Item -LiteralPath "scripts\rusty-mangos-launcher.cmd" -Destination (Join-Path $appRoot "scripts\rusty-mangos-launcher.cmd") -Force

New-Item -ItemType Directory -Force -Path (Join-Path $appRoot "tools\extractors") | Out-Null
$extractorSource = Get-ExtractorSource $repoRoot
Copy-Item -LiteralPath (Join-Path $extractorSource "ad.exe") -Destination (Join-Path $appRoot "tools\extractors\ad.exe") -Force
if (Test-Path -LiteralPath (Join-Path $extractorSource "zlib.dll") -PathType Leaf) {
    Copy-Item -LiteralPath (Join-Path $extractorSource "zlib.dll") -Destination (Join-Path $appRoot "tools\extractors\zlib.dll") -Force
}

New-Item -ItemType Directory -Force -Path (Join-Path $appRoot "docs") | Out-Null
Copy-Item -LiteralPath "docs\rusty_mangos_launcher.md" -Destination (Join-Path $appRoot "docs\rusty_mangos_launcher.md") -Force

New-Item -ItemType Directory -Force -Path (Join-Path $appRoot "sql\base") | Out-Null
Copy-Item -LiteralPath "sql\base\realmd.sql" -Destination (Join-Path $appRoot "sql\base\realmd.sql") -Force
Copy-Item -LiteralPath "sql\base\characters.sql" -Destination (Join-Path $appRoot "sql\base\characters.sql") -Force
Copy-Item -LiteralPath "sql\base\mangos.sql" -Destination (Join-Path $appRoot "sql\base\mangos.sql") -Force

New-Item -ItemType Directory -Force -Path (Join-Path $appRoot "sql\updates\mangos") | Out-Null
Copy-Item -Path "sql\updates\mangos\*.sql" -Destination (Join-Path $appRoot "sql\updates\mangos") -Force

if ($SkipInstaller) {
    Write-Host "Packaged app folder: $appRoot"
    exit 0
}

$iscc = Get-InnoCompiler $repoRoot
Invoke-Checked $iscc @("installer\RustyMangos.iss")
Write-Host "Installer output: $(Join-Path $packageRoot 'installer\RustyMangosSetup.exe')"
