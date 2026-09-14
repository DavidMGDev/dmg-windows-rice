use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::menu::{CheckMenuItem, Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewWindow};
use tauri_plugin_autostart::{ManagerExt, MacosLauncher};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

/// Every one of these is registered, not just the first that takes, so the
/// overlay answers to whichever the user reaches for.
///
/// The shell reserves Win+V, Win+Shift+V and Win+C, and `RegisterHotKey`
/// refuses all three; Win+C only frees up once Copilot is turned off. Win+Alt+C
/// is free either way (verified on Win11 26200). The rest avoid the usual
/// conflicts: Ctrl+Shift+V is paste-as-plain-text in browsers and VS Code, and
/// Ctrl+` toggles the VS Code terminal.
const HOTKEYS: [&str; 3] = ["Super+KeyC", "Super+Alt+KeyC", "Ctrl+Alt+KeyV"];

#[cfg(windows)]
const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
#[cfg(windows)]
const DWMWA_BORDER_COLOR: u32 = 34;

#[cfg(windows)]
fn dwm_set(win: &WebviewWindow, attribute: u32, value: u32) {
    use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;
    if let Ok(hwnd) = win.hwnd() {
        unsafe {
            DwmSetWindowAttribute(
                hwnd.0 as _,
                attribute,
                std::ptr::addr_of!(value).cast(),
                std::mem::size_of::<u32>() as u32,
            );
        }
    }
}

/// Win11 does not round undecorated windows on its own, and a CSS shadow would
/// be clipped by the window rect. Handing both to DWM keeps the shadow outside
/// the window where it cannot be cut off.
#[cfg(windows)]
fn round_corners(win: &WebviewWindow) {
    const DWMWCP_ROUND: u32 = 2;
    dwm_set(win, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND);
}

/// Strips the Claude Mode label down to nothing but its own pixels.
///
/// Square, because the label should not read as a window at all. Borderless,
/// because tao keeps `WS_CAPTION` on undecorated windows and hides the frame in
/// `WM_NCCALCSIZE`, which leaves DWM still drawing its border: a dark line
/// along the top edge of every label. And never activated, because it appears
/// while you are working in something else and must not take the keyboard off
/// it — Tauri's `focus: false` does not cover that, since tao clears its
/// don't-focus marker after the first show and every show after that activates.
#[cfg(windows)]
fn bare_window(win: &WebviewWindow) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_NOACTIVATE,
    };
    const DWMWCP_DONOTROUND: u32 = 1;
    const DWMWA_COLOR_NONE: u32 = 0xFFFF_FFFE;

    dwm_set(win, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND);
    dwm_set(win, DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE);

    if let Ok(hwnd) = win.hwnd() {
        unsafe {
            let style = GetWindowLongPtrW(hwnd.0 as _, GWL_EXSTYLE);
            SetWindowLongPtrW(hwnd.0 as _, GWL_EXSTYLE, style | WS_EX_NOACTIVATE as isize);
        }
    }
}

/// Swaps the arrow for the working cursor while a query is out, and hands it
/// back afterwards. The only sign Claude Mode gives that it is busy.
///
/// ponytail: this is the system cursor, not ours — our label is not under the
/// pointer and an unfocused window cannot set the cursor anywhere else. Kill
/// the process mid-query and the arrow stays busy until something reloads the
/// cursor scheme. Worth it for feedback that costs no pixels.
#[cfg(windows)]
fn busy_cursor(busy: bool) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CopyIcon, LoadCursorW, SetSystemCursor, SystemParametersInfoW, IDC_APPSTARTING, OCR_NORMAL,
        SPI_SETCURSORS,
    };

    unsafe {
        if busy {
            // SetSystemCursor destroys the handle it is given, and the one
            // LoadCursorW hands back is shared, so give it a copy.
            let working = CopyIcon(LoadCursorW(std::ptr::null_mut(), IDC_APPSTARTING));
            SetSystemCursor(working, OCR_NORMAL);
        } else {
            // Reloads every cursor from the registry, undoing the swap.
            SystemParametersInfoW(SPI_SETCURSORS, 0, std::ptr::null_mut(), 0);
        }
    }
}

/// Gap between the cursor and the panel, in physical pixels.
const GAP: i32 = 14;

