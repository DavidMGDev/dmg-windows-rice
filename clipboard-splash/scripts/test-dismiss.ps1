# Checks that the overlay dismisses itself whenever it is on screen without
# focus. Run against an installed release build, with the app running:
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts/test-dismiss.ps1
#
# Debug builds skip dismissal entirely, so this only means anything on a
# release build. It puts the overlay on screen for a couple of seconds.

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

# Re-running the exe toggles the running instance, the same path the hotkey and
# the AutoHotkey script take.
if ([W]::IsWindowVisible($ov)) { Start-Process $proc.Path; Start-Sleep -Milliseconds 600 }
Start-Process $proc.Path
Start-Sleep -Milliseconds 900

# A window that holds focus must not be dismissed under it.
Check "focused window stays open" (([W]::IsWindowVisible($ov)) -and ([W]::GetForegroundWindow() -eq $ov))

# SW_SHOWNA puts it on screen without activating: visible, unfocused, and with
# no blur event ever raised. This is the state a missed event leaves behind.
[void][W]::ShowWindow($ov, 0)
Start-Sleep -Milliseconds 600
[void][W]::ShowWindow($ov, 8)
Check "shown unfocused, still up after one tick" ([W]::IsWindowVisible($ov))
Start-Sleep -Milliseconds 1200
Check "unfocused window dismissed itself" (-not [W]::IsWindowVisible($ov))

if ($fail) { "`n$fail check(s) failed"; exit 1 } else { "`nall checks passed" }
