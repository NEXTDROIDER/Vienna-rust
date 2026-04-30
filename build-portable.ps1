$ErrorActionPreference = "Stop"

$workspaceRoot = if ($PSScriptRoot) { $PSScriptRoot } else { Split-Path -Parent $PSCommandPath }
$buildScript = Join-Path $workspaceRoot "build-debug.ps1"
$packageScript = Join-Path $workspaceRoot "package-server.ps1"
$packageArgs = @()
$buildArgs = @()

foreach ($argument in $args) {
    if ($argument -eq "--clean-portable" -or $argument -eq "-CleanPortable") {
        $packageArgs += "-Clean"
    }
    else {
        $buildArgs += $argument
    }
}

if (-not (Test-Path -LiteralPath $buildScript)) {
    throw "Missing build script: $buildScript"
}
if (-not (Test-Path -LiteralPath $packageScript)) {
    throw "Missing package script: $packageScript"
}

Write-Host "Building debug binaries first..."
& powershell -ExecutionPolicy Bypass -File $buildScript @buildArgs
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

Write-Host "Updating server-portable..."
& powershell -ExecutionPolicy Bypass -File $packageScript @packageArgs
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

Write-Host "Portable build is ready: $(Join-Path $workspaceRoot 'server-portable')"
