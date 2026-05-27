@echo off
powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0rusty-mangos-launcher.ps1" %*
