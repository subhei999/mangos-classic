@echo off
powershell -ExecutionPolicy Bypass -File "%~dp0test-rust-db.ps1" %*
