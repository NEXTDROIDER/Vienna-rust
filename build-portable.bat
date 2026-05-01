@echo off
cd /d "%~dp0"

echo === Vienna Portable Builder ===

set ICON=%~dp0icon.ico

REM проверка иконки
if not exist "%ICON%" (
echo [WARN] icon.ico не найден, сборка будет без иконки
set ICON_PARAM=
) else (
set ICON_PARAM=-iconFile "%ICON%"
)

REM установка ps2exe если нет
powershell -NoProfile -ExecutionPolicy Bypass -Command ^
"if (-not (Get-Module -ListAvailable -Name ps2exe)) { Install-Module ps2exe -Scope CurrentUser -Force }"

REM сборка exe
powershell -NoProfile -ExecutionPolicy Bypass -Command ^
"Invoke-PS2EXE '%~dp0run.ps1' '%~dp0ViennaLauncher.exe' -noConsole %ICON_PARAM%"

echo.
echo === DONE ===
echo Output: ViennaLauncher.exe
echo.

pause
