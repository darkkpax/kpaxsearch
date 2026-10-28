#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Mutex,
    },
};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt as AutostartExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

#[derive(Clone, Serialize, Deserialize, Debug)]
struct Program {
    id: String,
    name: String,
    #[serde(default)]
    icon: String,
}
#[derive(Default)]
struct Catalog {
    programs: Mutex<Vec<Program>>,
    shortcut: Mutex<String>,
    refresh: Mutex<()>,
    radius: AtomicU32,
    updating: AtomicBool,
}

fn cache_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_local_data_dir()
        .map(|p| p.join("programs.json"))
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn cached_programs(state: State<Catalog>) -> Vec<Program> {
    state.programs.lock().unwrap().clone()
}

#[tauri::command]
fn shortcut_label(state: State<Catalog>) -> String {
    state.shortcut.lock().unwrap().clone()
}

#[tauri::command]
async fn refresh_programs(app: tauri::AppHandle) -> Result<Vec<Program>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        use std::os::windows::process::CommandExt;
        let state = app.state::<Catalog>();
        let _refresh = state.refresh.lock().unwrap();
        let output = Command::new("powershell.exe")
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                include_str!("catalog.ps1"),
            ])
            .creation_flags(0x08000000)
            .output()
            .map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err(format!(
                "windows не вернула список программ: {}",
                String::from_utf8_lossy(&output.stderr)
                    .chars()
                    .take(240)
                    .collect::<String>()
            ));
        }
        let programs: Vec<Program> = serde_json::from_slice(&output.stdout)
            .map_err(|e| format!("не удалось прочитать список программ: {e}"))?;
        if programs.is_empty() {
            return Err("windows вернула пустой список программ; попробуй обновить ещё раз".into());
        }
        *state.programs.lock().unwrap() = programs.clone();
        let path = cache_path(&app)?;
        fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        let temp = path.with_extension("tmp");
        fs::write(
            &temp,
            serde_json::to_vec(&programs).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        fs::rename(&temp, &path)
            .map_err(|e| format!("программы найдены, но кэш не сохранён: {e}"))?;
        Ok(programs)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn known_program<'a>(programs: &'a [Program], id: &str) -> Result<&'a Program, String> {
    programs
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| "программа больше не в списке; обнови поиск".into())
}

#[tauri::command]
fn launch_program(
    window: tauri::WebviewWindow,
    state: State<Catalog>,
    id: String,
) -> Result<(), String> {
    let programs = state.programs.lock().unwrap();
    let program = known_program(&programs, &id)?;
    // ShellExecute handles AppsFolder IDs directly, without a command shell or interpolated script.
    #[link(name = "shell32")]
    extern "system" {
        fn ShellExecuteW(
            hwnd: isize,
            operation: *const u16,
            file: *const u16,
            parameters: *const u16,
            directory: *const u16,
            show: i32,
        ) -> isize;
    }
    let target: Vec<u16> = format!("shell:AppsFolder\\{}", program.id)
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let result = unsafe {
        ShellExecuteW(
            0,
            std::ptr::null(),
            target.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
        )
    };
    if result <= 32 {
        return Err(format!(
            "не удалось открыть программу (код {result}); обнови список"
        ));
    }
    let _ = window.hide();
    Ok(())
}

fn reveal(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
        let _ = window.emit("launcher-open", ());
    }
}

fn configure_frame(window: &tauri::WebviewWindow) {
    #[link(name = "dwmapi")]
    extern "system" {
        fn DwmSetWindowAttribute(hwnd: isize, attribute: u32, value: *const i32, size: u32) -> i32;
    }
    if let Ok(hwnd) = window.hwnd() {
        unsafe {
            DwmSetWindowAttribute(hwnd.0 as isize, 33, &1, 4);
            // DWMWA_COLOR_NONE removes the extra system outline, retaining the shadow.
            DwmSetWindowAttribute(hwnd.0 as isize, 34, &-2, 4);
        }
    }
}

fn round_window(window: &tauri::Window, radius: u32) -> Result<(), String> {
    #[link(name = "gdi32")]
    extern "system" {
        fn CreateRoundRectRgn(
            left: i32,
            top: i32,
            right: i32,
            bottom: i32,
            width: i32,
            height: i32,
        ) -> isize;
        fn DeleteObject(object: isize) -> i32;
    }
    #[link(name = "user32")]
    extern "system" {
        fn SetWindowRgn(hwnd: isize, region: isize, redraw: i32) -> i32;
    }
    let hwnd = window.hwnd().map_err(|e| e.to_string())?;
    let size = window.outer_size().map_err(|e| e.to_string())?;
    let diameter =
        (radius.min(40) as f64 * 2. * window.scale_factor().map_err(|e| e.to_string())?).round()
            as i32;
    unsafe {
        let region = CreateRoundRectRgn(
            0,
            0,
            size.width as i32 + 1,
            size.height as i32 + 1,
            diameter,
            diameter,
        );
        if region == 0 {
            return Err("не удалось округлить окно".into());
        }
        if SetWindowRgn(hwnd.0 as isize, region, 1) == 0 {
            DeleteObject(region);
            return Err("не удалось применить округлость".into());
        }
    }
    Ok(())
}

