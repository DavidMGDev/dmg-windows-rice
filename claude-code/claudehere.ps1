<#
    claudehere - add "Open Claude Code here" to the Explorer right-click menu.

    Opens Windows Terminal in the clicked folder and starts Claude Code there.

        claudehere.ps1 -Install      add the menu entry
        claudehere.ps1 -Uninstall    remove it
#>
[CmdletBinding()]
param(
    [switch]$Install,
    [switch]$Uninstall
)

$MenuLabel = 'Open Claude Code here'
# Static verbs are drawn after the packaged and shellex handlers and sort
# alphabetically among themselves by key name. The key name is never shown, so
# it is free to pick for placement: "ContinueWithC..." puts this directly above
# ContinueWithT3Code rather than below git_gui and git_shell.
$KeyName   = 'ContinueWithClaudeCode'
$ShellKeys = @(
    'HKCU:\Software\Classes\Directory\shell'             # right-click a folder
    'HKCU:\Software\Classes\Directory\Background\shell'  # right-click inside a folder
)

# The icon is read out of claude.exe itself, so it follows whatever Claude Code
# ships instead of a copy of the logo going stale in this repo.
function Get-ClaudeExe {
    $command = Get-Command claude -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($command) { return $command.Source }

    $fallback = Join-Path $env:USERPROFILE '.local\bin\claude.exe'
    if (Test-Path -LiteralPath $fallback) { return $fallback }

    throw 'Claude Code is not installed: claude.exe is not on PATH and not under ~\.local\bin.'
}

function Install-ClaudeHere {
    $exe = Get-ClaudeExe
    if (-not (Get-Command wt.exe -ErrorAction SilentlyContinue)) {
        throw 'Windows Terminal (wt.exe) is not installed, and this entry launches through it.'
    }

    foreach ($base in $ShellKeys) {
        $key = Join-Path $base $KeyName
        New-Item -Path (Join-Path $key 'command') -Force | Out-Null
        Set-ItemProperty -Path $key -Name '(default)' -Value $MenuLabel
        Set-ItemProperty -Path $key -Name 'Icon' -Value "`"$exe`",0"
        # %V is the clicked folder under both classes. The trailing dot is the
        # drive-root guard: "C:\" would escape the closing quote, and Windows
        # strips a trailing dot during path normalisation anyway. cmd resolves
        # claude off PATH, and /k leaves the output readable after it exits.
        Set-ItemProperty -Path (Join-Path $key 'command') -Name '(default)' `
            -Value 'wt.exe -d "%V." cmd /k claude'
    }

    Write-Host "Added `"$MenuLabel`" to the folder and folder-background context menus."
    Write-Host 'Explorer caches the menu. Restart it: Stop-Process -Name explorer -Force'
}

function Uninstall-ClaudeHere {
    foreach ($base in $ShellKeys) {
        Remove-Item -Path (Join-Path $base $KeyName) -Recurse -Force -ErrorAction SilentlyContinue
    }
    Write-Host 'Removed the context-menu entry. Claude Code itself was left installed.'
}

if ($Install)   { Install-ClaudeHere;   return }
if ($Uninstall) { Uninstall-ClaudeHere; return }

Write-Host 'Usage: claudehere.ps1 -Install | -Uninstall'
