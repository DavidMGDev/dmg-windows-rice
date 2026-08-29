#Requires AutoHotkey v2.0
#SingleInstance Force

; Win+C is reserved by the Windows shell for Copilot. RegisterHotKey refuses it,
; so the app cannot claim it the way it claims Win+Alt+C. AutoHotkey's low-level
; keyboard hook runs ahead of the shell, so binding it here does two things at
; once: Copilot never sees the keypress, and Clipboard Splash gets it instead.
;
; Clipboard Splash is single-instance, so re-running the exe toggles the overlay
; in the copy that is already running rather than starting a second one.
;
; To start this with Windows, drop a shortcut to it in shell:startup.

exe := EnvGet("LOCALAPPDATA") . "\Clipboard Splash\clipboard-splash.exe"

#c:: {
    if FileExist(exe)
        Run('"' . exe . '"')
    else
        TrayTip("Clipboard Splash not installed", "Expected: " . exe)
}
