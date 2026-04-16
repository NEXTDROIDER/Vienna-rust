Add-Type -AssemblyName PresentationFramework

[xml]$xaml = @"
<Window xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
        Title="Vienna Server Manager" Height="430" Width="560"
        WindowStartupLocation="CenterScreen">
    <Grid>
        <StackPanel Margin="10">
            <TextBlock Text="Vienna Server Control" FontSize="18" Margin="0,0,0,10"/>
            <WrapPanel Margin="0,0,0,10">
                <Button Name="StartBtn" Content="Start Servers" Width="160" Height="40" Margin="0,5,10,5"/>
                <Button Name="StopBtn" Content="Stop Servers" Width="160" Height="40" Margin="0,5,10,5"/>
                <Button Name="RefreshBtn" Content="Refresh Paths" Width="160" Height="40" Margin="0,5,0,5"/>
            </WrapPanel>
            <TextBlock Name="StatusText" Text="Idle" Margin="0,0,0,8"/>
            <TextBox Name="LogBox" Height="300" AcceptsReturn="True" IsReadOnly="True" VerticalScrollBarVisibility="Auto"/>
        </StackPanel>
    </Grid>
</Window>
"@

$reader = New-Object System.Xml.XmlNodeReader $xaml
$window = [Windows.Markup.XamlReader]::Load($reader)

$StartBtn = $window.FindName("StartBtn")
$StopBtn = $window.FindName("StopBtn")
$RefreshBtn = $window.FindName("RefreshBtn")
$StatusText = $window.FindName("StatusText")
$LogBox = $window.FindName("LogBox")

$script:Processes = @()
$script:Config = $null

function Write-Log {
    param([string]$Message)

    $timestamp = Get-Date -Format "HH:mm:ss"
    $LogBox.AppendText("[$timestamp] $Message`r`n")
    $LogBox.ScrollToEnd()
}

function Set-Status {
    param([string]$Message)

    $StatusText.Text = $Message
    Write-Log $Message
}

function Get-Config {
    $scriptPath = $null
    if ($PSCommandPath) {
        $scriptPath = $PSCommandPath
    }
    elseif ($MyInvocation -and $MyInvocation.MyCommand -and $MyInvocation.MyCommand.Path) {
        $scriptPath = $MyInvocation.MyCommand.Path
    }
    elseif ($PSScriptRoot) {
        $scriptDir = $PSScriptRoot
    }

    if (-not $scriptDir) {
        if ($scriptPath) {
            $scriptDir = Split-Path -Parent $scriptPath
        }
        else {
            $scriptDir = (Get-Location).Path
        }
    }

    if ((Split-Path -Leaf $scriptDir) -ieq "debug" -and (Split-Path -Leaf (Split-Path -Parent $scriptDir)) -ieq "target") {
        $workspaceRoot = Split-Path -Parent (Split-Path -Parent $scriptDir)
    }
    else {
        $workspaceRoot = Split-Path -Parent $scriptDir
    }

    $targetDir = Join-Path $workspaceRoot "target\debug"
    $modsDir = Join-Path $workspaceRoot "mods"
    $dataDir = Join-Path $workspaceRoot "data"
    $objectstoreDataDir = Join-Path $dataDir "data"

    [pscustomobject]@{
        ScriptDir = $scriptDir
        WorkspaceRoot = $workspaceRoot
        TargetDir = $targetDir
        ModsDir = $modsDir
        DataDir = $dataDir
        ObjectstoreDataDir = $objectstoreDataDir
        EventbusExe = Join-Path $targetDir "vienna-eventbus-server.exe"
        ObjectstoreExe = Join-Path $targetDir "vienna-objectstore-server.exe"
        ApiserverExe = Join-Path $targetDir "vienna-apiserver.exe"
    }
}

function Refresh-Config {
    $script:Config = Get-Config

    Write-Log "Workspace root: $($script:Config.WorkspaceRoot)"
    Write-Log "Target dir: $($script:Config.TargetDir)"
    Write-Log "Mods dir: $($script:Config.ModsDir)"
}

function Test-RequiredFiles {
    $required = @(
        $script:Config.EventbusExe,
        $script:Config.ObjectstoreExe,
        $script:Config.ApiserverExe
    )

    foreach ($path in $required) {
        if (-not (Test-Path -LiteralPath $path)) {
            throw "Required file not found: $path"
        }
    }
}

function Ensure-Directories {
    foreach ($path in @($script:Config.ModsDir, $script:Config.DataDir, $script:Config.ObjectstoreDataDir)) {
        if (-not (Test-Path -LiteralPath $path)) {
            New-Item -ItemType Directory -Path $path -Force | Out-Null
            Write-Log "Created directory: $path"
        }
    }
}

