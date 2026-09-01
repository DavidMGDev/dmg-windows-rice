<#
    pastemd - write the clipboard's text into a new Clipboard.md in a folder.

    Numbers the file the way Explorer does when one already exists:
    Clipboard.md, Clipboard (1).md, Clipboard (2).md...

        pastemd [path]        defaults to the current directory
        pastemd -Install      add the "Paste into Markdown File" menu entry
        pastemd -Uninstall    remove it
#>
[CmdletBinding()]
param(
    [string]$Path,
    [switch]$Install,
    [switch]$Uninstall
)

$MenuLabel = 'Paste into Markdown File'
$KeyName   = 'PasteIntoMarkdownFile'
# Background only: this pastes into the folder you are looking at, so there is
# nothing sensible to do with a folder you merely right-clicked.
$ShellKey  = 'HKCU:\Software\Classes\Directory\Background\shell'

# Under wscript there is no console to write to, so failures need a dialog.
function Show-Failure($message) {
    if ([Environment]::UserInteractive -and $Host.Name -eq 'ConsoleHost') {
        Write-Error $message
    } else {
        (New-Object -ComObject WScript.Shell).Popup($message, 0, 'Paste into Markdown File', 16) | Out-Null
    }
}

function Install-PasteMd {
    $vbs = Join-Path $PSScriptRoot 'pastemd.vbs'
    $key = Join-Path $ShellKey $KeyName

    New-Item -Path (Join-Path $key 'command') -Force | Out-Null
    Set-ItemProperty -Path $key -Name '(default)' -Value $MenuLabel
    # %V is the clicked folder; %1 expands to nothing under Directory\Background.
    Set-ItemProperty -Path (Join-Path $key 'command') -Name '(default)' `
        -Value "wscript.exe `"$vbs`" `"%V`""

    Write-Host "Added `"$MenuLabel`" to the folder-background context menu."
    Write-Host 'Explorer caches the menu. Restart it: Stop-Process -Name explorer -Force'
}

function Uninstall-PasteMd {
    Remove-Item -Path (Join-Path $ShellKey $KeyName) -Recurse -Force -ErrorAction SilentlyContinue
    Write-Host 'Removed the context-menu entry.'
}

if ($Install)   { Install-PasteMd;   return }
if ($Uninstall) { Uninstall-PasteMd; return }


if (-not $Path) { $Path = $PWD.Path }

try {
    $dir = (Resolve-Path -LiteralPath $Path -ErrorAction Stop).Path
} catch {
    Show-Failure $_.Exception.Message
    exit 1
}

# -Raw asks for the text format specifically, so a copied image or file yields
# nothing here rather than some stringified stand-in.
$text = Get-Clipboard -Raw
if ([string]::IsNullOrEmpty($text)) {
    Show-Failure 'There is no text on the clipboard.'
    exit 1
}

$file = Join-Path $dir 'Clipboard.md'
$n = 1
while (Test-Path -LiteralPath $file) {
    $file = Join-Path $dir "Clipboard ($n).md"
    $n++
}

try {
    # WriteAllText gives UTF-8 without a BOM; Set-Content -Encoding utf8 would
    # prepend one on Windows PowerShell and some markdown tools choke on it.
    [IO.File]::WriteAllText($file, $text, [Text.UTF8Encoding]::new($false))
} catch {
    Show-Failure $_.Exception.Message
    exit 1
}
