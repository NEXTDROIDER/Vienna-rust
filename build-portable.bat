@echo off
setlocal

cd /d "%~dp0"

echo === Vienna Portable Builder ===

set "BUILD_PS=%~dp0build-portable.ps1"
set "ICON=%~dp0icon.ico"
set "SRC=assets\run.ps1"
set "OUT=server-portable\ViennaLauncher.exe"

REM 1. Запуск build-portable.ps1
if exist "%BUILD_PS%" (
    echo [INFO] Запуск build-portable.ps1...
    powershell -NoProfile -ExecutionPolicy Bypass -File "%BUILD_PS%"
    if errorlevel 1 (
        echo [ERROR] build-portable.ps1 завершился с ошибкой
        pause
        exit /b 1
    )
) else (
    echo [WARN] build-portable.ps1 не найден, пропускаю
)

REM 2. Проверка исходника
if not exist "%SRC%" (
    echo [ERROR] run.ps1 не найден: %SRC%
    pause
    exit /b 1
)

REM 3. Установка ps2exe если нет
echo [INFO] Проверка ps2exe...
powershell -NoProfile -ExecutionPolicy Bypass -Command ^
"if (-not (Get-Module -ListAvailable -Name ps2exe)) { Install-Module ps2exe -Scope CurrentUser -Force }"

if errorlevel 1 (
    echo [ERROR] Не удалось установить ps2exe
    pause
    exit /b 1
)

REM 4. Сборка EXE
echo [INFO] Сборка EXE...

if exist "%ICON%" (
    powershell -NoProfile -ExecutionPolicy Bypass -Command ^
    "Invoke-PS2EXE '%SRC%' '%OUT%' -noConsole -iconFile '%ICON%'"
) else (
    echo [WARN] icon.ico не найден, сборка без иконки
    powershell -NoProfile -ExecutionPolicy Bypass -Command ^
    "Invoke-PS2EXE '%SRC%' '%OUT%' -noConsole"
)

if errorlevel 1 (
    echo [ERROR] Ошибка сборки EXE
    pause
    exit /b 1
)

echo.
echo === DONE ===
echo Output: %OUT%
echo.

pause
endlocal