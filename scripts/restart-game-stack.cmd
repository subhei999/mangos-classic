@echo off
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0restart-game-stack.ps1" %*