#[tauri::command]
fn set_rounding(window: tauri::Window, state: State<Catalog>, radius: u32) -> Result<(), String> {
    state.radius.store(radius.min(40), Ordering::Relaxed);
    round_window(&window, radius)
}

#[tauri::command]
fn background_start() -> bool {
    std::env::args().any(|arg| arg == "--background" || arg == "--updated")
}

#[tauri::command]
fn reserve_update(state: State<Catalog>) -> bool {
    state
        .updating
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
}

#[tauri::command]
fn release_update(state: State<Catalog>) {
    state.updating.store(false, Ordering::Release);
}

#[tauri::command]
fn get_startup(app: tauri::AppHandle) -> Result<bool, String> {
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

#[tauri::command]
fn set_startup(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    if enabled {
        app.autolaunch().enable()
    } else {
        app.autolaunch().disable()
    }
    .map_err(|e| e.to_string())?;
    let path = app.path().app_local_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    fs::write(
        path.join("startup-enabled"),
        if enabled { "true" } else { "false" },
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn open_settings(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.hide();
    }
    if let Some(window) = app.get_webview_window("settings") {
        window.show().map_err(|e| e.to_string())?;
        return window.set_focus().map_err(|e| e.to_string());
    }
    let window = tauri::WebviewWindowBuilder::new(
        &app,
        "settings",
        tauri::WebviewUrl::App("index.html?settings".into()),
    )
    .title("настройки kpax search")
    .inner_size(420., 640.)
    .resizable(false)
    .decorations(false)
    .transparent(true)
    .shadow(true)
    .center()
    .visible(false)
    .build()
    .map_err(|e| e.to_string())?;
    configure_frame(&window);
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| reveal(app)))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--background"]),
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _, event| {
                    if event.state == ShortcutState::Pressed {
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false)
                                && window.is_focused().unwrap_or(false)
                            {
                                let _ = window.hide();
                            } else {
                                reveal(app);
                            }
                        }
                    }
                })
                .build(),
        )
        .manage(Catalog::default())
        .setup(|app| {
            let state = app.state::<Catalog>();
            state.radius.store(16, Ordering::Relaxed);
            if !cfg!(debug_assertions) {
                let enabled = app
                    .path()
                    .app_local_data_dir()
                    .ok()
                    .and_then(|p| fs::read_to_string(p.join("startup-enabled")).ok())
                    .map(|v| v != "false")
                    .unwrap_or(true);
                if enabled {
                    app.autolaunch().enable()?;
                }
            }
            if let Ok(path) = cache_path(app.handle()) {
                if let Ok(bytes) = fs::read(path) {
                    if let Ok(programs) = serde_json::from_slice(&bytes) {
                        *state.programs.lock().unwrap() = programs;
                    }
                }
            }
            let mut label = "открыть из трея";
            for (key, text) in [
                ("Alt+Space", "alt + пробел"),
                ("Ctrl+Alt+Space", "ctrl + alt + пробел"),
            ] {
                if app.global_shortcut().register(key).is_ok() {
                    label = text;
                    break;
                }
            }
            *state.shortcut.lock().unwrap() = label.into();
            let open = MenuItem::with_id(app, "open", "открыть поиск", true, None::<&str>)?;
            let settings = MenuItem::with_id(app, "settings", "настройки", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "выйти", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &settings, &quit])?;
            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip(format!("kpax search · {label}"))
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => reveal(app),
                    "settings" => {
                        let app = app.clone();
                        tauri::async_runtime::spawn(async move {
                            let _ = open_settings(app).await;
                        });
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if matches!(
                        event,
                        TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        }
                    ) {
                        let app = tray.app_handle().clone();
                        tauri::async_runtime::spawn(async move {
                            let _ = open_settings(app).await;
                        });
                    }
                })
                .build(app)?;
            let window = app.get_webview_window("main").unwrap();
            configure_frame(&window);
            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::ScaleFactorChanged { .. }) {
                let radius = window.state::<Catalog>().radius.load(Ordering::Relaxed);
                let _ = round_window(window, radius);
            }
            if window.label() != "main" {
                return;
            }
            match event {
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    let _ = window.hide();
                }
                tauri::WindowEvent::Focused(false) => {
                    let window = window.clone();
                    std::thread::spawn(move || {
                        // Theme changes and accessibility focus can briefly deactivate the window.
                        std::thread::sleep(std::time::Duration::from_millis(150));
                        if !window.is_focused().unwrap_or(false) {
                            let _ = window.hide();
                        }
                    });
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            cached_programs,
            refresh_programs,
            launch_program,
            shortcut_label,
            open_settings,
            set_rounding,
            background_start,
            get_startup,
            set_startup,
            reserve_update,
            release_update
        ])
        .run(tauri::generate_context!())
        .expect("не удалось запустить kpax search");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refuses_arbitrary_launch_targets() {
        let programs = vec![Program {
            id: "known".into(),
            name: "app".into(),
            icon: String::new(),
        }];
        assert!(known_program(&programs, "powershell.exe -Command anything").is_err());
        assert_eq!(known_program(&programs, "known").unwrap().name, "app");
    }
    #[test]
    fn malformed_cache_is_rejected() {
        assert!(serde_json::from_str::<Vec<Program>>(r#"[{"name":"missing id"}]"#).is_err());
    }
}