#[cfg(windows)]
const COPILOT_KEY: &str = r"HKCU\Software\Policies\Microsoft\Windows\WindowsCopilot";
/// Keeps `reg.exe` and `powershell.exe` from flashing a console window.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[cfg(windows)]
fn copilot_disabled() -> bool {
    use std::os::windows::process::CommandExt;
    std::process::Command::new("reg")
        .args(["query", COPILOT_KEY, "/v", "TurnOffWindowsCopilot"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(|out| {
            // Last token is the value, e.g. "0x1". Comparing the whole token
            // rather than searching avoids matching 0x10, 0x11 and friends.
            String::from_utf8_lossy(&out.stdout)
                .split_whitespace()
                .last()
                == Some("0x1")
        })
        .unwrap_or(false)
}

/// Windows ACLs the Policies hive to administrators, so this has to elevate.
/// Blocks on the UAC prompt, so callers run it off the menu thread.
#[cfg(windows)]
fn set_copilot_disabled(disable: bool) {
    use std::os::windows::process::CommandExt;
    let value = u8::from(disable);
    let script = format!(
        "Start-Process reg -Verb RunAs -WindowStyle Hidden -Wait -ArgumentList \
         'add','{COPILOT_KEY}','/v','TurnOffWindowsCopilot','/t','REG_DWORD','/d','{value}','/f'"
    );
    let _ = std::process::Command::new("powershell")
        .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW)
        .status();
}

fn store_path(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .expect("no app data dir")
        .join("clips.json")
}

#[tauri::command]
fn load_clips(app: AppHandle) -> String {
    fs::read_to_string(store_path(&app)).unwrap_or_default()
}

#[tauri::command]
fn save_clips(app: AppHandle, data: String) -> Result<(), String> {
    let path = store_path(&app);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    fs::write(path, data).map_err(|e| e.to_string())
}

/// The endpoint Claude Code's own `/usage` screen calls. Undocumented, and
/// flagged experimental in Claude Code's bundle, so treat a failure as normal.
const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";

/// Claude Code parks an OAuth token here in plaintext and refreshes it as it
/// runs. We only read it; refreshing is left to Claude Code.
fn oauth_token() -> Option<String> {
    let home = std::env::var("USERPROFILE").ok()?;
    let path = PathBuf::from(home).join(".claude").join(".credentials.json");
    let creds: serde_json::Value = serde_json::from_str(&fs::read_to_string(path).ok()?).ok()?;
    creds["claudeAiOauth"]["accessToken"]
        .as_str()
        .map(str::to_owned)
}

/// Returns `{ five_hour, seven_day, ... }`, each `{ utilization, resets_at }`
/// with utilization as a 0-100 percentage.
#[tauri::command]
async fn fetch_usage() -> Result<serde_json::Value, String> {
    let token = oauth_token().ok_or("no Claude credentials on this machine")?;
    let response = reqwest::Client::new()
        .get(USAGE_URL)
        .bearer_auth(token)
        .header("anthropic-beta", "oauth-2025-04-20")
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    match response.status() {
        s if s.is_success() => response.json().await.map_err(|e| e.to_string()),
        s if s.as_u16() == 401 => Err("token expired, open Claude Code to refresh".into()),
        s => Err(format!("usage endpoint returned {s}")),
    }
}

/// Places a window next to the cursor, flipping and then clamping so it always
/// lands fully inside the monitor the cursor is on.
///
/// ponytail: clamps to full monitor bounds, not the work area, so a panel
/// pinned to the very bottom edge can sit under the taskbar. Swap in
/// `monitor.work_area()` if that ever actually bites.
fn place_near_cursor(win: &WebviewWindow) -> tauri::Result<()> {
    let cursor = win.app_handle().cursor_position()?;
    let monitor = match win.monitor_from_point(cursor.x, cursor.y)? {
        Some(m) => Some(m),
        None => win.primary_monitor()?,
    };

    if let Some(monitor) = monitor {
        let size = win.outer_size()?;
        let (w, h) = (size.width as i32, size.height as i32);

        let origin = monitor.position();
        let area = monitor.size();
        let (min_x, min_y) = (origin.x, origin.y);
        let (max_x, max_y) = (origin.x + area.width as i32, origin.y + area.height as i32);

        let (cx, cy) = (cursor.x as i32, cursor.y as i32);

        // Prefer down-right of the cursor; flip to the other side if it overflows.
        let mut x = cx + GAP;
        if x + w > max_x {
            x = cx - GAP - w;
        }
        let mut y = cy + GAP;
        if y + h > max_y {
            y = cy - GAP - h;
        }

        // A monitor narrower than the panel would make the clamp range invalid.
        x = x.clamp(min_x, (max_x - w).max(min_x));
        y = y.clamp(min_y, (max_y - h).max(min_y));

        win.set_position(PhysicalPosition::new(x, y))?;
    }
    Ok(())
}

fn show_near_cursor(win: &WebviewWindow) -> tauri::Result<()> {
    place_near_cursor(win)?;

    // Setting this in setup() does not survive to first paint, so re-apply it
    // here. Idempotent and a single cheap syscall.
    #[cfg(windows)]
    round_corners(win);

    win.show()?;
    win.set_focus()?;
    win.emit("shown", ())?;
    Ok(())
}

/// Dismiss-on-blur is driven by an event, and an event is a transition, so a
/// window that loses focus without one landing sits there with nothing left to
/// dismiss it: clicking outside changes no focus state either, and the only way
/// out is to click the panel and click away again.
///
/// Focus is the wrong thing to watch. Launched from the AutoHotkey script the
/// panel often never becomes the foreground window at all — a background process
/// asking for foreground is exactly what Windows' foreground lock refuses — so
/// treating unfocused as dismissable closed the panel out from under the user
/// before they could click anything. Watch for the gesture that actually means
/// dismiss instead: a click that lands outside the panel.
///
/// ponytail: polls the mouse buttons rather than installing WH_MOUSE_LL. A hook
/// puts us on the input path for every click on the machine, which is a far
/// worse thing to get wrong than a 60ms sample.
#[cfg(all(windows, not(debug_assertions)))]
fn dismiss_on_click_away(win: WebviewWindow) {
    use windows_sys::Win32::Foundation::{POINT, RECT};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VK_LBUTTON, VK_MBUTTON, VK_RBUTTON,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetCursorPos, GetWindowRect, ShowWindow, SW_HIDE,
    };

    let any_button_down = || {
        [VK_LBUTTON, VK_RBUTTON, VK_MBUTTON]
            .iter()
            .any(|vk| unsafe { GetAsyncKeyState(*vk as i32) } as u16 & 0x8000 != 0)
    };

    std::thread::spawn(move || {
        // Starts true so a button already held when the app launches does not
        // read as a fresh press.
        let mut was_down = true;
        loop {
            std::thread::sleep(Duration::from_millis(60));

            let down = any_button_down();
            let pressed = down && !was_down;
            was_down = down;

            if !pressed || !win.is_visible().unwrap_or(false) {
                continue;
            }
            let Ok(hwnd) = win.hwnd() else { return };

            unsafe {
                let mut rect: RECT = std::mem::zeroed();
                let mut cursor: POINT = std::mem::zeroed();
                if GetWindowRect(hwnd.0 as _, &mut rect) == 0 || GetCursorPos(&mut cursor) == 0 {
                    continue;
                }
                let inside = cursor.x >= rect.left
                    && cursor.x < rect.right
                    && cursor.y >= rect.top
                    && cursor.y < rect.bottom;
                if inside {
                    continue;
                }
                win.hide().ok();
                // tao's `set_visible` diffs against cached window flags and does
                // nothing when they already say hidden, so a window that is
                // really on screen with a stale flag cannot be closed by the app
                // at all. Hide it outright; `hide()` above is only there to keep
                // the cached flag in step for the next show.
                ShowWindow(hwnd.0 as _, SW_HIDE);
            }
        }
    });
}

