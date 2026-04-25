@echo off
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0import-classic-db-world.ps1" %*
