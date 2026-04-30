@echo off
if "%~1"=="" (
    powershell -ExecutionPolicy Bypass -File "%~dp0build-debug.ps1"
) else (
    powershell -ExecutionPolicy Bypass -File "%~dp0build-debug.ps1" %*
)
pause