// --- Claude Mode -----------------------------------------------------------
//
// The hotkey stops opening the gallery and instead sends the snippet labelled
// `Current-Prompt` to Claude with a screenshot of the screen, then shows the
// answer in a label that follows the cursor. Nothing else appears: no terminal,
// no taskbar entry, no panel.

/// ponytail: lives in memory, so a restart lands back in normal mode. A mode
/// this invisible silently surviving a reboot is worse than one you opt into.
static CLAUDE_MODE: AtomicBool = AtomicBool::new(false);

/// Bumped by every show and every dismissal. A worker that finds the counter
/// moved on knows it is stale and drops what it was doing, which is what makes
/// "press again and it is gone, never to be seen again" hold mid-query too.
static TIP_GEN: AtomicU64 = AtomicU64::new(0);

struct Tip {
    /// The failure reason held back behind `(Err)`, revealed on the next press.
    detail: Option<String>,
}

/// `Some` from the moment a query is claimed until the label leaves the screen.
static TIP: Mutex<Option<Tip>> = Mutex::new(None);

/// The Claude Code conversation a typed follow-up resumes, so holding the
/// hotkey and typing carries on from the screenshot rather than starting over.
/// The folders go with it: Claude Code files a session under the folder it ran
/// from, so a resume run from anywhere else would not find it.
static SESSION: Mutex<Option<(String, Vec<PathBuf>)>> = Mutex::new(None);

/// Everything the label knows about itself. Position and lifetime stay here;
/// the webview only draws this and reports back how wide it came out.
struct Label {
    text: String,
    /// Smaller type and twice the characters. An answer to a line you typed is
    /// one you are already looking at the label to read.
    small: bool,
    /// The rest of the answer is on the clipboard. Drawn as a grey dot rather
    /// than written, so it can never be mistaken for something Claude said.
    dot: bool,
}

impl Label {
    fn err() -> Self {
        Label { text: "(Err)".into(), small: false, dot: false }
    }

    /// A line of ours rather than an answer: a failure reason, or the prompt.
    fn plain(text: &str) -> Self {
        Label { text: one_line(text, 80), small: true, dot: false }
    }

    /// What is being typed. A typed line has no length limit, so the label
    /// shows the end of it, the way a one-line field scrolls to the caret.
    fn typing(line: &str) -> Self {
        let tail: String = {
            let chars: Vec<char> = line.chars().collect();
            chars[chars.len().saturating_sub(90)..].iter().collect()
        };
        Label { text: if tail.is_empty() { "…".into() } else { tail }, small: true, dot: false }
    }
}

/// A glance, not a read: one line, `limit` characters.
fn one_line(text: &str, limit: usize) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(limit)
        .collect()
}

/// Splits an answer into what goes on screen and what goes to the clipboard:
/// the first line up to `limit` characters, and the whole of it whenever any
/// part did not fit.
fn glance(answer: &str, limit: usize) -> (String, Option<String>) {
    let head = one_line(answer.lines().next().unwrap_or(""), limit);
    let whole = answer.trim();
    let rest = (whole != head).then(|| whole.to_string());
    (head, rest)
}

fn tip_window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window("tip")
}

fn emit_tip(app: &AppHandle, label: &Label) {
    let _ = app.emit_to(
        "tip",
        "tip",
        serde_json::json!({ "text": label.text, "small": label.small, "dot": label.dot }),
    );
}

fn hide_tip(app: &AppHandle) {
    #[cfg(windows)]
    busy_cursor(false);
    TIP_GEN.fetch_add(1, Ordering::SeqCst);
    *TIP.lock().unwrap() = None;
    if let Some(win) = tip_window(app) {
        let _ = win.hide();
    }
}

