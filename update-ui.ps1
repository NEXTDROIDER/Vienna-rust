Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$script:RootPath = Split-Path -Parent $MyInvocation.MyCommand.Path
$script:SettingsPath = Join-Path $script:RootPath 'update-ui.settings.json'
$script:LatestUpdateInfo = $null
$script:ActiveWebClient = $null

function Get-DefaultSettings {
    [ordered]@{
        AppName        = 'Vienna Updater'
        CurrentVersion = '0.0.0'
        VersionInfoUrl = ''
        DownloadUrl    = ''
        DownloadFolder = Join-Path $script:RootPath 'downloads'
        FileName       = ''
        OpenFolder     = $true
    }
}

function ConvertTo-SettingsObject {
    param(
        [Parameter(Mandatory)]
        [object]$InputObject
    )

    $defaults = Get-DefaultSettings
    $result = [ordered]@{}

    foreach ($key in $defaults.Keys) {
        $value = $null
        if ($null -ne $InputObject.PSObject.Properties[$key]) {
            $value = $InputObject.$key
        }

        if ($null -eq $value -or ($value -is [string] -and [string]::IsNullOrWhiteSpace($value) -and $key -ne 'DownloadFolder')) {
            $result[$key] = $defaults[$key]
        }
        else {
            $result[$key] = $value
        }
    }

    [pscustomobject]$result
}

function Load-Settings {
    $defaults = [pscustomobject](Get-DefaultSettings)

    if (-not (Test-Path -LiteralPath $script:SettingsPath)) {
        return $defaults
    }

    try {
        $loaded = Get-Content -LiteralPath $script:SettingsPath -Raw | ConvertFrom-Json
        return ConvertTo-SettingsObject -InputObject $loaded
    }
    catch {
        [System.Windows.Forms.MessageBox]::Show(
            "Could not read settings file.`r`n`r`n$($_.Exception.Message)",
            'Settings Error',
            [System.Windows.Forms.MessageBoxButtons]::OK,
            [System.Windows.Forms.MessageBoxIcon]::Warning
        ) | Out-Null

        return $defaults
    }
}

function Save-Settings {
    param(
        [Parameter(Mandatory)]
        [pscustomobject]$Settings
    )

    $Settings | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $script:SettingsPath -Encoding UTF8
}

function Compare-VersionValue {
    param(
        [string]$CurrentVersion,
        [string]$RemoteVersion
    )

    try {
        return ([version]$RemoteVersion).CompareTo([version]$CurrentVersion)
    }
    catch {
        if ($RemoteVersion -eq $CurrentVersion) {
            return 0
        }

        return [string]::Compare($RemoteVersion, $CurrentVersion, $true)
    }
}

function Get-UpdateInfo {
    param(
        [Parameter(Mandatory)]
        [pscustomobject]$Settings
    )

    if ([string]::IsNullOrWhiteSpace($Settings.VersionInfoUrl)) {
        return [pscustomobject]@{
            Version     = $null
            DownloadUrl = $Settings.DownloadUrl
            Notes       = 'Version endpoint is empty. Direct download URL will be used.'
            FileName    = $Settings.FileName
        }
    }

    $response = Invoke-WebRequest -Uri $Settings.VersionInfoUrl -UseBasicParsing
    $raw = [string]$response.Content
    $contentType = [string]$response.Headers['Content-Type']

    if ([string]::IsNullOrWhiteSpace($raw)) {
        throw 'Version endpoint returned an empty response.'
    }

    $trimmed = $raw.Trim()

    if ($contentType -like '*json*' -or $trimmed.StartsWith('{')) {
        $json = $trimmed | ConvertFrom-Json

        $downloadUrl = $Settings.DownloadUrl
        if ($null -ne $json.PSObject.Properties['downloadUrl'] -and -not [string]::IsNullOrWhiteSpace([string]$json.downloadUrl)) {
            $downloadUrl = [string]$json.downloadUrl
        }

        $fileName = $Settings.FileName
        if ($null -ne $json.PSObject.Properties['fileName'] -and -not [string]::IsNullOrWhiteSpace([string]$json.fileName)) {
            $fileName = [string]$json.fileName
        }

        $notes = ''
        if ($null -ne $json.PSObject.Properties['notes']) {
            $notes = [string]$json.notes
        }

        $version = $null
        if ($null -ne $json.PSObject.Properties['version'] -and -not [string]::IsNullOrWhiteSpace([string]$json.version)) {
            $version = [string]$json.version
        }

        return [pscustomobject]@{
            Version     = $version
            DownloadUrl = $downloadUrl
            Notes       = $notes
            FileName    = $fileName
        }
    }

    return [pscustomobject]@{
        Version     = $trimmed
        DownloadUrl = $Settings.DownloadUrl
        Notes       = 'Server returned plain text. It is treated as the latest version.'
        FileName    = $Settings.FileName
    }
}

