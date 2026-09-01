# Paste into Markdown File

Right-click empty space inside a folder and pick **Paste into Markdown File**.
Whatever text is on your clipboard lands in a new `Clipboard.md` in that folder.

If `Clipboard.md` is already there you get `Clipboard (1).md`, then
`Clipboard (2).md`, the same way Explorer numbers a duplicate paste.

## Install

```powershell
.\pastemd.ps1 -Install
```

One key under `HKCU`, no admin rights. Explorer caches the context menu, so
restart it to see the entry:

```powershell
Stop-Process -Name explorer -Force
```

```powershell
.\pastemd.ps1 -Uninstall
```

removes it again.

It also works from a terminal, where `pastemd.ps1 [path]` writes into `[path]`
or the current directory.

## How it works

The menu entry runs `wscript.exe pastemd.vbs "%V"`, and the VBS launches the
PowerShell script hidden. Calling `powershell.exe` from the registry directly
would flash a console window for as long as the script runs.

**`%V`** is the clicked folder. `%1` is the usual token, but it expands to
nothing under `Directory\Background`, which is the only class this registers
on — pasting into the folder you are looking at is the whole point, so there is
nothing sensible to do with a folder you merely right-clicked.

**Text only.** `Get-Clipboard -Raw` asks for the clipboard's text format, so a
copied image or file gives nothing back rather than some stringified stand-in.
Nothing on the clipboard means a dialog instead of an empty file — under
`wscript` there is no console for an error message to go to.

**No BOM.** The file is written with `[IO.File]::WriteAllText`, since
`Set-Content -Encoding utf8` prepends a byte-order mark on Windows PowerShell
and some markdown tools render it as a stray character on line one.

See [`skills/windows-context-menu`](../skills/windows-context-menu) for the
general rules this follows.
