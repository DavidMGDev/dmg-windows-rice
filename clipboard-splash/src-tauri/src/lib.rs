use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewWindow};
use tauri_plugin_autostart::{ManagerExt, MacosLauncher};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

/// Tried in order; the first one Windows will hand us wins.
/// Super+Shift+V mirrors Win+V (which the shell owns and will not release).
/// The rest are fallbacks for when another app already holds the primary.
const HOTKEYS: [&str; 3] = ["Super+Shift+KeyV", "Ctrl+Alt+KeyV", "Ctrl+Shift+Backquote"];

/// Gap between the cursor and the panel, in physical pixels.
const GAP: i32 = 14;

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
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .invoke_handler(tauri::generate_handler![load_clips, save_clips, fetch_usage])
        .setup(|app| {
            let handle = app.handle();
            let _ = handle.autolaunch().enable();

            for hotkey in HOTKEYS {
                let registered = handle.global_shortcut().on_shortcut(hotkey, |app, _, event| {
                    if event.state() == ShortcutState::Pressed {
                        toggle(app);
                    }
                });
                if registered.is_ok() {
                    break;
                }
                eprintln!("hotkey {hotkey} unavailable, trying next");
            }

            let show = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Clipboard Splash")
                .menu(&Menu::with_items(app, &[&show, &quit])?)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => toggle(app),
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