function Get-DownloadFileName {
    param(
        [string]$DownloadUrl,
        [string]$PreferredName
    )

    if (-not [string]::IsNullOrWhiteSpace($PreferredName)) {
        return $PreferredName.Trim()
    }

    try {
        $uri = [System.Uri]$DownloadUrl
        $name = [System.IO.Path]::GetFileName($uri.LocalPath)
        if (-not [string]::IsNullOrWhiteSpace($name)) {
            return $name
        }
    }
    catch {
    }

    return 'update.zip'
}

function Set-Status {
    param(
        [Parameter(Mandatory)]
        [System.Windows.Forms.Label]$Label,
        [Parameter(Mandatory)]
        [string]$Text
    )

    $Label.Text = $Text
}

function Add-LogLine {
    param(
        [Parameter(Mandatory)]
        [System.Windows.Forms.TextBox]$LogBox,
        [Parameter(Mandatory)]
        [string]$Message
    )

    $stamp = (Get-Date).ToString('HH:mm:ss')
    $line = "[{0}] {1}" -f $stamp, $Message

    if ([string]::IsNullOrWhiteSpace($LogBox.Text)) {
        $LogBox.Text = $line
    }
    else {
        $LogBox.AppendText([Environment]::NewLine + $line)
    }
}

function Set-BusyState {
    param(
        [Parameter(Mandatory)]
        [bool]$Busy
    )

    $btnSave.Enabled = -not $Busy
    $btnCheck.Enabled = -not $Busy
    $btnBrowse.Enabled = -not $Busy
    $btnDownload.Enabled = -not $Busy
}

$settings = Load-Settings

$form = New-Object System.Windows.Forms.Form
$form.Text = 'Updater Settings'
$form.StartPosition = 'CenterScreen'
$form.Size = New-Object System.Drawing.Size(760, 560)
$form.MinimumSize = New-Object System.Drawing.Size(760, 560)
$form.MaximizeBox = $false
$form.Font = New-Object System.Drawing.Font('Segoe UI', 10)

$labelWidth = 150
$textX = 180
$textWidth = 540

$labels = @(
    @{ Text = 'App name';        Y = 20  },
    @{ Text = 'Current version'; Y = 60  },
    @{ Text = 'Version URL';     Y = 100 },
    @{ Text = 'Download URL';    Y = 140 },
    @{ Text = 'Download folder'; Y = 180 },
    @{ Text = 'File name';       Y = 220 }
)

foreach ($item in $labels) {
    $label = New-Object System.Windows.Forms.Label
    $label.Text = $item.Text
    $label.Location = New-Object System.Drawing.Point -ArgumentList 20, ([int]($item.Y + 4))
    $label.Size = New-Object System.Drawing.Size($labelWidth, 24)
    $form.Controls.Add($label)
}

$txtAppName = New-Object System.Windows.Forms.TextBox
$txtAppName.Location = New-Object System.Drawing.Point($textX, 20)
$txtAppName.Size = New-Object System.Drawing.Size($textWidth, 24)
$txtAppName.Text = [string]$settings.AppName
$form.Controls.Add($txtAppName)

$txtCurrentVersion = New-Object System.Windows.Forms.TextBox
$txtCurrentVersion.Location = New-Object System.Drawing.Point($textX, 60)
$txtCurrentVersion.Size = New-Object System.Drawing.Size(220, 24)
$txtCurrentVersion.Text = [string]$settings.CurrentVersion
$form.Controls.Add($txtCurrentVersion)

$txtVersionInfoUrl = New-Object System.Windows.Forms.TextBox
$txtVersionInfoUrl.Location = New-Object System.Drawing.Point($textX, 100)
$txtVersionInfoUrl.Size = New-Object System.Drawing.Size($textWidth, 24)
$txtVersionInfoUrl.Text = [string]$settings.VersionInfoUrl
$form.Controls.Add($txtVersionInfoUrl)

