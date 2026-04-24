@echo off
set "RUST_LOG=info"
cd /d "%~dp0\.."
target\debug\authserver.exe --config config\authserver.local.toml > auth-client-13724.log 2>&1
