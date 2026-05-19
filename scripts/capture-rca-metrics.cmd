@echo off
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0capture-rca-metrics.ps1" %*
