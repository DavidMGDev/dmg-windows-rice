# Continue with T3Code

Open any folder as a T3 Code project from the terminal or from the Explorer
right-click menu. The project is created only if one doesn't already exist for
that path, so running it twice on the same folder just brings the app forward.

```
t3here            # current directory
t3here D:\some\repo
```

Right-click a folder, or right-click empty space inside one, and pick
**Continue with T3Code**.

## Install

```powershell
.\t3here.ps1 -Install
```

That does three things: writes the two context-menu registry keys under
`HKCU`, adds this folder to your user PATH so `t3here` resolves, and runs
`npm install -g t3@latest` if the T3 Code CLI isn't already there. No admin
rights needed, since everything lives under `HKEY_CURRENT_USER`.

Open a new shell afterwards to pick up the PATH change. The context menu works
immediately.

```powershell
.\t3here.ps1 -Uninstall
```

removes the menu entries and the PATH entry. It leaves the `t3` CLI installed,
since you may be using it for other things.

## How it works

T3 Code's desktop app has no command-line interface of its own. The `.exe`
ignores argv apart from Electron's own switches, and while it does register a
`t3code://` protocol handler, the handler only serves the renderer and Clerk's
OAuth callback. There is no `t3code://open?path=...` route. Someone asked for
exactly this in [issue #1811](https://github.com/pingdotgg/t3code/issues/1811)
and it was closed as not planned.

What does exist is the `t3` npm package, the same server that ships inside the
desktop app. It has a `project add` subcommand:

```
t3 project add <path> [--title <title>]
```

The useful part is how it dispatches. It reads
`~/.t3/userdata/server-runtime.json`, and if a desktop server is listening on
the origin recorded there, it POSTs the `project.create` command to that
running server over HTTP. The project shows up in the sidebar right away,
no restart. If nothing is listening, it falls back to writing the event store
at `~/.t3/userdata/state.sqlite` directly, and the app picks it up on next
launch. Both paths work, so the script doesn't need to care whether the app
is currently open.

So `t3here` is just:

1. Resolve the T3 Code exe out of the `t3code://` handler registration, which
   means it keeps working across auto-updates rather than going stale like a
   hardcoded path would.
2. `t3 project add <folder>`.
3. `Start-Process` the exe. T3 Code holds a single-instance lock, so a second
   launch reveals the existing window instead of opening a duplicate.

Step 2 is expected to fail on any folder you've opened before, with
`ProjectAlreadyExistsError`. That's the normal steady state, not a problem, so
the script treats it as success and only shows a dialog for failures it
doesn't recognise.

## The context menu entries

Two keys, both under `HKCU\Software\Classes`, which Windows merges into
`HKEY_CLASSES_ROOT` for the current user:

| Key | Fires when you right-click |
| --- | --- |
| `Directory\shell\T3Code` | a folder |
| `Directory\Background\shell\T3Code` | empty space inside a folder |

Each one holds the label as its default value, an `Icon` value pointing at
`"<t3code.exe>",0` so the entry gets the app icon, `Position` set to `Top`, and
a `command` subkey:

```
wscript.exe "<this folder>\t3here.vbs" "%V"
```

`%V` expands to the clicked folder and is the right token for both keys
(`%1` works for `Directory` but is empty for `Directory\Background`).

`Position` is what keeps the entry near the top. Windows draws
`IExplorerCommand` handlers and `shellex\ContextMenuHandlers` first (Open in
Terminal, which is a packaged command from the Windows Terminal appx, and
Rename with PowerRename), then classic `shell` verbs sorted alphabetically by
key name. A verb called `T3Code` therefore lands below `git_gui` and
`git_shell`, near the bottom of the block. `Position` accepts `Top` or
`Bottom`; `Top` pulls it up to the head of the menu. The alternative, if you'd
rather not pin it, is to rename the key so it sorts ahead of the other verbs.

The `wscript.exe` indirection is there to avoid a console window. Pointing the
command straight at `powershell.exe` would flash a black box for the two-odd
seconds the CLI takes; `-WindowStyle Hidden` doesn't help, because the console
host is created before the style is applied. `t3here.vbs` is three lines that
launch the same script with the window hidden. From a terminal you get
`t3here.cmd` instead, which does show output.

Because the script is invisible when launched that way, errors it can't handle
come back as a message box rather than going to a console nobody is watching.

### Windows 11 and the classic menu

Windows 11's default context menu ignores these keys. It only shows entries
from packaged `IExplorerCommand` handlers, and everything registered the
classic way gets buried under "Show more options".

You already have the registry mod that disables the new menu, so the entry
shows up at the top level. For reference, it's an empty default value on:

```
HKCU\Software\Classes\CLSID\{86ca1aa0-34aa-4e8b-a509-50c905bae2a2}\InprocServer32
```

Setting that up on a new machine, then restarting Explorer:

```powershell
New-Item -Path 'HKCU:\Software\Classes\CLSID\{86ca1aa0-34aa-4e8b-a509-50c905bae2a2}\InprocServer32' -Force | Out-Null
Set-ItemProperty -Path 'HKCU:\Software\Classes\CLSID\{86ca1aa0-34aa-4e8b-a509-50c905bae2a2}\InprocServer32' -Name '(default)' -Value ''
Stop-Process -Name explorer -Force
```

Without the mod, "Continue with T3Code" still works, it just lives one click
deeper under "Show more options".

## Keeping it working

The `t3` CLI talks to the desktop server with a versioned command schema, so
keep the two roughly in step. After T3 Code auto-updates:

```powershell
npm install -g t3@latest
```

Checked against T3 Code 0.0.36 and `t3` 0.0.36 on Windows 11, PowerShell 5.1,
Node 24.19.
