use std::fs;
use std::path::PathBuf;
use std::time::Duration;

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

/// Win11 does not round undecorated windows on its own, and a CSS shadow would
/// be clipped by the window rect. Handing both to DWM keeps the shadow outside
/// the window where it cannot be cut off.
#[cfg(windows)]
fn round_corners(win: &WebviewWindow) {
    use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;
    const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
    const DWMWCP_ROUND: u32 = 2;

    if let Ok(hwnd) = win.hwnd() {
        let preference: u32 = DWMWCP_ROUND;
        unsafe {
            DwmSetWindowAttribute(
                hwnd.0 as _,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                std::ptr::addr_of!(preference).cast(),
                std::mem::size_of::<u32>() as u32,
            );
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

/// Places the panel next to the cursor, flipping and then clamping so it always
/// lands fully inside the monitor the cursor is on.
///
/// ponytail: clamps to full monitor bounds, not the work area, so a panel
/// pinned to the very bottom edge can sit under the taskbar. Swap in
/// `monitor.work_area()` if that ever actually bites.
fn show_near_cursor(win: &WebviewWindow) -> tauri::Result<()> {
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

    // Setting this in setup() does not survive to first paint, so re-apply it
    // here. Idempotent and a single cheap syscall.
    #[cfg(windows)]
    round_corners(win);

    win.show()?;
    win.set_focus()?;
    win.emit("shown", ())?;
    Ok(())
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
            toggle(app);
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
                        toggle(app);
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
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

            let startup_item = startup.clone();
            #[cfg(windows)]
            let copilot_item = copilot.clone();
            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip(format!("Clipboard Splash  ({label})"))
                .menu(&Menu::with_items(app, &[&show, &startup, &copilot, &quit])?)
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
                    // Dismiss on click-away, but not in dev or debugging is impossible.
                    #[cfg(not(debug_assertions))]
                    tauri::WindowEvent::Focused(false) => {
                        let _ = hide_target.hide();
                    }
                    _ => {}
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
