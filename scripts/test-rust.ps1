param(
    [switch]$SkipClippy
)

$ErrorActionPreference = "Stop"

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "cargo was not found on PATH. Install Rust with rustup, then reopen this terminal."
}

cargo fmt --check

if (-not $SkipClippy) {
    cargo clippy --workspace --all-targets -- -D warnings
}

cargo test --workspace
cargo build -p authserver
