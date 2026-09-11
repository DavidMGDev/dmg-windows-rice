# Clipboard Splash

A cursor-anchored overlay for clipboard snippets you reuse: file paths, prompts,
boilerplate. Tauri + Svelte 5, no runtime beyond WebView2.

## Use

| Action | Result |
| --- | --- |
| `Win+Alt+C` | Toggle the overlay at the cursor |
| `Ctrl+Alt+V` | Same, second binding |
| `Win+C` | Same, but needs the AutoHotkey script below |
| Click an item | Copy it, then dismiss |
| Right-click an item | Edit or delete it |
| `+` on a folder header | Save the current clipboard into that folder |
| Double-click a folder name | Rename |
| Type in the search box | Flat search across every folder; `Enter` copies the top hit |
| `Esc` | Close the editor, or dismiss the overlay |

A click outside the panel closes it, and so does losing focus. The click is
watched for separately because focus is not reliable here: opened from the
AutoHotkey script the panel is often never the foreground window at all, since a
background process asking for foreground is what Windows' foreground lock
refuses, and a window that was never focused raises no blur event when you click
away from it. `scripts/test-dismiss.ps1` checks the panel is not closed out from
under you while it sits there unfocused.

Every binding in the list is registered, not just the first that succeeds, so
the overlay answers to whichever you reach for. Which ones took is printed to
stderr at startup and shown in the tray tooltip.

The shell reserves `Win+V`, `Win+Shift+V` and `Win+C`; `RegisterHotKey` refuses
all three. `Win+Alt+C` is free either way, verified on Win11 26200.

### Getting `Win+C`

Run the wizard, which does everything below and verifies each step:

```sh
bash scripts/enable-win-c.sh
```

It probes who owns `Win+C` with `RegisterHotKey`, walks you through the one
elevated step, restarts Explorer, and falls back to AutoHotkey if the key
stays reserved. Safe to re-run. The manual route follows.

`Win+C` cannot be claimed through `RegisterHotKey`: something already holds it and
registration returns `ERROR_HOTKEY_ALREADY_REGISTERED`. `clipboard-splash.ahk`
binds it with AutoHotkey v2 instead, whose low-level keyboard hook runs ahead of
that. The app is single-instance, so the script re-runs the exe and the copy
already running toggles.

The script sets `A_MenuMaskKey := "vk07"`, which is load-bearing. AutoHotkey
masks the Win keyup so releasing it does not open the Start menu, and the default
mask is `vk11` (Ctrl). Any tool hooking Ctrl+Win then sees that mask as its own
shortcut: OpenWhispr binds dictation to Control+Super and fired on every `Win+C`
until the mask moved to an unassigned key.

Run the script, or drop a shortcut to it in `shell:startup` to have it always on.

Disabling Copilot does **not** free `Win+C`, tested on Win11 26200: the key stayed
registered after `TurnOffWindowsCopilot=1` and an Explorer restart. Treat the
AutoHotkey route as the answer rather than a fallback.

The app lives in the tray and registers itself for autostart, so the hotkey
works after a reboot.

## Tray menu

| Item | Does |
| --- | --- |
| Show | Open the overlay at the cursor |
| Claude Mode | Point the hotkey at Claude instead of the overlay |
| Start with Windows | Toggle the autostart entry |
| Disable Windows Copilot | Toggle `TurnOffWindowsCopilot`; prompts for UAC |
| Quit | Exit, releasing the hotkeys |

Disabling Copilot writes to the Policies hive, which Windows ACLs to
administrators, so it elevates via `reg.exe` behind a UAC prompt. It runs on its
own thread so the tray does not hang while that prompt is up.

## Claude Mode

Tick it in the tray and the hotkey stops opening the overlay. Instead it
screenshots the screen, sends your saved snippet labelled `Current-Prompt`
along with it to `claude -p`, and puts the answer next to the cursor: one line,
first 40 characters, white box, black system text, no corners and no shadow.
It follows the mouse for ten seconds and then it is gone.

Nothing else shows up. No terminal, no taskbar button, no window that takes the
keyboard off whatever you were typing in.

| While the label is up | Hotkey does |
| --- | --- |
| An answer, or `…` waiting on one | Drops it, never to be seen again |
| `(Err)` | Swaps it for why it failed, or for what the terminal said |

Make the snippet the way you make any other: save it into a folder and name it
`Current-Prompt`. Case and surrounding spaces do not matter, the first match
across every folder wins, and with no match at all you get `(Err)`.

Needs Claude Code on the machine, found at `~/.local/bin/claude.exe` or on
`PATH`. It runs with `--allowedTools Read`, so it can read the screenshot and
nothing else can be written. Queries are capped at 90 seconds.

The mode lives in memory only: a restart lands back in normal mode, which for
something this invisible beats having it quietly survive a reboot.

## Data

One JSON file: `%APPDATA%\com.davidmg.clipboardsplash\clips.json`. Written on
every change. Back it up or edit it by hand if you like.

## Develop

```sh
pnpm install
pnpm tauri dev     # dismiss-on-blur is disabled in debug builds
pnpm tauri build   # NSIS installer in src-tauri/target/release/bundle
```

Prerequisites: Node, Rust (MSVC toolchain), and VS Build Tools with the C++
workload.

## Not built yet

- Pasting into the window behind the overlay. Clicking copies; you paste.
- Reordering or moving items between folders.
- Images and other non-text clipboard formats.
