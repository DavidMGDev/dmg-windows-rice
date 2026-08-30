# Checks that the overlay stays put while it is on screen unfocused, which is
# how it sits whenever the AutoHotkey script opened it: a background process
# asking for foreground is what the foreground lock refuses, so the panel is
# visible without being focused, and an earlier build read that as a cue to
# close itself half a second after opening.
#
# Run against an installed release build, with the app running:
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts/test-dismiss.ps1
#
# Debug builds skip dismissal entirely, so this only means anything on a release
# build. Dismissal itself is a click outside the panel, which cannot be checked
# without taking over the mouse; do that one by hand.

Add-Type @"
using System; using System.Text; using System.Runtime.InteropServices;
public class W {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
  public static IntPtr Find(int pid) {
    IntPtr found = IntPtr.Zero;
    EnumWindows(delegate(IntPtr h, IntPtr l) {
      uint p; GetWindowThreadProcessId(h, out p);
      if (p == (uint)pid) {
        StringBuilder sb = new StringBuilder(256); GetWindowTextW(h, sb, 256);
        if (sb.ToString() == "Clipboard Splash") { found = h; return false; }
      }
      return true; }, IntPtr.Zero);
    return found; }
}
"@

$proc = Get-Process clipboard-splash -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $proc) { throw "Clipboard Splash is not running" }
$ov = [W]::Find($proc.Id)
if ($ov -eq [IntPtr]::Zero) { throw "no Clipboard Splash window" }

$fail = 0
function Check($name, $ok) {
  if ($ok) { "PASS  $name" } else { $script:fail++; "FAIL  $name" }
}

# SW_SHOWNA puts it on screen without activating: visible, unfocused, and with
# no blur event raised. Two seconds is eight watcher ticks.
[void][W]::ShowWindow($ov, 0)
Start-Sleep -Milliseconds 400
[void][W]::ShowWindow($ov, 8)
Check "shows without being activated" ([W]::IsWindowVisible($ov))
"      foreground is the panel: $([W]::GetForegroundWindow() -eq $ov)"
Start-Sleep -Seconds 2
Check "unfocused window is left alone" ([W]::IsWindowVisible($ov))

[void][W]::ShowWindow($ov, 0)
"`nBy hand: open it and click outside the panel. It should close."
if ($fail) { "$fail check(s) failed"; exit 1 } else { "all checks passed" }