/// Shows a label beside the cursor for `secs`, following the mouse while it is
/// up. The webview sizes the window to the text, so the box hugs whatever it
/// ends up being.
fn show_tip(app: &AppHandle, label: Label, detail: Option<String>, secs: u64) {
    let Some(win) = tip_window(app) else { return };

    *TIP.lock().unwrap() = Some(Tip { detail });
    let generation = TIP_GEN.fetch_add(1, Ordering::SeqCst) + 1;

    emit_tip(app, &label);
    let _ = place_near_cursor(&win);
    let _ = win.show();

    std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(secs);
        while Instant::now() < deadline {
            if TIP_GEN.load(Ordering::SeqCst) != generation {
                return; // dismissed, or replaced by a newer label
            }
            let _ = place_near_cursor(&win);
            std::thread::sleep(Duration::from_millis(16));
        }
        if TIP_GEN.load(Ordering::SeqCst) == generation {
            *TIP.lock().unwrap() = None;
            let _ = win.hide();
        }
    });
}

/// The first snippet labelled `Current-Prompt`, in whichever folder it sits.
fn current_prompt(app: &AppHandle) -> Option<String> {
    find_current_prompt(&fs::read_to_string(store_path(app)).ok()?)
}

fn find_current_prompt(stored: &str) -> Option<String> {
    let stored: serde_json::Value = serde_json::from_str(stored).ok()?;
    stored["folders"]
        .as_array()?
        .iter()
        .flat_map(|folder| folder["items"].as_array().into_iter().flatten())
        .find(|item| {
            item["label"]
                .as_str()
                .is_some_and(|label| label.trim().eq_ignore_ascii_case("current-prompt"))
        })?["text"]
        .as_str()
        .map(str::to_owned)
}

/// Folders the prompt names by absolute path, which is how a prompt gives Claude
/// somewhere to write. A path may hold spaces and run straight on into the
/// sentence, so each is cut back to the longest stretch that is a real folder.
/// A bare drive never counts.
///
/// ponytail: cuts back a character at a time, one stat each. Prompts are a few
/// paragraphs, so that is a few hundred stats at most.
fn prompt_dirs(prompt: &str, is_dir: impl Fn(&Path) -> bool) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    for line in prompt.lines() {
        let bytes = line.as_bytes();
        for start in 0..bytes.len().saturating_sub(2) {
            let drive = bytes[start].is_ascii_alphabetic()
                && bytes[start + 1] == b':'
                && matches!(bytes[start + 2], b'\\' | b'/')
                && (start == 0 || !bytes[start - 1].is_ascii_alphanumeric());
            if !drive {
                continue;
            }
            let found = (start + 4..=line.len())
                .rev()
                .filter(|&end| line.is_char_boundary(end))
                .map(|end| line[start..end].trim_end_matches(['\\', '/']))
                .find(|path| path.len() > 3 && is_dir(Path::new(path)));
            if let Some(path) = found.map(PathBuf::from) {
                if !dirs.contains(&path) {
                    dirs.push(path);
                }
            }
        }
    }
    dirs
}

/// ponytail: shells out to PowerShell for the capture rather than taking on a
/// screenshot crate or hand-rolling GDI+ encoding. It costs a few hundred ms on
/// a path that already waits seconds on Claude.
#[cfg(windows)]
fn screenshot() -> Option<PathBuf> {
    use std::os::windows::process::CommandExt;

    let path = std::env::temp_dir().join("clipboard-splash-screen.png");
    let script = format!(
        "Add-Type -AssemblyName System.Windows.Forms,System.Drawing; \
         $bounds = [System.Windows.Forms.SystemInformation]::VirtualScreen; \
         $bmp = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height; \
         $g = [System.Drawing.Graphics]::FromImage($bmp); \
         $g.CopyFromScreen($bounds.Left, $bounds.Top, 0, 0, $bmp.Size); \
         $bmp.Save('{}', [System.Drawing.Imaging.ImageFormat]::Png)",
        path.display()
    );
    let _ = fs::remove_file(&path);
    let _ = std::process::Command::new("powershell")
        .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW)
        .status();
    path.exists().then_some(path)
}

/// Claude Code's own installer drops the binary here. Preferred over bare
/// `claude` because a GUI process inherits the registry PATH, which may not
/// have picked up an install from this login session yet.
#[cfg(windows)]
fn claude_exe() -> PathBuf {
    std::env::var("USERPROFILE")
        .map(|home| Path::new(&home).join(".local").join("bin").join("claude.exe"))
        .ok()
        .filter(|path| path.exists())
        .unwrap_or_else(|| PathBuf::from("claude"))
}

