Add-Type -AssemblyName PresentationFramework
Add-Type -AssemblyName System.Windows.Forms

[xml]$xaml = @"
<Window xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
        Title="Vienna Portable Server" Height="430" Width="760"
        WindowStartupLocation="CenterScreen">
    <Grid>
        <StackPanel Margin="10">
            <TextBlock Text="Vienna Portable Server" FontSize="18" Margin="0,0,0,10"/>
            <WrapPanel Margin="0,0,0,10">
                <Button Name="StartBtn" Content="Start Servers" Width="140" Height="40" Margin="0,5,10,5"/>
                <Button Name="StopBtn" Content="Stop Servers" Width="140" Height="40" Margin="0,5,10,5"/>
                <Button Name="OpenLogsBtn" Content="Open Logs" Width="120" Height="40" Margin="0,5,10,5"/>
                <Button Name="ClearLogsBtn" Content="Clear Logs" Width="120" Height="40" Margin="0,5,10,5"/>
                <Button Name="UpdateBtn" Content="Update Server" Width="140" Height="40" Margin="0,5,0,5"/>
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
$OpenLogsBtn = $window.FindName("OpenLogsBtn")
$ClearLogsBtn = $window.FindName("ClearLogsBtn")
$UpdateBtn = $window.FindName("UpdateBtn")
$StatusText = $window.FindName("StatusText")
$LogBox = $window.FindName("LogBox")

$script:Processes = @()
$script:BundleRoot = if ($PSScriptRoot) { $PSScriptRoot } else { Split-Path -Parent $PSCommandPath }
$script:BinDir = Join-Path $script:BundleRoot "bin"
$script:DataDir = Join-Path $script:BundleRoot "data"
$script:ObjectstoreDataDir = Join-Path $script:DataDir "data"
$script:ModsDir = Join-Path $script:BundleRoot "mods"
$script:LogsDir = Join-Path $script:BundleRoot "logs"
$script:StatePath = Join-Path $script:BundleRoot "run-state.json"
$script:ResourcePackFile = Join-Path $script:BundleRoot "data/pack1.zip"
$script:VersionFile = Join-Path $script:BundleRoot "VERSION"
$script:ApiPort = 8080
$script:CdnPort = 8081
$script:LocatorPort = 8082
$script:EventbusPort = 5532
$script:ObjectstorePort = 5396

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

function Get-ServerVersion {
    if (Test-Path -LiteralPath $script:VersionFile) {
        return (Get-Content -LiteralPath $script:VersionFile -Raw).Trim()
    }

    return "dev"
}

function Ensure-Directory {
    param([Parameter(Mandatory)][string]$Path)

    if (-not (Test-Path -LiteralPath $Path)) {
        New-Item -ItemType Directory -Path $Path -Force | Out-Null
    }
}

function Test-RequiredFiles {
    $required = @(
        (Join-Path $script:BinDir "vienna-eventbus-server.exe"),
        (Join-Path $script:BinDir "vienna-objectstore-server.exe"),
        (Join-Path $script:BinDir "vienna-apiserver.exe"),
        (Join-Path $script:BinDir "vienna-cdn.exe"),
        (Join-Path $script:BinDir "vienna-locator.exe")
    )

    foreach ($path in $required) {
        if (-not (Test-Path -LiteralPath $path)) {
            throw "Required file not found: $path"
        }
    }
}

function New-QuotedArgumentLine {
    param([string[]]$Arguments)

    $formatted = @()
    foreach ($argument in $Arguments) {
        if ($null -eq $argument) {
            continue
        }

        $text = [string]$argument
        if ($text.Contains('"')) {
            $text = $text.Replace('"', '\"')
        }

        if ($text -match '\s') {
            $formatted += ('"{0}"' -f $text)
        }
        else {
            $formatted += $text
        }
    }

    return ($formatted -join " ")
}

