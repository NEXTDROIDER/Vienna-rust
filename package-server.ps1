 $ErrorActionPreference = "Stop"

function Ensure-Directory {
    param([Parameter(Mandatory)][string]$Path)

    if (-not (Test-Path -LiteralPath $Path)) {
        New-Item -ItemType Directory -Path $Path -Force | Out-Null
    }
}

$workspaceRoot = Split-Path -Parent $PSCommandPath
$targetDir = Join-Path $workspaceRoot "target\debug"
$bundleRoot = Join-Path $workspaceRoot "server-portable"
$binDir = Join-Path $bundleRoot "bin"
$dataDir = Join-Path $bundleRoot "data"
$objectstoreDataDir = Join-Path $dataDir "data"
$modsDir = Join-Path $bundleRoot "mods"
$logsDir = Join-Path $bundleRoot "logs"
$vmaSdkDir = Join-Path $bundleRoot "vma-sdk"

$requiredFiles = @(
    (Join-Path $targetDir "vienna-eventbus-server.exe"),
    (Join-Path $targetDir "vienna-objectstore-server.exe"),
    (Join-Path $targetDir "vienna-apiserver.exe"),
    (Join-Path $targetDir "vienna-cdn.exe"),
    (Join-Path $targetDir "vienna-locator.exe")
)

foreach ($path in $requiredFiles) {
    if (-not (Test-Path -LiteralPath $path)) {
        throw "Required binary not found: $path"
    }
}

Ensure-Directory $bundleRoot
Ensure-Directory $binDir
Ensure-Directory $dataDir
Ensure-Directory $objectstoreDataDir
Ensure-Directory $modsDir
Ensure-Directory $logsDir
Ensure-Directory $vmaSdkDir

Get-ChildItem -LiteralPath $targetDir -Filter "vienna-*.exe" -File | ForEach-Object {
    try {
        Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $binDir $_.Name) -Force
    }
    catch {
        Write-Warning ("Could not copy {0}: {1}" -f $_.Name, $_.Exception.Message)
    }
}

Get-ChildItem -LiteralPath $targetDir -Filter "*.dll" -File -ErrorAction SilentlyContinue | ForEach-Object {
    try {
        Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $binDir $_.Name) -Force
    }
    catch {
        Write-Warning ("Could not copy {0}: {1}" -f $_.Name, $_.Exception.Message)
    }
}

if (Test-Path -LiteralPath (Join-Path $workspaceRoot "mods")) {
    Get-ChildItem -LiteralPath (Join-Path $workspaceRoot "mods") -Filter "*.mcemod" -File -ErrorAction SilentlyContinue | ForEach-Object {
        try {
            Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $modsDir $_.Name) -Force
        }
        catch {
            Write-Warning ("Could not copy mod {0}: {1}" -f $_.Name, $_.Exception.Message)
        }
    }
}

$vmaSourceDir = Join-Path $workspaceRoot "rust\vma"
$exampleModSourceDir = Join-Path $workspaceRoot "rust\example-hello-mcemod"

if (Test-Path -LiteralPath $vmaSourceDir) {
    $vmaTargetDir = Join-Path $vmaSdkDir "vma"
    if (Test-Path -LiteralPath $vmaTargetDir) {
        Remove-Item -LiteralPath $vmaTargetDir -Recurse -Force -ErrorAction SilentlyContinue
    }
    Copy-Item -LiteralPath $vmaSourceDir -Destination $vmaTargetDir -Recurse -Force
}

if (Test-Path -LiteralPath $exampleModSourceDir) {
    $exampleModTargetDir = Join-Path $vmaSdkDir "example-hello-mcemod"
    if (Test-Path -LiteralPath $exampleModTargetDir) {
        Remove-Item -LiteralPath $exampleModTargetDir -Recurse -Force -ErrorAction SilentlyContinue
    }
    Copy-Item -LiteralPath $exampleModSourceDir -Destination $exampleModTargetDir -Recurse -Force
}

$vmaReadmePath = Join-Path $vmaSdkDir "README.txt"
@"
Vienna Modding API SDK

Contents:
- vma\                Rust crate with the Vienna Modding API
- example-hello-mcemod\  Example Rust mod that exports a .mcemod plugin

Typical flow:
1. Open the example mod.
2. Replace its logic with your own hooks.
3. Build it as a cdylib.
4. Rename or package the resulting DLL as .mcemod.
5. Put the .mcemod file into the server's mods folder.
"@ | Set-Content -LiteralPath $vmaReadmePath -Encoding Ascii


$runTemplate = Join-Path $workspaceRoot "server-portable\run.ps1"
$stopTemplate = Join-Path $workspaceRoot "server-portable\stop.ps1"
if (-not (Test-Path -LiteralPath $runTemplate)) {
    throw "Missing portable run script: $runTemplate"
}
if (-not (Test-Path -LiteralPath $stopTemplate)) {
    throw "Missing portable stop script: $stopTemplate"
}

Write-Host "Portable server bundle is ready at: $bundleRoot"
