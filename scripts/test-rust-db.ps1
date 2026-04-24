param(
    [switch]$KeepRunning
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

if (-not (Get-Command docker -ErrorAction SilentlyContinue)) {
    throw "docker was not found on PATH. Install/start Docker Desktop first."
}

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

$process = Start-Process cargo `
    -ArgumentList @("run", "-p", "authserver", "--", "--config", "config/authserver.local.toml") `
    -WorkingDirectory (Get-Location) `
    -NoNewWindow `
    -PassThru

Start-Sleep -Seconds 5

if ($process.HasExited) {
    throw "authserver exited during startup with code $($process.ExitCode)"
}

Write-Host "authserver started successfully against local MariaDB using config/authserver.local.toml."

if (-not $KeepRunning) {
    Stop-Process -Id $process.Id -Force
}