function Start-ViennaProcess {
    param(
        [Parameter(Mandatory)][string]$Name,
        [Parameter(Mandatory)][string]$FilePath,
        [string[]]$Arguments = @()
    )

    Ensure-Directory $script:LogsDir
    $stdoutPath = Join-Path $script:LogsDir (($Name -replace '\s+', '-').ToLowerInvariant() + ".stdout.log")
    $stderrPath = Join-Path $script:LogsDir (($Name -replace '\s+', '-').ToLowerInvariant() + ".stderr.log")
    $argumentLine = New-QuotedArgumentLine -Arguments $Arguments

    Write-Log "Starting $Name -> $FilePath $argumentLine"

    $startInfo = New-Object System.Diagnostics.ProcessStartInfo
    $startInfo.FileName = $FilePath
    $startInfo.Arguments = $argumentLine
    $startInfo.WorkingDirectory = $script:BundleRoot
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true

    $process = New-Object System.Diagnostics.Process
    $process.StartInfo = $startInfo
    $process.EnableRaisingEvents = $true

    $stdoutWriter = New-Object System.IO.StreamWriter($stdoutPath, $true)
    $stderrWriter = New-Object System.IO.StreamWriter($stderrPath, $true)
    $stdoutWriter.AutoFlush = $true
    $stderrWriter.AutoFlush = $true

    $process.add_OutputDataReceived({
        param($sender, $eventArgs)
        if ($eventArgs.Data) {
            $stdoutWriter.WriteLine($eventArgs.Data)
        }
    })
    $process.add_ErrorDataReceived({
        param($sender, $eventArgs)
        if ($eventArgs.Data) {
            $stderrWriter.WriteLine($eventArgs.Data)
        }
    })
    $process.add_Exited({
        $stdoutWriter.Dispose()
        $stderrWriter.Dispose()
    })

    [void]$process.Start()
    $process.BeginOutputReadLine()
    $process.BeginErrorReadLine()

    $entry = [pscustomobject]@{
        Name = $Name
        Process = $process
        StdoutPath = $stdoutPath
        StderrPath = $stderrPath
    }

    $script:Processes += $entry
    return $entry
}

