@echo off
if "%~1"=="" (
    powershell -ExecutionPolicy Bypass -File "%~dp0build-portable.ps1"
) else (
    powershell -ExecutionPolicy Bypass -File "%~dp0build-portable.ps1" %*
)
pause
