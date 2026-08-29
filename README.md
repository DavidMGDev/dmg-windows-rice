# dmg-windows-rice

My Windows setup, kept in one place so a new machine ends up with the same
shortcuts, remappings and small tools as the last one. Everything here is
user-scoped: no admin rights, nothing written outside `HKEY_CURRENT_USER` and
your own profile, except where a script says otherwise.

## Tools

| | |
| --- | --- |
| [`t3code/`](t3code) | **Continue with T3Code.** Opens any folder as a T3 Code project, from the terminal (`t3here`) or the Explorer right-click menu. Creates the project only if it doesn't exist yet. |
| [`remappings/`](remappings) | AutoHotkey v2. `HyperRemap.ahk` kills Caps Lock and turns it into a hyper key for symbols you'd otherwise reach for with a modifier (`Caps+q` → `@`, `Caps+j` → `{}`, and so on). `BlackScreen.ahk` blanks every monitor on `Ctrl+Alt+Shift+B` and hides the cursor, any key wakes it. |
| [`steam/`](steam) | Toggles a firewall rule that blocks `steam.exe` outbound, to force offline mode. Self-elevates, shuts Steam down first, restarts it after. |
| [`skills/windows-context-menu/`](skills/windows-context-menu) | Reference for driving the Explorer right-click menu from the registry: how entries are ordered, how to place one where you want it, and how to fix an item that shows up twice. |
| [`th901-schedule/`](th901-schedule) | TH901 group schedule viewer, a single HTML page over a JSON file. Open `schedule-conflicts.html`, no build step. |
| [`clipboard-splash/`](clipboard-splash) | Tauri + Svelte clipboard viewer. Scaffold only so far: `src-tauri` has no `Cargo.toml` or `tauri.conf.json` yet, so it does not build. |

## Submodules

These are separate published projects. They live here as submodules so one
clone gets the whole working setup, while each keeps its own history and
remote.

| | |
| --- | --- |
| [`claude-init/`](https://github.com/DavidMGDev/claude-init) | One command to set a folder up for Claude Code: installs a fixed set of skills, optionally starts a git repo, opens a session. |
| [`stignore/`](https://github.com/DavidMGDev/stignore) | Manage a Syncthing `.stignore` file from a browser UI, scoped to the folder you run it in. |

## Setting up a new machine

```powershell
git clone --recurse-submodules https://github.com/DavidMGDev/dmg-windows-rice.git
cd dmg-windows-rice
```

If you already cloned without the submodules:

```powershell
git submodule update --init --recursive
```

Then, per tool:

- **T3Code menu entry:** `.\t3code\t3here.ps1 -Install`. Needs
  [T3 Code](https://github.com/pingdotgg/t3code) and Node 22.16+. See
  [`t3code/README.md`](t3code/README.md), including the registry mod that
  brings back the classic Windows 11 context menu.
- **Remappings:** install [AutoHotkey v2](https://www.autohotkey.com/), then
  drop shortcuts to both `.ahk` files in `shell:startup` so they come back
  after a reboot.
- **Steam toggle:** run it when you want it, nothing to install.

To pull the submodules up to their latest published commits later:

```powershell
git submodule update --remote --merge
```
