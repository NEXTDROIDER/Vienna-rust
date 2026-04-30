$ErrorActionPreference = "Stop"

$workspaceRoot = if ($PSScriptRoot) { $PSScriptRoot } else { Split-Path -Parent $PSCommandPath }
$targetDebugDir = Join-Path $workspaceRoot "target\debug"
$copySourceDir = Join-Path $workspaceRoot "Move files in folder to target debug"

Write-Host "Building Vienna workspace..."
Push-Location $workspaceRoot
try {
    if ($args.Count -gt 0) {
        cargo build --workspace @args
    }
    else {
        cargo build --workspace
    }
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
}
finally {
    Pop-Location
}

if (-not (Test-Path -LiteralPath $targetDebugDir)) {
    New-Item -ItemType Directory -Path $targetDebugDir -Force | Out-Null
}

if (-not (Test-Path -LiteralPath $copySourceDir)) {
    throw "Copy source folder not found: $copySourceDir"
}

Write-Host "Copying extra debug files..."
Get-ChildItem -LiteralPath $copySourceDir -Force | ForEach-Object {
    $destination = Join-Path $targetDebugDir $_.Name
    Copy-Item -LiteralPath $_.FullName -Destination $destination -Recurse -Force
    Write-Host ("Copied {0}" -f $_.Name)
}

Write-Host "Build is ready at: $targetDebugDir"