$txtDownloadUrl = New-Object System.Windows.Forms.TextBox
$txtDownloadUrl.Location = New-Object System.Drawing.Point($textX, 140)
$txtDownloadUrl.Size = New-Object System.Drawing.Size($textWidth, 24)
$txtDownloadUrl.Text = [string]$settings.DownloadUrl
$form.Controls.Add($txtDownloadUrl)

$txtDownloadFolder = New-Object System.Windows.Forms.TextBox
$txtDownloadFolder.Location = New-Object System.Drawing.Point($textX, 180)
$txtDownloadFolder.Size = New-Object System.Drawing.Size(430, 24)
$txtDownloadFolder.Text = [string]$settings.DownloadFolder
$form.Controls.Add($txtDownloadFolder)

$btnBrowse = New-Object System.Windows.Forms.Button
$btnBrowse.Text = 'Browse'
$btnBrowse.Location = New-Object System.Drawing.Point(620, 178)
$btnBrowse.Size = New-Object System.Drawing.Size(100, 28)
$form.Controls.Add($btnBrowse)

$txtFileName = New-Object System.Windows.Forms.TextBox
$txtFileName.Location = New-Object System.Drawing.Point($textX, 220)
$txtFileName.Size = New-Object System.Drawing.Size(220, 24)
$txtFileName.Text = [string]$settings.FileName
$form.Controls.Add($txtFileName)

$chkOpenFolder = New-Object System.Windows.Forms.CheckBox
$chkOpenFolder.Text = 'Open folder after download'
$chkOpenFolder.Location = New-Object System.Drawing.Point($textX, 255)
$chkOpenFolder.Size = New-Object System.Drawing.Size(260, 24)
$chkOpenFolder.Checked = [bool]$settings.OpenFolder
$form.Controls.Add($chkOpenFolder)

$btnSave = New-Object System.Windows.Forms.Button
$btnSave.Text = 'Save settings'
$btnSave.Location = New-Object System.Drawing.Point(20, 300)
$btnSave.Size = New-Object System.Drawing.Size(180, 36)
$form.Controls.Add($btnSave)

$btnCheck = New-Object System.Windows.Forms.Button
$btnCheck.Text = 'Check update'
$btnCheck.Location = New-Object System.Drawing.Point(220, 300)
$btnCheck.Size = New-Object System.Drawing.Size(180, 36)
$form.Controls.Add($btnCheck)

$btnDownload = New-Object System.Windows.Forms.Button
$btnDownload.Text = 'Download update'
$btnDownload.Location = New-Object System.Drawing.Point(420, 300)
$btnDownload.Size = New-Object System.Drawing.Size(180, 36)
$form.Controls.Add($btnDownload)

$progressBar = New-Object System.Windows.Forms.ProgressBar
$progressBar.Location = New-Object System.Drawing.Point(20, 350)
$progressBar.Size = New-Object System.Drawing.Size(700, 24)
$progressBar.Minimum = 0
$progressBar.Maximum = 100
$form.Controls.Add($progressBar)

$lblStatus = New-Object System.Windows.Forms.Label
$lblStatus.Location = New-Object System.Drawing.Point(20, 385)
$lblStatus.Size = New-Object System.Drawing.Size(700, 24)
$lblStatus.Text = 'Ready.'
$form.Controls.Add($lblStatus)

$grpNotes = New-Object System.Windows.Forms.GroupBox
$grpNotes.Text = 'Update notes'
$grpNotes.Location = New-Object System.Drawing.Point(20, 415)
$grpNotes.Size = New-Object System.Drawing.Size(340, 100)
$form.Controls.Add($grpNotes)

$txtNotes = New-Object System.Windows.Forms.TextBox
$txtNotes.Location = New-Object System.Drawing.Point(10, 25)
$txtNotes.Size = New-Object System.Drawing.Size(320, 65)
$txtNotes.Multiline = $true
$txtNotes.ReadOnly = $true
$txtNotes.ScrollBars = 'Vertical'
$grpNotes.Controls.Add($txtNotes)

