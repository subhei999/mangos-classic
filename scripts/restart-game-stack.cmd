@echo off
setlocal EnableExtensions EnableDelayedExpansion
set "SCRIPT_DIR=%~dp0"
if /I "%~1"=="release" (
    shift
    goto run_release
)
if /I "%~1"=="--release" (
    shift
    goto run_release
)
if /I "%~1"=="/release" (
    shift
    goto run_release
)
powershell -NoProfile -ExecutionPolicy Bypass -File "%SCRIPT_DIR%restart-game-stack.ps1" %*
exit /b %ERRORLEVEL%

:run_release
set "EXTRA_ARGS="
:collect_release_args
if "%~1"=="" goto launch_release
set "EXTRA_ARGS=!EXTRA_ARGS! %1"
shift
goto collect_release_args

:launch_release
powershell -NoProfile -ExecutionPolicy Bypass -File "%SCRIPT_DIR%restart-game-stack.ps1" -Release !EXTRA_ARGS!
exit /b %ERRORLEVEL%
