param(
    [Parameter(Mandatory)]
    [string]$Source
)

$ErrorActionPreference = "Stop"

$targetRoot = if ($PSScriptRoot) { $PSScriptRoot } else { Split-Path -Parent $PSCommandPath }
$sourceRoot = (Resolve-Path -LiteralPath $Source).Path
$backupRoot = Join-Path $targetRoot ("backups\update-" + (Get-Date -Format "yyyyMMdd-HHmmss"))

function Ensure-Directory {
    param([Parameter(Mandatory)][string]$Path)

    if (-not (Test-Path -LiteralPath $Path)) {
        New-Item -ItemType Directory -Path $Path -Force | Out-Null
    }
}

function Copy-DirectoryReplacing {
    param(
        [Parameter(Mandatory)][string]$SourcePath,
        [Parameter(Mandatory)][string]$TargetPath
    )

    if (-not (Test-Path -LiteralPath $SourcePath)) {
        return
    }

    if (Test-Path -LiteralPath $TargetPath) {
        $relative = Split-Path -Leaf $TargetPath
        Ensure-Directory $backupRoot
        Move-Item -LiteralPath $TargetPath -Destination (Join-Path $backupRoot $relative) -Force
    }

    Copy-Item -LiteralPath $SourcePath -Destination $TargetPath -Recurse -Force
}

function Copy-DirectoryMergingMissing {
    param(
        [Parameter(Mandatory)][string]$SourcePath,
        [Parameter(Mandatory)][string]$TargetPath
    )

    if (-not (Test-Path -LiteralPath $SourcePath)) {
        return
    }

    Ensure-Directory $TargetPath
    Get-ChildItem -LiteralPath $SourcePath -Recurse -File | ForEach-Object {
        $relative = $_.FullName.Substring($SourcePath.Length).TrimStart("\", "/")
        $targetFile = Join-Path $TargetPath $relative
        $targetDir = Split-Path -Parent $targetFile
        Ensure-Directory $targetDir

        if (-not (Test-Path -LiteralPath $targetFile)) {
            Copy-Item -LiteralPath $_.FullName -Destination $targetFile -Force
        }
        else {
            Write-Host "Keeping existing file: $targetFile"
        }
    }
}

if ($sourceRoot -eq $targetRoot) {
    throw "Source and target are the same folder."
}

$stopScript = Join-Path $targetRoot "stop.ps1"
if (Test-Path -LiteralPath $stopScript) {
    powershell -ExecutionPolicy Bypass -File $stopScript | Out-Null
}

Ensure-Directory $backupRoot

Copy-DirectoryReplacing -SourcePath (Join-Path $sourceRoot "bin") -TargetPath (Join-Path $targetRoot "bin")
Copy-DirectoryReplacing -SourcePath (Join-Path $sourceRoot "vma-sdk") -TargetPath (Join-Path $targetRoot "vma-sdk")

foreach ($fileName in @("run.ps1", "stop.ps1", "update.ps1", "run.bat", "stop.bat", "resource-pack.zip")) {
    $sourceFile = Join-Path $sourceRoot $fileName
    $targetFile = Join-Path $targetRoot $fileName
    if (Test-Path -LiteralPath $sourceFile) {
        if (Test-Path -LiteralPath $targetFile) {
            Copy-Item -LiteralPath $targetFile -Destination (Join-Path $backupRoot $fileName) -Force
        }
        Copy-Item -LiteralPath $sourceFile -Destination $targetFile -Force
    }
}

Copy-DirectoryMergingMissing -SourcePath (Join-Path $sourceRoot "data") -TargetPath (Join-Path $targetRoot "data")
Copy-DirectoryMergingMissing -SourcePath (Join-Path $sourceRoot "mods") -TargetPath (Join-Path $targetRoot "mods")

Write-Host "Update finished. User data, mods, logs, and earth.db were preserved."
Write-Host "Backup folder: $backupRoot"