/// Returns the answer and the session it landed in, which is what lets a typed
/// follow-up resume the same conversation. `dirs` are the folders it may write
/// in; with none it can only read.
#[cfg(windows)]
fn run_claude(
    prompt: &str,
    shot: Option<&Path>,
    resume: Option<&str>,
    dirs: &[PathBuf],
) -> Result<(String, Option<String>), String> {
    use std::os::windows::process::CommandExt;
    use std::sync::mpsc;

    let prompt = match shot {
        Some(path) => format!(
            "{prompt}\n\nA screenshot of the screen right now is at {}. Read it first.",
            path.display()
        ),
        None => prompt.to_string(),
    };

    let mut command = std::process::Command::new(claude_exe());
    command
        .arg("-p")
        .arg(&prompt)
        // Plain `claude-opus-5` is the 200k window; the million-token one is a
        // `[1m]` suffix, and this asks a question about one screenshot.
        .args(["--model", "claude-opus-5", "--effort", "high"])
        // json rather than text only for the session id that comes with it.
        .args(["--output-format", "json"]);
    if let Some(session) = resume {
        // `--resume` takes an optional value, so its id follows it immediately.
        command.args(["--resume", session]);
    }
    // Writing is scoped by acceptEdits, which approves edits inside the folder
    // Claude runs from and the added ones and refuses the rest. Putting Write
    // in --allowedTools instead approves it anywhere on disk, tested. Python is
    // what runs the scripts in those folders, and it runs with the user's
    // rights wherever it points.
    let tools: &[&str] = match dirs.first() {
        None => &["Read"],
        Some(home) => {
            command.current_dir(home).args(["--permission-mode", "acceptEdits"]);
            for dir in dirs {
                command.arg("--add-dir").arg(dir);
            }
            &["Read", "Glob", "Grep", "Bash(python *)", "PowerShell(python *)"]
        }
    };
    // The prompt goes before --add-dir and --allowedTools: both are variadic
    // and swallow any positional that follows them.
    command
        .arg("--allowedTools")
        .args(tools)
        .stdin(std::process::Stdio::null())
        .creation_flags(CREATE_NO_WINDOW);

    // `output()` has no timeout and this is a background thread with a label
    // pinned to the screen, so put the wait on a channel that does. Five
    // minutes because a query that fills a workbook runs a minute on a small
    // model and longer on a big one.
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || tx.send(command.output()));

    let output = match rx.recv_timeout(Duration::from_secs(300)) {
        Ok(Ok(output)) => output,
        Ok(Err(e)) => return Err(format!("could not run claude: {e}")),
        Err(_) => return Err("claude did not answer within 5 minutes".into()),
    };

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let said = if stderr.is_empty() { stdout } else { stderr };
        return Err(if said.is_empty() {
            format!("claude exited {}", output.status)
        } else {
            said
        });
    }
    if stdout.is_empty() {
        return Err("claude answered with nothing".into());
    }

    // Anything that is not the shape we asked for is still worth showing.
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) else {
        return Ok((stdout, None));
    };
    let session = json["session_id"].as_str().map(str::to_owned);
    let answer = json["result"].as_str().unwrap_or(&stdout).trim().to_string();
    if json["is_error"].as_bool() == Some(true) {
        return Err(if answer.is_empty() { "claude reported an error".into() } else { answer });
    }
    if answer.is_empty() {
        return Err("claude answered with nothing".into());
    }
    Ok((answer, session))
}

/// The glance goes next to the cursor and the whole answer goes to the
/// clipboard, announced by the dot. Line one is the answer; the rest is there
/// for when line one says it is needed.
#[cfg(windows)]
fn deliver(app: &AppHandle, answer: &str, small: bool) {
    use tauri_plugin_clipboard_manager::ClipboardExt;

    let (text, rest) = glance(answer, if small { 80 } else { 40 });
    if let Some(rest) = rest.clone() {
        let _ = app.clipboard().write_text(rest);
    }
    show_tip(app, Label { text, small, dot: rest.is_some() }, None, 10);
}

/// `follow_up` is a line the user typed, which continues the conversation the
/// screenshot started; `None` asks the standing question about the screen.
#[cfg(windows)]
fn ask_claude(app: &AppHandle, follow_up: Option<String>) {
    // Claim the slot before anything is drawn, so a press during the capture
    // cancels instead of starting a second query.
    *TIP.lock().unwrap() = Some(Tip { detail: None });
    let generation = TIP_GEN.fetch_add(1, Ordering::SeqCst) + 1;
    let cancelled = move || TIP_GEN.load(Ordering::SeqCst) != generation;

    let app = app.clone();
    std::thread::spawn(move || {
        let typed = follow_up.is_some();

        // Nothing is drawn while the query is out. A placeholder would be text
        // the user has to read and discard, so the waiting shows in the cursor
        // instead, which costs no pixels and is already where they are looking.
        busy_cursor(true);
        let (answered, dirs) = match follow_up {
            // No second capture: Claude still has the first one in the session,
            // and a screenshot of the label being typed into is not the screen.
            Some(line) => {
                let (resume, dirs) = SESSION.lock().unwrap().clone().unzip();
                let dirs = dirs.unwrap_or_default();
                (run_claude(&line, None, resume.as_deref(), &dirs), dirs)
            }
            None => match current_prompt(&app) {
                Some(prompt) => {
                    let dirs = prompt_dirs(&prompt, Path::is_dir);
                    let shot = screenshot();
                    if cancelled() {
                        busy_cursor(false);
                        return;
                    }
                    (run_claude(&prompt, shot.as_deref(), None, &dirs), dirs)
                }
                None => (Err("no snippet named Current-Prompt".into()), Vec::new()),
            },
        };
        busy_cursor(false);
        if cancelled() {
            return; // dismissed while waiting: never to be seen again
        }

        match answered {
            Ok((answer, session)) => {
                if let Some(id) = session {
                    *SESSION.lock().unwrap() = Some((id, dirs));
                }
                deliver(&app, &answer, typed);
            }
            Err(reason) => show_tip(&app, Label::err(), Some(reason), 10),
        }
    });
}

// --- Typing a follow-up ----------------------------------------------------

/// How the keyboard hook reports the state of the line being typed.
const TYPING: u8 = 0;
const SEND: u8 = 1;
const CANCEL: u8 = 2;

