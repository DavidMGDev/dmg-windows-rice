<#
    t3here - open a directory as a T3 Code project and bring the app forward.

    The project is created only when one does not already exist for that path,
    so this is safe to run repeatedly on the same folder.

        t3here [path]        defaults to the current directory
        t3here -Install      add the PATH command + "Continue with T3Code" menu entry
        t3here -Uninstall    remove both
#>
[CmdletBinding()]
param(
    [string]$Path,
    [switch]$Install,
    [switch]$Uninstall
)

$MenuLabel = 'Continue with T3Code'
# Static verbs are drawn after the packaged and shellex handlers, and sort
# alphabetically among themselves by key name, so the key name is the only
# lever on placement. This one sorts ahead of git_gui and git_shell, which puts
# the entry directly under Open in Terminal's group instead of below them.
$KeyName    = 'ContinueWithT3Code'
$LegacyKeys = @('T3Code')
$ShellKeys = @(
    'HKCU:\Software\Classes\Directory\shell'             # right-click a folder
    'HKCU:\Software\Classes\Directory\Background\shell'  # right-click inside a folder
)

# T3 Code rewrites its t3code:// handler on every auto-update, so reading the exe
# path back out of it survives version bumps that a hardcoded path would not.
function Get-T3CodeExe {
    $command = (Get-ItemProperty 'HKCU:\Software\Classes\t3code\shell\open\command' -ErrorAction SilentlyContinue).'(default)'
    if ($command -match '^"([^"]+)"') { return $Matches[1] }

    $fallback = Join-Path $env:LOCALAPPDATA 'Programs\t3code\T3 Code (Alpha).exe'
    if (Test-Path -LiteralPath $fallback) { return $fallback }

    throw 'T3 Code is not installed: no t3code:// handler and no exe under LOCALAPPDATA\Programs\t3code.'
}

# Under wscript there is no console to write to, so failures need a dialog.
function Show-Failure($message) {
    if ([Environment]::UserInteractive -and $Host.Name -eq 'ConsoleHost') {
        Write-Error $message
    } else {
        (New-Object -ComObject WScript.Shell).Popup($message, 0, 'T3 Code', 16) | Out-Null
    }
}

function Remove-ShellKeys($names) {
    foreach ($base in $ShellKeys) {
        foreach ($name in $names) {
            Remove-Item -Path (Join-Path $base $name) -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
}

function Install-T3Here {
    $exe = Get-T3CodeExe
    $vbs = Join-Path $PSScriptRoot 't3here.vbs'

    Remove-ShellKeys $LegacyKeys

    foreach ($base in $ShellKeys) {
        $key = Join-Path $base $KeyName
        New-Item -Path (Join-Path $key 'command') -Force | Out-Null
        Set-ItemProperty -Path $key -Name '(default)' -Value $MenuLabel
        Set-ItemProperty -Path $key -Name 'Icon' -Value "`"$exe`",0"
        # %V is the clicked folder for both Directory and Directory\Background.
        Set-ItemProperty -Path (Join-Path $key 'command') -Name '(default)' `
            -Value "wscript.exe `"$vbs`" `"%V`""
    }
    Write-Host "Added `"$MenuLabel`" to the folder and folder-background context menus."

    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    if (($userPath -split ';') -notcontains $PSScriptRoot) {
        [Environment]::SetEnvironmentVariable('Path', "$userPath;$PSScriptRoot", 'User')
        Write-Host "Added $PSScriptRoot to your user PATH. Open a new shell to pick up 't3here'."
    }

    if (-not (Get-Command t3 -ErrorAction SilentlyContinue)) {
        Write-Host 'Installing the t3 CLI globally...'
        npm install -g t3@latest
    }
}

function Uninstall-T3Here {
    Remove-ShellKeys (@($KeyName) + $LegacyKeys)
    $userPath = ([Environment]::GetEnvironmentVariable('Path', 'User') -split ';') | Where-Object { $_ -and $_ -ne $PSScriptRoot }
    [Environment]::SetEnvironmentVariable('Path', ($userPath -join ';'), 'User')
    Write-Host 'Removed the context-menu entry and the PATH entry. The t3 CLI was left installed.'
}

if ($Install)   { Install-T3Here;   return }
if ($Uninstall) { Uninstall-T3Here; return }


if (-not $Path) { $Path = $PWD.Path }

try {
    $exe    = Get-T3CodeExe
    $target = (Resolve-Path -LiteralPath $Path -ErrorAction Stop).Path.TrimEnd('\')
} catch {
    Show-Failure $_.Exception.Message
    exit 1
}

if (-not (Get-Command t3 -ErrorAction SilentlyContinue)) {
    Show-Failure 'The t3 CLI is not on PATH. Run: npm install -g t3@latest'
    exit 1
}

# t3 dispatches to the running desktop server when one is up, so the project appears
# in the sidebar immediately; with no server running it writes the event store directly.
$output = (& t3 project add $target 2>&1 | Out-String)

# An existing project is the normal steady state here and t3 reports it as a hard
# error, so only an unrecognised failure is worth interrupting the user for.
if ($output -notmatch 'Added project|ProjectAlreadyExistsError') {
    Show-Failure "t3 project add failed for ${target}:`n$output"
    exit 1
}

# T3 Code holds a single-instance lock: a second launch reveals the existing window
# rather than opening another one.
Start-Process -FilePath $exe
