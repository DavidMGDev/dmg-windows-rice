#Requires AutoHotkey v2.0

; Define the array globally so all functions/hotkeys can see it
GUIs := [] 

; Hotkey to activate: Press Ctrl+Alt+B to go black
!^+b::ToggleBlackScreen()

ToggleBlackScreen() {
    global GUIs ; Tell the function to use the global 'GUIs' variable
    
    ; If screens are already black, destroy them (Wake up)
    if (GUIs.Length > 0) {
        for guiObj in GUIs
            guiObj.Destroy()
        GUIs := [] ; Clear the array
        DllCall("ShowCursor", "Int", 1) ; Show cursor again
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
    
    ; Hide the mouse cursor
    DllCall("ShowCursor", "Int", 0)
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