# Clipboard Splash

A cursor-anchored overlay for clipboard snippets you reuse: file paths, prompts,
boilerplate. Tauri + Svelte 5, no runtime beyond WebView2.

## Use

| Action | Result |
| --- | --- |
| `Win+Alt+C` | Toggle the overlay at the cursor |
| Click an item | Copy it, then dismiss |
| Right-click an item | Edit or delete it |
| `+` on a folder header | Save the current clipboard into that folder |
| Double-click a folder name | Rename |
| Type in the search box | Flat search across every folder; `Enter` copies the top hit |
| `Esc` | Close the editor, or dismiss the overlay |

Windows reserves `Win+V` and `Win+Shift+V` — `RegisterHotKey` refuses both — but
`Win+Alt+C` is free, verified on Win11 26200. No keyboard hook or AutoHotkey
bridge is needed to use the Windows key.

If another app already holds it, registration falls back to `Ctrl+Alt+V` and
then `Ctrl+Shift+` `` ` ``. Whichever took is printed to stderr at startup and
shown in the tray tooltip.

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
