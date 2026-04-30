
Add-Type -AssemblyName System.Net.HttpListener

$listener = New-Object System.Net.HttpListener
$listener.Prefixes.Add("http://localhost:5050/")
$listener.Start()

Write-Host "Vienna PRO Panel API running on http://localhost:5050"

$script:Users = Get-Content ".\users.json" | ConvertFrom-Json
$script:Token = "admin-token"

$script:Services = @{}
$script:Logs = @{}

function Write-Log($service,$msg){
    if (-not $script:Logs[$service]) { $script:Logs[$service] = @() }
    $script:Logs[$service] += "[$((Get-Date).ToString('HH:mm:ss'))] $msg"
}

function Start-ServiceSim($name){
    $p = Start-Process "cmd.exe" -ArgumentList "/c ping localhost -t" -PassThru -WindowStyle Hidden
    $script:Services[$name] = @{ Name=$name; PID=$p.Id; Status="running" }
    Write-Log $name "started"
}

function Stop-All{
    foreach($s in $script:Services.Values){
        Stop-Process -Id $s.PID -Force -ErrorAction SilentlyContinue
    }
    $script:Services.Clear()
}

while ($listener.IsListening) {

    $ctx = $listener.GetContext()
    $req = $ctx.Request
    $res = $ctx.Response

    $body = ""

    try {

        $path = $req.Url.AbsolutePath

        if ($path -eq "/api/login") {
            $reader = New-Object IO.StreamReader $req.InputStream
            $data = ($reader.ReadToEnd() | ConvertFrom-Json)

            if ($script:Users.$($data.user) -eq $data.pass) {
                $body = @{ token=$script:Token } | ConvertTo-Json
            } else {
                $body = "invalid"
            }
        }

        elseif ($req.Headers["Authorization"] -ne "Bearer admin-token") {
            $body = "unauthorized"
        }

        elseif ($path -eq "/api/start") {
            Start-ServiceSim "API"
            Start-ServiceSim "EventBus"
            Start-ServiceSim "ObjectStore"
            $body = "started"
        }

        elseif ($path -eq "/api/stop") {
            Stop-All
            $body = "stopped"
        }

        elseif ($path -eq "/api/services") {
            $body = $script:Services.Values | ConvertTo-Json -Depth 3
        }

        elseif ($path -like "/api/logs*") {
            $svc = $req.QueryString["service"]
            $body = $script:Logs[$svc] | ConvertTo-Json
        }

        else {
            $body = "Vienna PRO API"
        }

    } catch {
        $body = $_.Exception.Message
    }

    $bytes = [Text.Encoding]::UTF8.GetBytes($body)
    $res.OutputStream.Write($bytes,0,$bytes.Length)
    $res.Close()
}