$grpLog = New-Object System.Windows.Forms.GroupBox
$grpLog.Text = 'Log'
$grpLog.Location = New-Object System.Drawing.Point(380, 415)
$grpLog.Size = New-Object System.Drawing.Size(340, 100)
$form.Controls.Add($grpLog)

$txtLog = New-Object System.Windows.Forms.TextBox
$txtLog.Location = New-Object System.Drawing.Point(10, 25)
$txtLog.Size = New-Object System.Drawing.Size(320, 65)
$txtLog.Multiline = $true
$txtLog.ReadOnly = $true
$txtLog.ScrollBars = 'Vertical'
$grpLog.Controls.Add($txtLog)

$folderDialog = New-Object System.Windows.Forms.FolderBrowserDialog

function Get-UiSettings {
    [pscustomobject]@{
        AppName        = $txtAppName.Text.Trim()
        CurrentVersion = $txtCurrentVersion.Text.Trim()
        VersionInfoUrl = $txtVersionInfoUrl.Text.Trim()
        DownloadUrl    = $txtDownloadUrl.Text.Trim()
        DownloadFolder = $txtDownloadFolder.Text.Trim()
        FileName       = $txtFileName.Text.Trim()
        OpenFolder     = $chkOpenFolder.Checked
    }
}

$btnBrowse.Add_Click({
    $folderDialog.SelectedPath = $txtDownloadFolder.Text
    if ($folderDialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
        $txtDownloadFolder.Text = $folderDialog.SelectedPath
    }
})

$btnSave.Add_Click({
    try {
        $currentSettings = Get-UiSettings
        Save-Settings -Settings $currentSettings
        Set-Status -Label $lblStatus -Text 'Settings saved.'
        Add-LogLine -LogBox $txtLog -Message 'Settings saved to update-ui.settings.json.'
    }
    catch {
        Set-Status -Label $lblStatus -Text 'Failed to save settings.'
        Add-LogLine -LogBox $txtLog -Message ("Save error: {0}" -f $_.Exception.Message)
    }
})

$btnCheck.Add_Click({
    try {
        $currentSettings = Get-UiSettings
        Save-Settings -Settings $currentSettings
        Set-BusyState -Busy $true
        $progressBar.Value = 0
        $txtNotes.Text = ''

        Set-Status -Label $lblStatus -Text 'Checking for update...'
        Add-LogLine -LogBox $txtLog -Message 'Started update check.'

        $updateInfo = Get-UpdateInfo -Settings $currentSettings
        $script:LatestUpdateInfo = $updateInfo

        if (-not [string]::IsNullOrWhiteSpace([string]$updateInfo.Notes)) {
            $txtNotes.Text = [string]$updateInfo.Notes
        }

        if (-not [string]::IsNullOrWhiteSpace([string]$updateInfo.Version)) {
            $comparison = Compare-VersionValue -CurrentVersion $currentSettings.CurrentVersion -RemoteVersion ([string]$updateInfo.Version)

            if ($comparison -gt 0) {
                Set-Status -Label $lblStatus -Text ("New version found: {0}" -f $updateInfo.Version)
                Add-LogLine -LogBox $txtLog -Message ("New version found: {0}" -f $updateInfo.Version)
            }
            elseif ($comparison -eq 0) {
                Set-Status -Label $lblStatus -Text 'You already have the latest version.'
                Add-LogLine -LogBox $txtLog -Message 'Current version is already up to date.'
            }
            else {
                Set-Status -Label $lblStatus -Text ("Remote version {0} is not newer than current version." -f $updateInfo.Version)
                Add-LogLine -LogBox $txtLog -Message 'Remote version is older or equal after string comparison.'
            }
        }
        else {
            Set-Status -Label $lblStatus -Text 'Update endpoint checked. Download is available.'
            Add-LogLine -LogBox $txtLog -Message 'No remote version was returned, but a download URL is available.'
        }

        if ([string]::IsNullOrWhiteSpace([string]$updateInfo.DownloadUrl)) {
            Add-LogLine -LogBox $txtLog -Message 'Warning: no download URL is available yet.'
        }
    }
    catch {
        Set-Status -Label $lblStatus -Text 'Update check failed.'
        Add-LogLine -LogBox $txtLog -Message ("Check error: {0}" -f $_.Exception.Message)
    }
    finally {
        Set-BusyState -Busy $false
    }
})