/// What the hook is collecting. `None` whenever no line is open, and the hook
/// passes every key straight through while it reads `None`.
#[cfg(windows)]
static TYPED: Mutex<Option<String>> = Mutex::new(None);
#[cfg(windows)]
static TYPED_END: AtomicU8 = AtomicU8::new(TYPING);

/// Whether the hotkey's letter is still down `wait` later. Win+C, Win+Alt+C and
/// Ctrl+Alt+V all end in a letter, and the letter is what tells a tap from a
/// hold: the modifiers stay down for a moment after any ordinary press.
#[cfg(windows)]
fn held(wait: Duration) -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;

    let down = || {
        ['C', 'V']
            .iter()
            .any(|key| unsafe { GetAsyncKeyState(*key as i32) } as u16 & 0x8000 != 0)
    };
    let deadline = Instant::now() + wait;
    while Instant::now() < deadline {
        if !down() {
            return false;
        }
        std::thread::sleep(Duration::from_millis(16));
    }
    down()
}

/// The character a key produces on the layout the window in front is using.
///
/// ponytail: reads Shift with `GetAsyncKeyState` and ignores Caps Lock and dead
/// keys — toggle and dead-key state belong to a thread's own input queue, and
/// ours is not the one being typed into. Shift is what a question is typed with.
#[cfg(windows)]
unsafe fn typed_char(vk: u16, scan: u32) -> Option<char> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, GetKeyboardLayout, ToUnicodeEx, VK_CONTROL, VK_MENU, VK_SHIFT,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId,
    };

    let down = |vk: u16| GetAsyncKeyState(vk as i32) as u16 & 0x8000 != 0;
    // Ctrl on its own makes a shortcut, not a character. AltGr is Ctrl and Alt
    // together, which does make one.
    if down(VK_CONTROL) && !down(VK_MENU) {
        return None;
    }

    let mut state = [0u8; 256];
    for modifier in [VK_SHIFT, VK_CONTROL, VK_MENU] {
        if down(modifier) {
            state[modifier as usize] = 0x80;
        }
    }

    let layout =
        GetKeyboardLayout(GetWindowThreadProcessId(GetForegroundWindow(), std::ptr::null_mut()));
    let mut out = [0u16; 4];
    // Bit 2 keeps the call from disturbing the kernel's own dead-key state.
    let count = ToUnicodeEx(
        vk as u32,
        scan,
        state.as_ptr(),
        out.as_mut_ptr(),
        out.len() as i32,
        4,
        layout,
    );
    if count <= 0 {
        return None;
    }
    char::decode_utf16(out[..count as usize].iter().copied())
        .next()?
        .ok()
        .filter(|ch| !ch.is_control())
}

#[cfg(windows)]
unsafe extern "system" fn typing_hook(code: i32, wparam: usize, lparam: isize) -> isize {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        VK_BACK, VK_CAPITAL, VK_CONTROL, VK_ESCAPE, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN,
        VK_MENU, VK_RCONTROL, VK_RETURN, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SHIFT,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, HC_ACTION, KBDLLHOOKSTRUCT, WM_KEYDOWN, WM_SYSKEYDOWN,
    };

    let pass = || CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam);
    if code != HC_ACTION as i32 {
        return pass();
    }
    let key = &*(lparam as *const KBDLLHOOKSTRUCT);
    let vk = key.vkCode as u16;

    // Modifiers go through untouched. Eating a keyup leaves whatever is in
    // front of us believing the key is still held down, which is a worse thing
    // to do to an app than letting a bare Shift reach it.
    if matches!(
        vk,
        VK_SHIFT
            | VK_LSHIFT
            | VK_RSHIFT
            | VK_CONTROL
            | VK_LCONTROL
            | VK_RCONTROL
            | VK_MENU
            | VK_LMENU
            | VK_RMENU
            | VK_LWIN
            | VK_RWIN
            | VK_CAPITAL
    ) {
        return pass();
    }

    let mut typed = TYPED.lock().unwrap();
    let Some(line) = typed.as_mut() else {
        return pass();
    };

    if wparam as u32 == WM_KEYDOWN || wparam as u32 == WM_SYSKEYDOWN {
        match vk {
            VK_RETURN => TYPED_END.store(SEND, Ordering::SeqCst),
            VK_ESCAPE => TYPED_END.store(CANCEL, Ordering::SeqCst),
            VK_BACK => {
                line.pop();
            }
            _ => line.extend(typed_char(vk, key.scanCode)),
        }
    }
    // Swallowed: these keystrokes are the question, not input for the window
    // behind us, which would otherwise be taking them into a spreadsheet cell.
    1
}

