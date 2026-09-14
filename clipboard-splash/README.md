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
white box, black system text, no corners and no shadow. It follows the mouse
for ten seconds and then it is gone.

Nothing else shows up while it works. No terminal, no taskbar button, no
placeholder text to read and discard, and no window that takes the keyboard off
whatever you were typing in. The only sign it is busy is the cursor, which
turns to the working arrow until the answer lands.

### One line on screen, the rest on the clipboard

The label shows the first line of the answer, up to 40 characters. The whole
answer goes to the clipboard, so anything that did not fit is a paste away.
When that happens a small grey dot appears at the head of the label, drawn
rather than written so it can never be read as a character Claude chose to put
there. No dot means the line is the whole of it and your clipboard was left
alone.

Line one is worth spending the prompt on. The one below asks for an answer
that stands on its own in 40 characters and says when the clipboard is worth
opening:

```text
You are looking at a screenshot of my screen. Work out what I am doing, what I
am trying to reach, and what is stopping me. Then answer in exactly this shape.

Line 1: the answer itself, under 40 characters. Not a description of the answer,
not a restatement of my problem, no preamble, no "the screenshot shows", no
trailing period. If the answer is a value, a formula, a setting, a name or a
number, line 1 is that thing and nothing else. This is the only line I will see.

Line 2: blank.

Line 3 onward: the complete answer in at most three short paragraphs. What is
wrong, the fix, and the steps to apply it, including whatever my attempt is
missing for the fix to work. Plain sentences. No headings, no bullets, no
markdown, no code fences unless the answer is literally code: this gets pasted
into other programs exactly as you write it.

Everything from line 3 down lands on my clipboard automatically, and I will only
paste it if line 1 tells me it is worth it. So when I cannot act on line 1
alone, line 1 has to say so in its own words, like "Missing a JOIN, steps
copied" or "Wrong sheet, fix copied". When line 1 is the whole answer, say so by
not saying anything about the clipboard, and still write the rest as the
reasoning behind it.

If you genuinely cannot answer without knowing something from me, make line 1
that one short question. I can hold the hotkey and type an answer back.

When I do type back, keep the same shape, but line 1 may run to 80 characters.
```

Save it as a snippet named `Current-Prompt` the way you save any other. Case
and surrounding spaces do not matter, the first match across every folder wins,
and with no match at all you get `(Err)`.

### Typing back

Hold the hotkey for about a second and the label turns into a line you type
into, starting at three dots. Enter sends it, Esc drops it, and so does another
press of the hotkey. It continues the same conversation the screenshot started,
so the answer arrives with everything already said still in context. Answers to
something you typed are set a size smaller and run to 80 characters, on the
grounds that you are already looking at the label to read them.

Nothing is focused while you type. The label cannot take the keyboard without
stealing it from what is in front, so the keys are taken off the machine with a
keyboard hook and swallowed for as long as the line is open — otherwise the
question would be typed into the spreadsheet behind it. Modifiers still go
through, because eating a keyup leaves the app in front believing the key is
stuck down. A minute with nothing typed closes the line on its own.

Follow-ups carry no new screenshot. The conversation still has the first one,
and a second capture would mostly be a picture of the label.

| When you press it again | Hotkey does |
| --- | --- |
| Nothing on screen | Asks a new question about the screen |
| A query is still out | Drops it silently, answer never arrives |
| An answer is on screen | Drops it, never to be seen again |
| `(Err)` is on screen | Swaps it for why it failed, or for what the terminal said |
| A line is being typed | Drops the line, sends nothing |

Holding it does the same clearing on the way down, then opens the line.

### Letting it write files

Claude can only read unless the prompt names a folder. Put an absolute path
in `Current-Prompt`, like `C:\School\Parcial-I\`, and the query runs from that
folder with it added via `--add-dir` under `--permission-mode acceptEdits`, so
Claude can create and edit files there and is refused anywhere else. It may
also run `python`, which is how it gets at scripts kept in the folder. Paths
with spaces are fine. Each path is cut back to the longest part that exists as
a folder, so a path to a file inside it grants the folder. A bare drive never
counts. Typed follow-ups keep the folders of the query they continue.

Python is the loose end. It runs with your rights and writes wherever the
script points, and the screenshot is text Claude reads and may act on. Only
name a folder in prompts you point at screens you trust.

`scripts/test-claude-tools.ps1` runs claude the same hidden way and reports
which tools it used, what got refused, how long it took, and every window that
appeared, which is how "nothing flashes on screen" was checked.

### Running it

Needs Claude Code on the machine, found at `~/.local/bin/claude.exe` or on
`PATH`. Queries run `claude-opus-5` at high effort, with `--allowedTools Read`
and nothing else when the prompt names no folder, and a five minute cap, as a
query that fills a workbook takes a minute or more. The model name carries no
`[1m]` suffix, so this is the 200k context
window. Output comes back as `--output-format json`, which is only there for
the session id that makes typing back land in the same conversation.

The working cursor is the system arrow, swapped for the duration and put back
with `SPI_SETCURSORS`. Our own label is never under the pointer and an
unfocused window cannot set the cursor anywhere else, so there is no narrower
way to do it. Kill the app mid-query and the arrow stays busy until something
reloads the cursor scheme.

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
