param([switch]$StandardUser)

$ErrorActionPreference = 'Stop'

function Assert([bool]$Condition, [string]$Message) {
    if (!$Condition) { throw $Message }
}

function Invoke-Installer([string]$Path) {
    $process = Start-Process -FilePath $Path -ArgumentList '/S' -Wait -PassThru -WindowStyle Hidden
    Assert ($process.ExitCode -eq 0) "Installer or uninstaller exited with code $($process.ExitCode)"
}

function Invoke-StandardUserTest {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $admins = [Security.Principal.SecurityIdentifier]::new('S-1-5-32-544')
    Assert (!$identity.Groups.Contains($admins)) 'The test account belongs to Administrators'
    Assert ([Environment]::Is64BitProcess) 'Expected an x64 test process'
    Write-Output 'Running with a standard local user'

    $installer = Join-Path $PSScriptRoot 'setup.exe'
    $name = 'kvojps-monitor'
    $bundleId = 'dev.kvojps.monitor'
    $activator = '{57165877-A42D-4FF3-A2D9-B8B94EF32F09}'
    $runKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
    $uninstallKey = "Software\Microsoft\Windows\CurrentVersion\Uninstall\$name"
    $protocolKey = "HKCU:\Software\Classes\$name"
    $comKey = "HKCU:\Software\Classes\CLSID\$activator"
    $identityKey = "HKCU:\Software\Classes\AppUserModelId\$bundleId"
    $webviewClient = '{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
    $webviewPresent = @(
        "HKLM:\Software\WOW6432Node\Microsoft\EdgeUpdate\Clients\$webviewClient"
        "HKCU:\Software\Microsoft\EdgeUpdate\Clients\$webviewClient"
    ) | Where-Object {
        $version = Get-ItemPropertyValue -LiteralPath $_ -Name 'pv' -ErrorAction SilentlyContinue
        $version -and $version -ne '0.0.0.0'
    }
    Write-Output "WebView2 present before setup: $([bool]$webviewPresent)"

    Invoke-Installer $installer
    $entry = Get-ItemProperty -LiteralPath "HKCU:\$uninstallKey"
    $directory = $entry.InstallLocation.Trim('"')
    # Start-Process -Credential can inherit the runner's environment.
    $localAppData = [Environment]::GetFolderPath('LocalApplicationData')
    $roamingAppData = [Environment]::GetFolderPath('ApplicationData')
    $userProfile = [Environment]::GetFolderPath('UserProfile')
    Assert ($localAppData -and $directory.StartsWith("$localAppData\", [StringComparison]::OrdinalIgnoreCase)) 'Install directory is outside LocalAppData'
    $app = Join-Path $directory "$name.exe"
    $uninstaller = Join-Path $directory 'uninstall.exe'
    foreach ($binary in @($app, $uninstaller)) {
        Assert ((Get-AuthenticodeSignature -LiteralPath $binary).Status -eq 'NotSigned') 'Expected an unsigned executable'
    }
    Assert (!(Test-Path -LiteralPath "HKLM:\$uninstallKey")) 'Found an unexpected machine-wide install'
    Assert ($entry.UninstallString -and $entry.UninstallString.Trim('"') -eq $uninstaller) 'Wrong uninstall command'
    Assert (!(Get-ItemPropertyValue -LiteralPath $runKey -Name $name -ErrorAction SilentlyContinue)) 'Installation enabled automatic startup'

    $startMenu = [Environment]::GetFolderPath('Programs')
    $link = Join-Path $startMenu "$name.lnk"
    $desktopLink = Join-Path ([Environment]::GetFolderPath('Desktop')) "$name.lnk"
    Assert (Test-Path -LiteralPath $link) 'Start menu shortcut was not created'
    $shell = New-Object -ComObject WScript.Shell
    Assert ($shell.CreateShortcut($link).TargetPath -eq $app) 'Wrong Start menu shortcut target'
    $appProcess = Start-Process -FilePath $app -PassThru -WindowStyle Hidden
    try {
        $deadline = [DateTime]::UtcNow.AddSeconds(30)
        do {
            Assert (!$appProcess.HasExited) 'Installed application exited before registering notification activation'
            $commandKey = Get-Item -LiteralPath "$protocolKey\shell\open\command" -ErrorAction SilentlyContinue
            $command = if ($commandKey) { $commandKey.GetValue('') }
            if ($command -eq "`"$app`" --notification-activate `"%1`"") { break }
            Start-Sleep -Milliseconds 250
        } while ([DateTime]::UtcNow -lt $deadline)
        Assert ($command -eq "`"$app`" --notification-activate `"%1`"") 'Installed application did not register the notification protocol'
        Assert ((Get-Item -LiteralPath "$comKey\LocalServer32").GetValue('') -eq "`"$app`" --toast-activate") 'Wrong notification COM command'
        Assert ((Get-ItemPropertyValue -LiteralPath $identityKey -Name 'CustomActivator') -eq $activator) 'Wrong notification identity'
    } finally {
        if (!$appProcess.HasExited) { Stop-Process -Id $appProcess.Id; $appProcess.WaitForExit() }
    }

    # The installed application wrote first-run preferences in the isolated profile.
    $preferences = Join-Path $roamingAppData "$name\preferencias.json"
    $preferencesHash = (Get-FileHash -LiteralPath $preferences).Hash
    $codex = New-Item -ItemType Directory -Path (Join-Path $userProfile '.codex') -Force
    $hooks = Join-Path $codex.FullName 'hooks.json'
    Set-Content -LiteralPath $hooks '{"hooks":{}}'
    if (!(Test-Path -LiteralPath $runKey)) { New-Item -Path $runKey | Out-Null }
    $startupCommand = "`"$app`" --autostart"
    Set-ItemProperty -LiteralPath $runKey -Name $name -Value $startupCommand

    Invoke-Installer $installer
    Assert ((Get-FileHash -LiteralPath $preferences).Hash -eq $preferencesHash) 'Reinstall changed preferences'
    Assert ((Get-ItemPropertyValue -LiteralPath $runKey -Name $name) -eq $startupCommand) 'Reinstall removed automatic startup preference'
    Invoke-Installer $uninstaller
    foreach ($path in @($directory, $link, $desktopLink, "HKCU:\$uninstallKey", $protocolKey, $comKey, $identityKey)) {
        Assert (!(Test-Path -LiteralPath $path)) 'Uninstall left an installation file, shortcut, or registration'
    }
    Assert (!(Get-ItemPropertyValue -LiteralPath $runKey -Name $name -ErrorAction SilentlyContinue)) 'Uninstall left automatic startup registered'
    Assert ((Get-FileHash -LiteralPath $preferences).Hash -eq $preferencesHash) 'Uninstall changed preferences'
    Assert ((Get-Content -LiteralPath $hooks -Raw).Trim() -eq '{"hooks":{}}') 'Uninstall modified third-party agent settings'

    Invoke-Installer $installer
    New-Item -Path $protocolKey | Out-Null
    Set-ItemProperty -LiteralPath $protocolKey -Name 'MonitorOwner' -Value 'synthetic.other'
    Set-ItemProperty -LiteralPath $runKey -Name $name -Value '"C:\Example\portable.exe" --autostart'
    Invoke-Installer $uninstaller
    Assert ((Get-ItemPropertyValue -LiteralPath $protocolKey -Name 'MonitorOwner') -eq 'synthetic.other') 'Uninstall removed a foreign protocol'
    Assert ((Get-ItemPropertyValue -LiteralPath $runKey -Name $name) -eq '"C:\Example\portable.exe" --autostart') 'Uninstall removed a portable startup command'
    Write-Output 'Unsigned install, reinstall, launch, uninstall and ownership checks passed'
}

if ($StandardUser) {
    $log = Join-Path $PSScriptRoot 'result.log'
    try {
        Invoke-StandardUserTest *> $log
        exit 0
    } catch {
        Add-Content -LiteralPath $log "Line $($_.InvocationInfo.ScriptLineNumber): $($_.Exception.Message)"
        exit 1
    }
}

Assert ($env:GITHUB_ACTIONS -eq 'true') 'Run only on the disposable GitHub Actions VM'
$installers = @(Get-ChildItem -Path 'src-tauri/target/release/bundle/nsis/*-setup.exe' -File)
Assert ($installers.Count -eq 1) 'Expected one Windows installer'
Assert ((Get-AuthenticodeSignature -LiteralPath $installers[0].FullName).Status -eq 'NotSigned') 'Expected an unsigned installer'
$accountName = 'monitor-' + [Guid]::NewGuid().ToString('N').Substring(0, 8)
$password = ConvertTo-SecureString ("aZ9!" + [Guid]::NewGuid().ToString('N')) -AsPlainText -Force
$directory = Join-Path $env:ProgramData $accountName
$account = $null
try {
    $account = New-LocalUser -Name $accountName -Password $password
    $users = [Security.Principal.SecurityIdentifier]::new('S-1-5-32-545')
    if (!(Get-LocalGroupMember -SID $users | Where-Object SID -eq $account.SID)) {
        Add-LocalGroupMember -SID $users -Member $account
    }
    New-Item -ItemType Directory -Path $directory | Out-Null
    icacls $directory /grant "*$($account.SID.Value):(OI)(CI)M" | Out-Null
    Assert ($LASTEXITCODE -eq 0) 'Could not grant access to the test files'
    Copy-Item -LiteralPath $installers[0].FullName -Destination (Join-Path $directory 'setup.exe')
    $script = Join-Path $directory 'test.ps1'
    Copy-Item -LiteralPath $PSCommandPath -Destination $script
    $credential = [PSCredential]::new(".\$accountName", $password)
    $powershell = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
    $process = Start-Process -FilePath $powershell -Credential $credential -LoadUserProfile -WorkingDirectory $directory -ArgumentList "-NoProfile -NonInteractive -ExecutionPolicy RemoteSigned -File `"$script`" -StandardUser" -PassThru
    if (!$process.WaitForExit(240000)) {
        Stop-Process -Id $process.Id
        throw 'The standard-user installer test timed out'
    }
    $log = Join-Path $directory 'result.log'
    if (Test-Path -LiteralPath $log) { Get-Content -LiteralPath $log }
    Assert ($process.ExitCode -eq 0) 'The standard-user installer test failed'
} finally {
    if ($account) {
        try {
            Get-CimInstance Win32_UserProfile -Filter "SID='$($account.SID.Value)'" | Remove-CimInstance
        } finally {
            Remove-LocalUser -SID $account.SID
        }
    }
    if (Test-Path -LiteralPath $directory) {
        $resolved = Get-Item -LiteralPath $directory
        Assert ($resolved.FullName -eq [IO.Path]::GetFullPath($directory) -and
            !$resolved.Attributes.HasFlag([IO.FileAttributes]::ReparsePoint)) 'Unexpected test directory'
        Remove-Item -LiteralPath $resolved.FullName -Recurse -Force
    }
}