/// Hold the hotkey and the label becomes a line to type into. The keys come off
/// the machine for as long as it is open: an unfocused window is given no
/// keyboard, and the alternative is typing the question into whatever is in
/// front. Enter sends it, Esc drops it, and so does another press of the hotkey.
#[cfg(windows)]
fn compose(app: &AppHandle) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        PeekMessageW, SetWindowsHookExW, UnhookWindowsHookEx, MSG, PM_REMOVE, WH_KEYBOARD_LL,
    };

    *TYPED.lock().unwrap() = Some(String::new());
    TYPED_END.store(TYPING, Ordering::SeqCst);
    // The three dots are back, but only here, where they mean "type" and not
    // "thinking". An hour is just a ceiling; the idle timer below ends it.
    show_tip(app, Label::typing(""), None, 3600);
    let generation = TIP_GEN.load(Ordering::SeqCst);

    let app = app.clone();
    std::thread::spawn(move || unsafe {
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(typing_hook), std::ptr::null_mut(), 0);
        if hook.is_null() {
            *TYPED.lock().unwrap() = None;
            hide_tip(&app);
            return;
        }

        let mut shown = String::new();
        let mut last_key = Instant::now();
        let ended = loop {
            // A low-level hook only fires while the thread that set it is
            // retrieving messages, so this loop is what keeps it alive.
            let mut msg: MSG = std::mem::zeroed();
            while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {}

            let ended = TYPED_END.load(Ordering::SeqCst);
            if ended != TYPING {
                break ended;
            }
            // Another press, or the mode switched off underneath us.
            if TIP_GEN.load(Ordering::SeqCst) != generation {
                break CANCEL;
            }

            let line = TYPED.lock().unwrap().clone().unwrap_or_default();
            if line != shown {
                shown = line;
                last_key = Instant::now();
                emit_tip(&app, &Label::typing(&shown));
            }
            // A hook that swallows every key on the machine is not a thing to
            // leave up on a window someone walked away from.
            if last_key.elapsed() > Duration::from_secs(60) {
                break CANCEL;
            }
            std::thread::sleep(Duration::from_millis(10));
        };

        UnhookWindowsHookEx(hook);
        let line = TYPED.lock().unwrap().take().unwrap_or_default();
        if ended == SEND && !line.trim().is_empty() {
            ask_claude(&app, Some(line));
        } else {
            hide_tip(&app);
        }
    });
}

/// A tap clears whatever is on screen, or asks a new question when there is
/// nothing to clear. Holding it opens a line to type into instead. Which of the
/// two it was is only known a moment after the press, so the press does its
/// clearing straight away and the rest waits on the key coming back up.
#[cfg(windows)]
fn claude_hotkey(app: &AppHandle) {
    // A line already open ends on the press, the way everything else does.
    if TYPED.lock().unwrap().is_some() {
        TYPED_END.store(CANCEL, Ordering::SeqCst);
        return;
    }

    let showing = TIP.lock().unwrap().take();
    let idle = showing.is_none();
    match showing {
        // On (Err) the press trades the marker for the reason behind it.
        Some(Tip { detail: Some(reason) }) => show_tip(app, Label::plain(&reason), None, 10),
        Some(Tip { detail: None }) => hide_tip(app),
        // Claim the slot so a second press can cancel this one before it starts.
        None => *TIP.lock().unwrap() = Some(Tip { detail: None }),
    }

    let generation = TIP_GEN.load(Ordering::SeqCst);
    let app = app.clone();
    std::thread::spawn(move || {
        let hold = held(Duration::from_millis(900));
        if TIP_GEN.load(Ordering::SeqCst) != generation {
            return; // pressed again while we were waiting on the key
        }
        if hold {
            compose(&app);
        } else if idle {
            ask_claude(&app, None);
        }
    });
}

/// What the hotkey does, whichever route it arrived by.
fn on_hotkey(app: &AppHandle) {
    #[cfg(windows)]
    if CLAUDE_MODE.load(Ordering::Relaxed) {
        claude_hotkey(app);
        return;
    }
    toggle(app);
}

