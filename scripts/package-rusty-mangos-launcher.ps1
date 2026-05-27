param(
    [switch]$SkipRustBuild,
    [switch]$SkipInstaller,
    [string]$ExtractorSourcePath,
    [string]$InnoSetupUrl = "https://jrsoftware.org/download.php/is.exe",
    [string]$SignToolPath,
    [string]$SignCertSha1 = $env:RUSTY_MANGOS_SIGN_CERT_SHA1,
    [string]$SignPfxPath = $env:RUSTY_MANGOS_SIGN_PFX,
    [string]$SignPfxPassword = $env:RUSTY_MANGOS_SIGN_PFX_PASSWORD,
    [string]$TimestampUrl = "http://timestamp.digicert.com"
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

function Get-SignTool {
    if (-not [string]::IsNullOrWhiteSpace($SignToolPath)) {
        $resolved = Resolve-Path -LiteralPath $SignToolPath -ErrorAction SilentlyContinue
        if (-not $resolved) {
            throw "SignToolPath does not exist: $SignToolPath"
        }
        return $resolved.ProviderPath
    }

    $pathTool = Get-Command signtool.exe -ErrorAction SilentlyContinue
    if ($pathTool) {
        return $pathTool.Source
    }

    $kitsRoot = "${env:ProgramFiles(x86)}\Windows Kits\10\bin"
    if (Test-Path -LiteralPath $kitsRoot -PathType Container) {
        $tool = Get-ChildItem -LiteralPath $kitsRoot -Filter signtool.exe -Recurse -ErrorAction SilentlyContinue |
            Where-Object { $_.FullName -match '\\x64\\signtool\.exe$' } |
            Sort-Object FullName -Descending |
            Select-Object -First 1
        if ($tool) {
            return $tool.FullName
        }
    }

    throw "signtool.exe was not found. Install the Windows SDK or pass -SignToolPath."
}

function Invoke-AuthenticodeSign {
    param([Parameter(Mandatory = $true)][string]$Path)

    if ([string]::IsNullOrWhiteSpace($SignCertSha1) -and [string]::IsNullOrWhiteSpace($SignPfxPath)) {
        return
    }

    $resolved = Resolve-Path -LiteralPath $Path -ErrorAction SilentlyContinue
    if (-not $resolved) {
        throw "Cannot sign missing file: $Path"
    }

    $signtool = Get-SignTool
    $arguments = @("sign", "/fd", "SHA256", "/tr", $TimestampUrl, "/td", "SHA256")
    if (-not [string]::IsNullOrWhiteSpace($SignPfxPath)) {
        $pfx = Resolve-Path -LiteralPath $SignPfxPath -ErrorAction SilentlyContinue
        if (-not $pfx) {
            throw "PFX signing certificate does not exist: $SignPfxPath"
        }
        $arguments += @("/f", $pfx.ProviderPath)
        if (-not [string]::IsNullOrWhiteSpace($SignPfxPassword)) {
            $arguments += @("/p", $SignPfxPassword)
        }
    }
    else {
        $arguments += @("/sha1", $SignCertSha1)
    }
    $arguments += $resolved.ProviderPath

    Invoke-Checked $signtool $arguments
}

function Write-Sha256Manifest {
    param(
        [Parameter(Mandatory = $true)][string[]]$Paths,
        [Parameter(Mandatory = $true)][string]$OutputPath
    )

    $lines = foreach ($path in $Paths) {
        if (Test-Path -LiteralPath $path -PathType Leaf) {
            $hash = Get-FileHash -LiteralPath $path -Algorithm SHA256
            $name = Split-Path -Leaf $path
            "{0}  {1}" -f $hash.Hash.ToLowerInvariant(), $name
        }
    }

    if ($lines) {
        Set-Content -LiteralPath $OutputPath -Value $lines -Encoding ASCII
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

function Test-ExtractorSourceHasLauncherTools {
    param([Parameter(Mandatory = $true)][string]$Path)

    foreach ($file in @(
            "ad.exe",
            "vmap_extractor.exe",
            "vmap_assembler.exe",
            "MoveMapGen.exe",
            "config.json",
            "offmesh.txt"
        )) {
        if (-not (Test-Path -LiteralPath (Join-Path $Path $file) -PathType Leaf)) {
            return $false
        }
    }

    return $true
}

function Get-ExtractorSource {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [string]$OverridePath
    )

    if (-not [string]::IsNullOrWhiteSpace($OverridePath)) {
        $resolved = Resolve-Path -LiteralPath $OverridePath -ErrorAction SilentlyContinue
        if (-not $resolved) {
            throw "Extractor source path does not exist: $OverridePath"
        }
        if (-not (Test-ExtractorSourceHasLauncherTools $resolved.ProviderPath)) {
            throw "Extractor source path does not contain the required launcher extractor tools: $($resolved.ProviderPath)"
        }
        return $resolved.ProviderPath
    }

    $existing = Join-Path $RepoRoot "build-cmangos-tools\bin\x64_Release\Extractors"
    if (Test-ExtractorSourceHasLauncherTools $existing) {
        return $existing
    }

    $buildRoot = Join-Path $RepoRoot "target\launcher-extractor-build"
    $binaryRoot = Join-Path $buildRoot "bin\x64_Release\Extractors"
    if (-not (Test-ExtractorSourceHasLauncherTools $binaryRoot)) {
        Invoke-Checked cmake @(
            "-S", $RepoRoot,
            "-B", $buildRoot,
            "-G", "Visual Studio 17 2022",
            "-A", "x64",
            "-DBUILD_GAME_SERVER=OFF",
            "-DBUILD_LOGIN_SERVER=OFF",
            "-DBUILD_EXTRACTORS=ON",
            "-DBUILD_SCRIPTDEV=OFF",
            "-DBUILD_PLAYERBOTS=OFF",
            "-DBUILD_DEPRECATED_PLAYERBOT=OFF",
            "-DPCH=OFF",
            "-DDEV_BINARY_DIR=$buildRoot"
        ) | Out-Host
        Invoke-Checked cmake @(
            "--build", $buildRoot,
            "--config", "Release",
            "--target", "ad", "vmap_extractor", "vmap_assembler", "MoveMapGen"
        ) | Out-Host
    }

    if (-not (Test-ExtractorSourceHasLauncherTools $binaryRoot)) {
        throw "CMaNGOS extractor tools were not found or built at $binaryRoot"
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

Invoke-AuthenticodeSign (Join-Path $appRoot "RustyMangosLauncher.exe")
Invoke-AuthenticodeSign (Join-Path $appRoot "server\authserver.exe")
Invoke-AuthenticodeSign (Join-Path $appRoot "server\worldserver.exe")

$commit = (& git rev-parse HEAD).Trim()
$branch = (& git rev-parse --abbrev-ref HEAD).Trim()
$remote = ((& git config --get remote.origin.url 2>$null) -join "").Trim()
if ([string]::IsNullOrWhiteSpace($remote)) {
    $remote = "https://github.com/subhei999/rusty-mangos"
}
$dirty = ((& git status --short) -join "`n").Trim()
$dirtyState = "clean"
if (-not [string]::IsNullOrWhiteSpace($dirty)) {
    $dirtyState = "dirty"
}
$buildInfo = @"
Rusty MaNGOS launcher package

Source branch: $branch
Source repository: $remote
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
$extractorSource = Get-ExtractorSource $repoRoot $ExtractorSourcePath
$requiredExtractorFiles = @(
    "ad.exe",
    "vmap_extractor.exe",
    "vmap_assembler.exe",
    "MoveMapGen.exe",
    "config.json",
    "offmesh.txt"
)
foreach ($file in $requiredExtractorFiles) {
    $source = Join-Path $extractorSource $file
    if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
        throw "Extractor source is missing required first-run tool: $source"
    }
    Copy-Item -LiteralPath $source -Destination (Join-Path $appRoot "tools\extractors\$file") -Force
}
foreach ($file in @("zlib.dll", "MoveMapGen.sh", "ExtractResources.sh")) {
    $source = Join-Path $extractorSource $file
    if (Test-Path -LiteralPath $source -PathType Leaf) {
        Copy-Item -LiteralPath $source -Destination (Join-Path $appRoot "tools\extractors\$file") -Force
    }
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
    Write-Sha256Manifest @(
        (Join-Path $appRoot "RustyMangosLauncher.exe"),
        (Join-Path $appRoot "server\authserver.exe"),
        (Join-Path $appRoot "server\worldserver.exe")
    ) (Join-Path $packageRoot "SHA256SUMS.txt")
    Write-Host "Packaged app folder: $appRoot"
    exit 0
}

$iscc = Get-InnoCompiler $repoRoot
Invoke-Checked $iscc @("installer\RustyMangos.iss")
$installerPath = Join-Path $packageRoot "installer\RustyMangosSetup.exe"
Invoke-AuthenticodeSign $installerPath
Write-Sha256Manifest @(
    $installerPath,
    (Join-Path $appRoot "RustyMangosLauncher.exe"),
    (Join-Path $appRoot "server\authserver.exe"),
    (Join-Path $appRoot "server\worldserver.exe")
) (Join-Path $packageRoot "installer\SHA256SUMS.txt")
Write-Host "Installer output: $installerPath"
