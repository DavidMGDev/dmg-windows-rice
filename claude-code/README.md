# Open Claude Code here

Right-click a folder, or right-click empty space inside one, and pick **Open
Claude Code here**. Windows Terminal opens in that folder with Claude Code
already running.

It sits directly above **Continue with T3Code** in the menu, and takes its icon
from `claude.exe`.

## Install

```powershell
.\claudehere.ps1 -Install
```

That writes two keys under `HKCU`, one for the folder menu and one for the
folder-background menu. No admin rights needed. Explorer caches the context
menu, so restart it to see the entry:

```powershell
Stop-Process -Name explorer -Force
```

```powershell
.\claudehere.ps1 -Uninstall
```

removes both keys and leaves Claude Code installed.

Needs `claude` on PATH (or at `~\.local\bin\claude.exe`) and Windows Terminal.
The installer fails with a clear message if either is missing, rather than
writing a menu entry that does nothing.

## How it works

The whole entry is one command line:

```
wt.exe -d "%V." cmd /k claude
```

**`%V`** is the clicked folder. It is the right token for both classes here:
`%1` also works under `Directory`, but expands to nothing under
`Directory\Background`, so an entry using it would silently open in the wrong
place when you right-click empty space.

**The trailing dot** is a drive-root guard. At the root of a drive `%V` expands
to `C:\`, and `"C:\"` ends with a backslash that escapes the closing quote,
mangling the argument. Appending a dot gives `"C:\."`, which parses cleanly and
still points at the same directory, because Windows strips trailing dots during
path normalisation. Git's own "Open Git Bash here" entry uses the same trick.

**`cmd /k`** rather than invoking `claude` directly. Windows Terminal would have
to resolve `claude` itself, and its command-line parsing is awkward about quoted
paths containing spaces; handing the line to `cmd` sidesteps that, since `cmd`
resolves the name off PATH. `/k` keeps the console open after Claude Code exits
so you can still read the output, instead of the window vanishing.

**Placement.** Static verbs are drawn after the packaged and `shellex` handlers,
and sort alphabetically among themselves by key name. The key name is never
shown to the user, so it is free to choose for ordering: `ContinueWithClaudeCode`
sorts just ahead of `ContinueWithT3Code`, which puts the two together and above
`git_gui` and `git_shell`.

See [`skills/windows-context-menu`](../skills/windows-context-menu) for the
general rules this follows.
