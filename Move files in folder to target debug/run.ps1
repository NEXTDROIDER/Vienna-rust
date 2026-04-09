Add-Type -AssemblyName PresentationFramework

# --- UI ---
[xml]$xaml = @"
<Window xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
        Title="Vienna Server Manager" Height="400" Width="500">
    <Grid>
        <StackPanel Margin="10">
            <TextBlock Text="Vienna Server Control" FontSize="18" Margin="0,0,0,10"/>
            <Button Name="StartBtn" Content="Start Servers" Height="40" Margin="0,5"/>
            <Button Name="StopBtn" Content="Stop Servers" Height="40" Margin="0,5"/>
            <TextBox Name="LogBox" Height="250" Margin="0,10" AcceptsReturn="True" VerticalScrollBarVisibility="Auto"/>
        </StackPanel>
    </Grid>
</Window>
"@

$reader = (New-Object System.Xml.XmlNodeReader $xaml)
$window = [Windows.Markup.XamlReader]::Load($reader)

# --- Find controls ---
$StartBtn = $window.FindName("StartBtn")
$StopBtn = $window.FindName("StopBtn")
$LogBox = $window.FindName("LogBox")

# --- Config ---
# --- Config ---
$BASE_DIR = "."
$VIENNA_DIR = "."

$EVENTBUS = "$BASE_DIR/vienna-eventbus-server.exe"
$OBJECTSTORE = "$BASE_DIR/vienna-objectstore-server.exe"
$SERVER = "$BASE_DIR/vienna-apiserver.exe"

$global:processes = @()

function Log($msg) {
    $LogBox.AppendText("$msg`n")
    $LogBox.ScrollToEnd()
}

function Start-Exe($path, $args="") {
    Log "Starting $path $args"
    if ([string]::IsNullOrWhiteSpace($args)) {
        $p = Start-Process -FilePath $path -PassThru
    } else {
        $p = Start-Process -FilePath $path -ArgumentList $args -PassThru
    }
    $global:processes += $p
}

# --- Start Button ---
$StartBtn.Add_Click({
    Log "=== Starting Vienna ==="

    Start-Exe $EVENTBUS
    Start-Sleep -Seconds 2

    Start-Exe $OBJECTSTORE "-dataDir ./data/data -port 5396"
    Start-Sleep -Seconds 2

    Get-ChildItem $VIENNA_DIR -Filter *.exe | ForEach-Object {
        if ($_.FullName -ne $EVENTBUS -and $_.FullName -ne $OBJECTSTORE) {
            Start-Exe $_.FullName
        }
    }

    Start-Exe $SERVER "--db ./earth.db --staticData ./data/data"

    Log "=== All servers started ==="
})

# --- Stop Button ---
$StopBtn.Add_Click({
    Log "=== Stopping Vienna ==="

    foreach ($p in $global:processes) {
        try {
            Stop-Process -Id $p.Id -Force
        } catch {}
    }

    $global:processes = @()
    Log "=== All servers stopped ==="
})

# --- Run UI ---
$window.ShowDialog()
