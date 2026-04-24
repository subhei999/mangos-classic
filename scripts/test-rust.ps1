param(
    [switch]$SkipClippy
)

$ErrorActionPreference = "Stop"

function Invoke-Cargo {
    param(
        [Parameter(Mandatory = $true)]
        [string[]]$Arguments
    )

    & cargo @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "cargo $($Arguments -join ' ') failed with exit code $LASTEXITCODE"
    }
}

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "cargo was not found on PATH. Install Rust with rustup, then reopen this terminal."
}

Invoke-Cargo @("fmt", "--check")

if (-not $SkipClippy) {
    Invoke-Cargo @("clippy", "--workspace", "--all-targets", "--", "-D", "warnings")
}

Invoke-Cargo @("test", "--workspace")
Invoke-Cargo @("build", "-p", "authserver")
Invoke-Cargo @("build", "-p", "auth-flow-test")
