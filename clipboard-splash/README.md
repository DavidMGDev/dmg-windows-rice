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

Every binding in the list is registered, not just the first that succeeds, so
the overlay answers to whichever you reach for. Which ones took is printed to
stderr at startup and shown in the tray tooltip.

The shell reserves `Win+V`, `Win+Shift+V` and `Win+C`; `RegisterHotKey` refuses
all three. `Win+Alt+C` is free either way, verified on Win11 26200.

### Getting `Win+C` back from Copilot

Run the wizard, which does everything below and verifies each step:

```sh
bash scripts/enable-win-c.sh
```

It probes who owns `Win+C` with `RegisterHotKey`, walks you through the one
elevated step, restarts Explorer, and falls back to AutoHotkey if the key
stays reserved. Safe to re-run. The manual route follows.

`Win+C` cannot be claimed through `RegisterHotKey` while the shell holds it, so
`clipboard-splash.ahk` binds it with AutoHotkey v2 instead. A low-level keyboard
hook runs ahead of the shell, so Copilot never sees the keypress and the overlay
gets it. The app is single-instance, so the script just re-runs the exe and the
running copy toggles.

Run the script, or drop a shortcut to it in `shell:startup` to have it always on.

To take `Win+C` away from Copilot permanently instead, run this **as
administrator** — `HKCU\Software\Policies` is ACL'd to admins, so it will fail
otherwise — then sign out and back in:

```powershell
New-Item -Path "HKCU:\Software\Policies\Microsoft\Windows\WindowsCopilot" -Force
Set-ItemProperty -Path "HKCU:\Software\Policies\Microsoft\Windows\WindowsCopilot" `
  -Name TurnOffWindowsCopilot -Value 1 -Type DWord
```

Once that frees the key, the app claims `Win+C` natively on next launch and the
AutoHotkey script is no longer needed.

The app lives in the tray and registers itself for autostart, so the hotkey
works after a reboot.

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