function Start-ViennaProcess {
    param(
        [Parameter(Mandatory)][string]$FilePath,
        [string[]]$Arguments = @(),
        [Parameter(Mandatory)][string]$Name
    )

    $formattedArguments = @()
    foreach ($argument in $Arguments) {
        if ($null -eq $argument) {
            continue
        }

        $text = [string]$argument
        if ($text.Contains('"')) {
            $text = $text.Replace('"', '\"')
        }

        if ($text -match '\s') {
            $formattedArguments += ('"{0}"' -f $text)
        }
        else {
            $formattedArguments += $text
        }
    }

    $argumentLine = if ($formattedArguments.Count -gt 0) { $formattedArguments -join " " } else { "" }
    Write-Log "Starting $Name -> $FilePath $argumentLine"

    if ($formattedArguments.Count -gt 0) {
        $process = Start-Process -FilePath $FilePath -ArgumentList $argumentLine -WorkingDirectory $script:Config.WorkspaceRoot -PassThru
    }
    else {
        $process = Start-Process -FilePath $FilePath -WorkingDirectory $script:Config.WorkspaceRoot -PassThru
    }

    $entry = [pscustomobject]@{
        Name = $Name
        Process = $process
    }
    $script:Processes += $entry
    return $entry
}

function Wait-ForPort {
    param(
        [Parameter(Mandatory)][string]$HostName,
        [Parameter(Mandatory)][int]$Port,
        [object]$ProcessEntry = $null,
        [int]$TimeoutSeconds = 20
    )

    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        try {
            if ($ProcessEntry -and $ProcessEntry.Process -and $ProcessEntry.Process.HasExited) {
                throw "$($ProcessEntry.Name) exited with code $($ProcessEntry.Process.ExitCode)"
            }

            $client = New-Object System.Net.Sockets.TcpClient
            $client.ReceiveTimeout = 500
            $client.SendTimeout = 500
            $client.Connect($HostName, $Port)
            if ($client.Connected) {
                $client.Close()
                Write-Log "${HostName}:$Port is ready"
                return
            }
        }
        catch {
            if ($_.Exception.Message -like "*exited with code*") {
                throw
            }
        }
        finally {
            if ($client -is [System.Net.Sockets.TcpClient]) {
                $client.Close()
            }
        }

        Start-Sleep -Milliseconds 500
    }

    throw "Timed out waiting for ${HostName}:$Port"
}

function Stop-Vienna {
    foreach ($entry in @($script:Processes)) {
        try {
            if ($entry.Process -and -not $entry.Process.HasExited) {
                Write-Log "Stopping $($entry.Name) (PID $($entry.Process.Id))"
                Stop-Process -Id $entry.Process.Id -Force
            }
        }
        catch {
            Write-Log "Could not stop $($entry.Name): $($_.Exception.Message)"
        }
    }

    $script:Processes = @()
}

function Stop-ExistingViennaProcesses {
    $names = @(
        "vienna-apiserver",
        "vienna-eventbus-server",
        "vienna-objectstore-server"
    )

    foreach ($name in $names) {
        Get-Process -Name $name -ErrorAction SilentlyContinue | ForEach-Object {
            try {
                Write-Log "Killing existing process $($_.ProcessName) (PID $($_.Id))"
                Stop-Process -Id $_.Id -Force
            }
            catch {
                Write-Log "Could not kill $($_.ProcessName): $($_.Exception.Message)"
            }
        }
    }
}

function Start-Vienna {
    Test-RequiredFiles
    Ensure-Directories
    Stop-ExistingViennaProcesses
    $script:Processes = @()

    $eventbusEntry = Start-ViennaProcess -Name "Event Bus" -FilePath $script:Config.EventbusExe
    Wait-ForPort -HostName "127.0.0.1" -Port 5532 -ProcessEntry $eventbusEntry

    $objectstoreEntry = Start-ViennaProcess -Name "Object Store" -FilePath $script:Config.ObjectstoreExe -Arguments @(
        "--data-dir", $script:Config.ObjectstoreDataDir,
        "--port", "5396"
    )
    Wait-ForPort -HostName "127.0.0.1" -Port 5396 -ProcessEntry $objectstoreEntry

    $apiserverEntry = Start-ViennaProcess -Name "API Server" -FilePath $script:Config.ApiserverExe -Arguments @(
        "--db", (Join-Path $script:Config.WorkspaceRoot "earth.db"),
        "--static-data", $script:Config.DataDir,
        "--eventbus", "localhost:5532",
        "--objectstore", "localhost:5396",
        "--mods-dir", $script:Config.ModsDir,
        "--port", "8080"
    )
    Wait-ForPort -HostName "127.0.0.1" -Port 8080 -ProcessEntry $apiserverEntry
}

$StartBtn.Add_Click({
    try {
        Set-Status "Starting Vienna services"
        Start-Vienna
        Set-Status "Vienna services are running"
    }
    catch {
        Set-Status "Start failed"
        Write-Log $_.Exception.Message
    }
})

$StopBtn.Add_Click({
    Set-Status "Stopping Vienna services"
    Stop-Vienna
    Set-Status "Vienna services stopped"
})

$RefreshBtn.Add_Click({
    Refresh-Config
    Set-Status "Paths refreshed"
})

$window.Add_Closing({
    Stop-Vienna
})

Refresh-Config
Set-Status "Ready"
[void]$window.ShowDialog()