$btnDownload.Add_Click({
    try {
        $currentSettings = Get-UiSettings
        Save-Settings -Settings $currentSettings

        $downloadUrl = $currentSettings.DownloadUrl
        $preferredFileName = $currentSettings.FileName

        if ($null -ne $script:LatestUpdateInfo) {
            if (-not [string]::IsNullOrWhiteSpace([string]$script:LatestUpdateInfo.DownloadUrl)) {
                $downloadUrl = [string]$script:LatestUpdateInfo.DownloadUrl
            }

            if (-not [string]::IsNullOrWhiteSpace([string]$script:LatestUpdateInfo.FileName)) {
                $preferredFileName = [string]$script:LatestUpdateInfo.FileName
            }
        }

        if ([string]::IsNullOrWhiteSpace($downloadUrl)) {
            throw 'Download URL is empty.'
        }

        if ([string]::IsNullOrWhiteSpace($currentSettings.DownloadFolder)) {
            throw 'Download folder is empty.'
        }

        if (-not (Test-Path -LiteralPath $currentSettings.DownloadFolder)) {
            New-Item -Path $currentSettings.DownloadFolder -ItemType Directory -Force | Out-Null
        }

        $fileName = Get-DownloadFileName -DownloadUrl $downloadUrl -PreferredName $preferredFileName
        $targetPath = Join-Path $currentSettings.DownloadFolder $fileName

        if ($script:ActiveWebClient) {
            throw 'Another download is already running.'
        }

        $progressBar.Value = 0
        Set-BusyState -Busy $true
        Set-Status -Label $lblStatus -Text 'Downloading update...'
        Add-LogLine -LogBox $txtLog -Message ("Downloading from {0}" -f $downloadUrl)

        $script:ActiveWebClient = New-Object System.Net.WebClient

        $script:ActiveWebClient.add_DownloadProgressChanged({
            param($sender, $eventArgs)

            $value = [Math]::Max(0, [Math]::Min(100, [int]$eventArgs.ProgressPercentage))
            $progressBar.Value = $value
            Set-Status -Label $lblStatus -Text ("Downloading: {0}%%" -f $value)
        })

        $script:ActiveWebClient.add_DownloadFileCompleted({
            param($sender, $eventArgs)

            try {
                if ($eventArgs.Cancelled) {
                    $progressBar.Value = 0
                    Set-Status -Label $lblStatus -Text 'Download cancelled.'
                    Add-LogLine -LogBox $txtLog -Message 'Download cancelled.'
                    return
                }

                if ($null -ne $eventArgs.Error) {
                    $progressBar.Value = 0
                    Set-Status -Label $lblStatus -Text 'Download failed.'
                    Add-LogLine -LogBox $txtLog -Message ("Download error: {0}" -f $eventArgs.Error.Message)
                    return
                }

                $progressBar.Value = 100
                Set-Status -Label $lblStatus -Text ("Downloaded: {0}" -f $targetPath)
                Add-LogLine -LogBox $txtLog -Message ("Saved file: {0}" -f $targetPath)

                if ($chkOpenFolder.Checked) {
                    Start-Process -FilePath 'explorer.exe' -ArgumentList ('/select,"{0}"' -f $targetPath) | Out-Null
                }
            }
            finally {
                if ($script:ActiveWebClient) {
                    $script:ActiveWebClient.Dispose()
                    $script:ActiveWebClient = $null
                }
                Set-BusyState -Busy $false
            }
        })

        $script:ActiveWebClient.DownloadFileAsync([System.Uri]$downloadUrl, $targetPath)
    }
    catch {
        if ($script:ActiveWebClient) {
            $script:ActiveWebClient.Dispose()
            $script:ActiveWebClient = $null
        }
        Set-BusyState -Busy $false
        Set-Status -Label $lblStatus -Text 'Could not start download.'
        Add-LogLine -LogBox $txtLog -Message ("Start download error: {0}" -f $_.Exception.Message)
    }
})

$form.Add_FormClosing({
    if ($script:ActiveWebClient) {
        try {
            $script:ActiveWebClient.CancelAsync()
            $script:ActiveWebClient.Dispose()
        }
        catch {
        }
        finally {
            $script:ActiveWebClient = $null
        }
    }
})

[void]$form.ShowDialog()
