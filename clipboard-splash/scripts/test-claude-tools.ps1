# Runs claude the way Claude Mode does (no console, output piped) and reports
# what it did: which tools it called, what was refused, how long it took, and
# every window that appeared while it ran. A console flashing up from a child
# python or bash is exactly what "invisible" rules out, and it only shows when
# the parent has no console of its own, which is why this does not just shell
# out from a terminal.
#
#   powershell -File scripts\test-claude-tools.ps1 -Cwd <dir> -Prompt <text> [-AddDir <dir>]
param(
    [Parameter(Mandatory)] [string] $Cwd,
    [Parameter(Mandatory)] [string] $Prompt,
    [string[]] $AddDir = @(),
    [string] $Model = "claude-opus-5",
    [int] $TimeoutSec = 400,
    [string[]] $Tools = @("Read", "Write", "Edit", "Glob", "Grep", "Bash(python *)", "PowerShell(python *)"),
    [string] $Mode = ""
)

Add-Type @"
using System; using System.Collections.Generic; using System.Runtime.InteropServices; using System.Text;
public static class Win {
  delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc p, IntPtr l);
  [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] static extern int GetClassName(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  public static List<IntPtr> Visible() {
    var found = new List<IntPtr>();
    EnumWindows((h, l) => { if (IsWindowVisible(h)) found.Add(h); return true; }, IntPtr.Zero);
    return found;
  }
  public static string Describe(IntPtr h) {
    var cls = new StringBuilder(256); GetClassName(h, cls, 256);
    uint pid; GetWindowThreadProcessId(h, out pid);
    string name = "?";
    try { name = System.Diagnostics.Process.GetProcessById((int)pid).ProcessName; } catch {}
    return cls + " (" + name + ")";
  }
}
"@

function Quote([string] $arg) { '"' + ($arg -replace '"', '\"') + '"' }

$exe = Join-Path $env:USERPROFILE ".local\bin\claude.exe"
# Same order as run_claude: single-value flags first, variadic ones last.
$argv = @("-p", $Prompt, "--model", $Model, "--effort", "high",
          "--output-format", "stream-json", "--verbose")
if ($Mode) { $argv += @("--permission-mode", $Mode) }
foreach ($dir in $AddDir) { $argv += @("--add-dir", $dir) }
$argv += @("--allowedTools") + $Tools

$info = New-Object System.Diagnostics.ProcessStartInfo
$info.FileName = $exe
$info.Arguments = ($argv | ForEach-Object { Quote $_ }) -join " "
$info.WorkingDirectory = $Cwd
$info.UseShellExecute = $false
$info.CreateNoWindow = $true          # CREATE_NO_WINDOW, as the app passes it
$info.RedirectStandardOutput = $true
$info.RedirectStandardError = $true
$info.RedirectStandardInput = $true
# Run from inside a Claude Code session, the child would inherit that session's
# CLAUDE_* variables; the app starts from a clean environment, so match it.
foreach ($name in @($info.Environment.Keys)) {
    if ($name -like "CLAUDE*") { $info.Environment.Remove($name) | Out-Null }
}

$before = @{}
foreach ($h in [Win]::Visible()) { $before[$h] = $true }

$clock = [Diagnostics.Stopwatch]::StartNew()
$proc = [Diagnostics.Process]::Start($info)
$proc.StandardInput.Close()
$stdout = $proc.StandardOutput.ReadToEndAsync()
$stderr = $proc.StandardError.ReadToEndAsync()

$appeared = @{}
while (-not $proc.HasExited -and $clock.Elapsed.TotalSeconds -lt $TimeoutSec) {
    foreach ($h in [Win]::Visible()) {
        if (-not $before.ContainsKey($h) -and -not $appeared.ContainsKey($h)) {
            $appeared[$h] = "{0,6:N1}s  {1}" -f $clock.Elapsed.TotalSeconds, [Win]::Describe($h)
        }
    }
    Start-Sleep -Milliseconds 50
}
if (-not $proc.HasExited) { $proc.Kill(); "TIMED OUT after $TimeoutSec s" }

"exit {0} after {1:N1}s" -f $proc.ExitCode, $clock.Elapsed.TotalSeconds
"windows that appeared: {0}" -f $appeared.Count
$appeared.Values | Sort-Object
"--- stderr"
$stderr.Result
"--- stream"
$stdout.Result
