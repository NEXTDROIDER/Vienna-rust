$ErrorActionPreference = "SilentlyContinue"

$bundleRoot = if ($PSScriptRoot) { $PSScriptRoot } else { Split-Path -Parent $PSCommandPath }
$statePath = Join-Path $bundleRoot "run-state.json"

if (-not (Test-Path -LiteralPath $statePath)) {
    Write-Host "No running Vienna processes were recorded."
    exit 0
}

$entries = Get-Content -LiteralPath $statePath -Raw | ConvertFrom-Json
foreach ($entry in @($entries)) {
    try {
        Stop-Process -Id $entry.Id -Force
        Write-Host "Stopped $($entry.Name) (PID $($entry.Id))"
    }
    catch {
    }
}

Remove-Item -LiteralPath $statePath -Force -ErrorAction SilentlyContinue
