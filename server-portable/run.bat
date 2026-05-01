@echo off
cd /d "%~dp0"
powershell -NoExit -ExecutionPolicy Bypass -File "run.ps1"
pause