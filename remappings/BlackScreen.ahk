#Requires AutoHotkey v2.0

GUIs := []
IdleOffMs := 5000  ; how long input must sit still before the panels actually power down

; Hotkey to activate: Press Ctrl+Alt+Shift+B to go black
!^+b::ToggleBlackScreen()

ToggleBlackScreen() {
    global GUIs

    ; If screens are already black, destroy them (Wake up)
    if (GUIs.Length > 0) {
        SetTimer(PowerDownWhenIdle, 0)
        for guiObj in GUIs
            guiObj.Destroy()
        GUIs := []
        DllCall("ShowCursor", "Int", 1)
        return
    }

    ; Loop through all monitors and cover them with a black window
    Loop MonitorGetCount() {
        MonitorGet(A_Index, &L, &T, &R, &B)
        myGui := Gui("+AlwaysOnTop -Caption +ToolWindow +Owner")
        myGui.BackColor := "000000"
        myGui.Show("x" L " y" T " w" (R-L) " h" (B-T) " NoActivate")
        GUIs.Push(myGui)
    }

    DllCall("ShowCursor", "Int", 0)
    SetTimer(PowerDownWhenIdle, 1000)
}

; The overlays stay up the whole time the screen is blanked, so a stray mouse
; twitch wakes the panels back onto black instead of onto the desktop.
PowerDownWhenIdle() {
    global IdleOffMs
    static isOff := false

    if (A_TimeIdlePhysical < IdleOffMs) {
        isOff := false   ; something moved, the display woke itself, arm again
        return
    }
    if (isOff)
        return
    isOff := true
    ; WM_SYSCOMMAND + SC_MONITORPOWER off. Timeout so one hung window can't stall the timer.
    DllCall("SendMessageTimeout", "Ptr", 0xFFFF, "UInt", 0x112, "Ptr", 0xF170, "Ptr", 2, "UInt", 2, "UInt", 500, "Ptr*", 0)
}

; --- Wake Up Keys ---
; These hotkeys ONLY exist when the screen is black (GUIs.Length > 0).
; When the screen is normal, Spacebar acts like a normal Spacebar.

#HotIf GUIs.Length > 0
Space::ToggleBlackScreen()
Esc::ToggleBlackScreen()
Enter::ToggleBlackScreen()
LButton::ToggleBlackScreen() ; Optional: Click mouse to wake
#HotIf