fn toggle(app: &AppHandle) {
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    if win.is_visible().unwrap_or(false) {
        let _ = win.hide();
    } else {
        let _ = show_near_cursor(&win);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Must be first. Also lets an external launcher (an AutoHotkey script
        // bound to a key the shell will not release) toggle us by re-running
        // the exe instead of starting a second copy.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            on_hotkey(app);
        }))
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .invoke_handler(tauri::generate_handler![load_clips, save_clips, fetch_usage])
        .setup(|app| {
            let handle = app.handle();

            // Opt in on first run only. Enabling every launch would silently
            // undo the user turning it off.
            if !store_path(handle).exists() {
                let _ = handle.autolaunch().enable();
            }
            let autostart_on = handle.autolaunch().is_enabled().unwrap_or(false);

            let mut taken: Vec<String> = Vec::new();
            for hotkey in HOTKEYS {
                let registered = handle.global_shortcut().on_shortcut(hotkey, |app, _, event| {
                    if event.state() == ShortcutState::Pressed {
                        on_hotkey(app);
                    }
                });
                if registered.is_ok() {
                    taken.push(hotkey.replace("Key", "").replace("Backquote", "`"));
                } else {
                    eprintln!("hotkey {hotkey} unavailable");
                }
            }
            // Which ones took is not guessable from outside, so surface it.
            let label = if taken.is_empty() {
                "no hotkey".to_string()
            } else {
                taken.join("  ")
            };
            eprintln!("hotkeys active: {label}");

            let show = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
            let startup = CheckMenuItem::with_id(
                app,
                "autostart",
                "Start with Windows",
                true,
                autostart_on,
                None::<&str>,
            )?;
            #[cfg(windows)]
            let copilot = CheckMenuItem::with_id(
                app,
                "copilot",
                "Disable Windows Copilot",
                true,
                copilot_disabled(),
                None::<&str>,
            )?;
            #[cfg(windows)]
            let claude = CheckMenuItem::with_id(
                app,
                "claude_mode",
                "Claude Mode",
                true,
                false,
                None::<&str>,
            )?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

            let startup_item = startup.clone();
            #[cfg(windows)]
            let copilot_item = copilot.clone();
            #[cfg(windows)]
            let claude_item = claude.clone();
            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip(format!("Clipboard Splash  ({label})"))
                .menu(&Menu::with_items(
                    app,
                    &[&show, &claude, &startup, &copilot, &quit],
                )?)
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "show" => toggle(app),
                    "autostart" => {
                        let launcher = app.autolaunch();
                        let on = launcher.is_enabled().unwrap_or(false);
                        let _ = if on { launcher.disable() } else { launcher.enable() };
                        let _ = startup_item.set_checked(!on);
                    }
                    #[cfg(windows)]
                    "copilot" => {
                        // UAC blocks, so get off the menu thread or the tray hangs.
                        let item = copilot_item.clone();
                        std::thread::spawn(move || {
                            let wanted = !copilot_disabled();
                            set_copilot_disabled(wanted);
                            let _ = item.set_checked(copilot_disabled());
                        });
                    }
                    #[cfg(windows)]
                    "claude_mode" => {
                        let on = !CLAUDE_MODE.load(Ordering::Relaxed);
                        CLAUDE_MODE.store(on, Ordering::Relaxed);
                        let _ = claude_item.set_checked(on);
                        // Leaving the mode should not strand a label on screen,
                        // and least of all a keyboard hook eating every key.
                        if !on {
                            TYPED_END.store(CANCEL, Ordering::SeqCst);
                            hide_tip(app);
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        toggle(tray.app_handle());
                    }
                })
                .build(app)?;

            if let Some(win) = app.get_webview_window("main") {
                let hide_target = win.clone();
                win.on_window_event(move |event| match event {
                    // No titlebar to close it with, but a stray close must not kill the tray.
                    tauri::WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        let _ = hide_target.hide();
                    }
                    // Dismiss on click-away, but not in dev or debugging is
                    // impossible. The watcher below covers the same ground; this
                    // is here because it is instant where a poll has a tick of lag.
                    #[cfg(not(debug_assertions))]
                    tauri::WindowEvent::Focused(false) => {
                        let _ = hide_target.hide();
                    }
                    _ => {}
                });

                #[cfg(all(windows, not(debug_assertions)))]
                dismiss_on_click_away(win.clone());
            }

            #[cfg(windows)]
            if let Some(win) = app.get_webview_window("tip") {
                bare_window(&win);
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::{find_current_prompt, glance, one_line, prompt_dirs, Label};
    use std::path::{Path, PathBuf};

    #[test]
    fn a_prompt_grants_the_real_folders_it_names() {
        let real = ["C:\\Notes\\Parcial I", "D:/work"];
        let is_dir = |path: &Path| real.iter().any(|r| Path::new(r) == path);
        let prompt = "Working folder: C:\\Notes\\Parcial I\\\n\
                      Also D:/work, then C:\\Notes\\Parcial I\\SKILL.md again.\n\
                      Not C:\\ alone, not E:\\gone, not a URL like https://x";
        assert_eq!(
            prompt_dirs(prompt, is_dir),
            [PathBuf::from("C:\\Notes\\Parcial I"), PathBuf::from("D:/work")]
        );
        assert!(prompt_dirs("what is on screen?", is_dir).is_empty());
    }

    const CLIPS: &str = r#"{"folders":[
        {"name":"Paths","items":[{"label":"QuickTools","text":"C:/qt"}]},
        {"name":"Prompts","items":[
            {"label":" current-prompt ","text":"what is on screen?"},
            {"label":"Current-Prompt","text":"second one, ignored"}]}]}"#;

    #[test]
    fn finds_the_prompt_in_any_folder_whatever_its_case() {
        assert_eq!(
            find_current_prompt(CLIPS).as_deref(),
            Some("what is on screen?")
        );
        assert_eq!(find_current_prompt(r#"{"folders":[]}"#), None);
        assert_eq!(find_current_prompt("not json"), None);
    }

    #[test]
    fn label_is_one_line_of_forty_characters() {
        assert_eq!(one_line("  a\n\tb  c ", 40), "a b c");
        assert_eq!(one_line(&"x".repeat(100), 40).len(), 40);
        // Counted in characters, not bytes, or this would panic or truncate mid-glyph.
        assert_eq!(one_line(&"é".repeat(100), 40).chars().count(), 40);
    }

    #[test]
    fn the_screen_gets_line_one_and_the_clipboard_gets_the_rest() {
        // Short enough to fit whole: nothing to copy, so no dot and no clobbered
        // clipboard.
        assert_eq!(glance("Lock B2 as $B$2", 40), ("Lock B2 as $B$2".into(), None));

        // Anything held back puts the whole answer on the clipboard, not just
        // the tail, so pasting it gives something that reads on its own.
        let answer = "Wrong sheet - see clipboard\n\nThe formula points at Q3.";
        let (head, rest) = glance(answer, 40);
        assert_eq!(head, "Wrong sheet - see clipboard");
        assert_eq!(rest.as_deref(), Some(answer));

        // A first line too long to show still counts as held back.
        let (head, rest) = glance(&"y".repeat(60), 40);
        assert_eq!(head.chars().count(), 40);
        assert!(rest.is_some());
    }

    #[test]
    fn a_typed_line_shows_its_end() {
        assert_eq!(Label::typing("").text, "…");
        let long: String = ('a'..='z').cycle().take(200).collect();
        let shown = Label::typing(&long).text;
        assert_eq!(shown.chars().count(), 90);
        assert!(long.ends_with(&shown));
    }
}