function Wait-ForPort {
    param(
        [Parameter(Mandatory)][string]$HostName,
        [Parameter(Mandatory)][int]$Port,
        [Parameter(Mandatory)][object]$ProcessEntry,
        [int]$TimeoutSeconds = 20
    )

    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        try {
            if ($ProcessEntry.Process.HasExited) {
                $stderrTail = ""
                if (Test-Path -LiteralPath $ProcessEntry.StderrPath) {
                    $stderrTail = (Get-Content -LiteralPath $ProcessEntry.StderrPath -Tail 10 -ErrorAction SilentlyContinue) -join " "
                }

                if ([string]::IsNullOrWhiteSpace($stderrTail)) {
                    throw "$($ProcessEntry.Name) exited with code $($ProcessEntry.Process.ExitCode)"
                }

                throw "$($ProcessEntry.Name) exited with code $($ProcessEntry.Process.ExitCode): $stderrTail"
            }

            $client = New-Object System.Net.Sockets.TcpClient
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

function Save-State {
    $state = $script:Processes | ForEach-Object {
        [pscustomobject]@{
            Name = $_.Name
            Id = $_.Process.Id
        }
    }

    $state | ConvertTo-Json -Depth 3 | Set-Content -LiteralPath $script:StatePath -Encoding UTF8
}

function Stop-Vienna {
    foreach ($entry in @($script:Processes)) {
        try {
            if ($entry.Process -and -not $entry.Process.HasExited) {
                Write-Log "Stopping $($entry.Name) (PID $($entry.Process.Id))"
                $entry.Process.Kill()
                $entry.Process.WaitForExit(3000) | Out-Null
            }
        }
        catch {
            Write-Log "Could not stop $($entry.Name): $($_.Exception.Message)"
        }
    }

    $script:Processes = @()
    if (Test-Path -LiteralPath $script:StatePath) {
        Remove-Item -LiteralPath $script:StatePath -Force -ErrorAction SilentlyContinue
    }
}

function Has-RunningViennaProcesses {
    foreach ($entry in @($script:Processes)) {
        try {
            if ($entry.Process -and -not $entry.Process.HasExited) {
                return $true
            }
        }
        catch {
        }
    }

    return $false
}

function Clear-Logs {
    Ensure-Directory $script:LogsDir

    if (Has-RunningViennaProcesses) {
        [System.Windows.MessageBox]::Show(
            "Stop the servers before clearing logs.",
            "Vienna Logs",
            [System.Windows.MessageBoxButton]::OK,
            [System.Windows.MessageBoxImage]::Warning
        ) | Out-Null
        return
    }

    $result = [System.Windows.MessageBox]::Show(
        "Delete all log files from the logs folder?",
        "Clear Vienna Logs",
        [System.Windows.MessageBoxButton]::YesNo,
        [System.Windows.MessageBoxImage]::Question
    )

    if ($result -ne [System.Windows.MessageBoxResult]::Yes) {
        Write-Log "Clear logs cancelled"
        return
    }

    Get-ChildItem -LiteralPath $script:LogsDir -File -ErrorAction SilentlyContinue | ForEach-Object {
        Remove-Item -LiteralPath $_.FullName -Force -ErrorAction SilentlyContinue
    }

    Set-Status "Logs cleared"
}

function Start-UpdateFlow {
    if (Has-RunningViennaProcesses) {
        [System.Windows.MessageBox]::Show(
            "Stop the servers before updating.",
            "Vienna Update",
            [System.Windows.MessageBoxButton]::OK,
            [System.Windows.MessageBoxImage]::Warning
        ) | Out-Null
        return
    }

    $dialog = New-Object System.Windows.Forms.FolderBrowserDialog
    $dialog.Description = "Select the new server-portable folder to update from"

    if ($dialog.ShowDialog() -ne [System.Windows.Forms.DialogResult]::OK) {
        Write-Log "Update cancelled"
        return
    }

    $updateScript = Join-Path $script:BundleRoot "update.ps1"
    if (-not (Test-Path -LiteralPath $updateScript)) {
        Set-Status "Update failed"
        Write-Log "Missing update script: $updateScript"
        return
    }

    Set-Status "Updating server files"
    $process = Start-Process -FilePath "powershell.exe" -ArgumentList @(
        "-ExecutionPolicy", "Bypass",
        "-File", $updateScript,
        "-Source", $dialog.SelectedPath
    ) -WorkingDirectory $script:BundleRoot -PassThru -Wait

    if ($process.ExitCode -eq 0) {
        Set-Status "Update finished"
    }
    else {
        Set-Status "Update failed"
        Write-Log "Updater exited with code $($process.ExitCode)"
    }
}

function Stop-ExistingViennaProcesses {
    if (-not (Test-Path -LiteralPath $script:StatePath)) {
        return
    }

    try {
        $entries = Get-Content -LiteralPath $script:StatePath -Raw | ConvertFrom-Json
        foreach ($entry in @($entries)) {
            try {
                $process = Get-Process -Id $entry.Id -ErrorAction Stop
                Write-Log "Killing existing process $($entry.Name) (PID $($entry.Id))"
                Stop-Process -Id $entry.Id -Force
            }
            catch {
            }
        }
    }
    finally {
        Remove-Item -LiteralPath $script:StatePath -Force -ErrorAction SilentlyContinue
    }
}

function Start-Vienna {
    Test-RequiredFiles
    Ensure-Directory $script:DataDir
    Ensure-Directory $script:ObjectstoreDataDir
    Ensure-Directory $script:ModsDir
    Ensure-Directory $script:LogsDir
    Stop-ExistingViennaProcesses
    $script:Processes = @()

    $eventbus = Start-ViennaProcess -Name "Event Bus" -FilePath (Join-Path $script:BinDir "vienna-eventbus-server.exe")
    Wait-ForPort -HostName "127.0.0.1" -Port $script:EventbusPort -ProcessEntry $eventbus

    $objectstore = Start-ViennaProcess -Name "Object Store" -FilePath (Join-Path $script:BinDir "vienna-objectstore-server.exe") -Arguments @(
        "--data-dir", $script:ObjectstoreDataDir,
        "--port", [string]$script:ObjectstorePort
    )
    Wait-ForPort -HostName "127.0.0.1" -Port $script:ObjectstorePort -ProcessEntry $objectstore

    $cdn = Start-ViennaProcess -Name "CDN" -FilePath (Join-Path $script:BinDir "vienna-cdn.exe") -Arguments @(
        "--port", [string]$script:CdnPort,
        "--resource-pack-file", $script:ResourcePackFile
    )
    Wait-ForPort -HostName "127.0.0.1" -Port $script:CdnPort -ProcessEntry $cdn

    $apiserver = Start-ViennaProcess -Name "API Server" -FilePath (Join-Path $script:BinDir "vienna-apiserver.exe") -Arguments @(
        "--db", (Join-Path $script:BundleRoot "earth.db"),
        "--static-data", $script:DataDir,
        "--eventbus", ("localhost:{0}" -f $script:EventbusPort),
        "--objectstore", ("localhost:{0}" -f $script:ObjectstorePort),
        "--mods-dir", $script:ModsDir,
        "--port", [string]$script:ApiPort
    )
    Wait-ForPort -HostName "127.0.0.1" -Port $script:ApiPort -ProcessEntry $apiserver

    $locator = Start-ViennaProcess -Name "Locator" -FilePath (Join-Path $script:BinDir "vienna-locator.exe") -Arguments @(
        "--port", [string]$script:LocatorPort,
        "--api", ("http://127.0.0.1:{0}" -f $script:ApiPort),
        "--cdn", ("http://127.0.0.1:{0}" -f $script:CdnPort),
        "--playfab-title-id", "ViennaLocal"
    )
    Wait-ForPort -HostName "127.0.0.1" -Port $script:LocatorPort -ProcessEntry $locator

    Save-State
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

$OpenLogsBtn.Add_Click({
    Ensure-Directory $script:LogsDir
    Start-Process explorer.exe $script:LogsDir | Out-Null
})

$ClearLogsBtn.Add_Click({
    Clear-Logs
})

$UpdateBtn.Add_Click({
    Start-UpdateFlow
})

$window.Add_Closing({
    Stop-Vienna
})

Set-Status ("Ready - version {0}" -f (Get-ServerVersion))
[void]$window.ShowDialog()
