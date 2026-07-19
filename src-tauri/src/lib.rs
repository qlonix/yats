mod config;
mod keyboard_hook;
mod platform;
mod touchpad_monitor;

use crate::config::AppConfig;
use crate::keyboard_hook::{KeyboardHook, IS_PAUSED};
use crate::platform::SystemInfo;
use crate::touchpad_monitor::TouchpadMonitor;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};

static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();
static LOG_TX: OnceLock<Sender<String>> = OnceLock::new();
static PAUSE_MENU_ITEM: OnceLock<MenuItem<tauri::Wry>> = OnceLock::new();

pub fn audit_log(msg: &str) {
    if let Some(tx) = LOG_TX.get() {
        let _ = tx.send(msg.to_string()).ok();
    }
}

fn start_audit_worker() {
    let (tx, rx) = channel::<String>();
    let _ = LOG_TX.set(tx);

    std::thread::spawn(move || {
        let pid = std::process::id();
        while let Ok(msg) = rx.recv() {
            if let Some(path) = LOG_PATH.get() {
                if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
                    let _ = writeln!(
                        file,
                        "[{}] [PID:{}] {}",
                        chrono::Local::now().format("%H:%M:%S"),
                        pid,
                        msg
                    );
                }
            }
        }
    });
}

#[tauri::command]
fn get_config(config: tauri::State<'_, Arc<RwLock<AppConfig>>>) -> AppConfig {
    config.read().unwrap().clone()
}

#[tauri::command]
fn set_config(
    app: tauri::AppHandle,
    config: tauri::State<'_, Arc<RwLock<AppConfig>>>,
    monitor: tauri::State<'_, Arc<TouchpadMonitor>>,
    new_config: AppConfig,
) -> Result<(), String> {
    audit_log(&format!(
        "[SYSTEM] Saving config: Sens={}, Speed={}, Invert={}, Mappings={}",
        new_config.scroll_sensitivity,
        new_config.scroll_speed,
        new_config.scroll_invert,
        new_config.mappings.len()
    ));
    *config.write().unwrap() = new_config.clone();

    monitor
        .state
        .release_delay_ms
        .store(new_config.release_delay_ms as u32, Ordering::SeqCst);

    new_config.save(&app).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_touch_status(monitor: tauri::State<'_, Arc<TouchpadMonitor>>) -> bool {
    monitor.is_touched()
}

#[tauri::command]
fn refresh_inventory(monitor: tauri::State<'_, Arc<TouchpadMonitor>>) {
    monitor.scan_and_register();
}

#[tauri::command]
fn lock_device_cmd(monitor: tauri::State<'_, Arc<TouchpadMonitor>>, device_id: Option<usize>) {
    monitor.lock_device(device_id);
}

#[tauri::command]
fn check_elevation_cmd() -> bool {
    platform::Platform::is_elevated()
}

#[tauri::command]
fn clear_audit_log_cmd() {
    if let Some(path) = LOG_PATH.get() {
        let _ = std::fs::write(path, "");
        audit_log("[SYSTEM] Audit log cleared by user.");
    }
}

#[tauri::command]
fn open_log_folder_cmd() {
    if let Some(path) = LOG_PATH.get() {
        if let Some(parent) = path.parent() {
            let _ =
                tauri_plugin_opener::open_path(parent.to_string_lossy().to_string(), None::<&str>);
        }
    }
}

#[tauri::command]
fn get_log_path_cmd() -> String {
    LOG_PATH
        .get()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default()
}

#[tauri::command]
fn get_paused_cmd() -> bool {
    IS_PAUSED.load(Ordering::SeqCst)
}

#[tauri::command]
fn set_paused_cmd(app: tauri::AppHandle, paused: bool) {
    IS_PAUSED.store(paused, Ordering::SeqCst);
    if let Some(item) = PAUSE_MENU_ITEM.get() {
        let label = if paused {
            "▶ 機能を再開"
        } else {
            "⏸ 一時停止"
        };
        let _ = item.set_text(label);
    }
    let _ = app.emit("pause-status", paused).ok();
    audit_log(&format!("[SYSTEM] Pause state changed to: {}", paused));
}

// Startup functions using platform-specific implementation
#[tauri::command]
fn set_startup_cmd(enabled: bool) -> Result<(), String> {
    audit_log(&format!("[SYSTEM] Startup toggle requested: {}", enabled));
    platform::Platform::set_startup(enabled)
}

#[tauri::command]
fn get_startup_status_cmd() -> bool {
    platform::Platform::get_startup_status()
}

#[tauri::command]
fn get_aap_threshold() -> Result<i32, String> {
    platform::Platform::get_aap_threshold()
}

#[tauri::command]
fn set_aap_threshold(value: i32) -> Result<(), String> {
    audit_log(&format!("[SYSTEM] Changing AAPThreshold to {}", value));
    platform::Platform::set_aap_threshold(value)
}

#[tauri::command]
fn deep_registry_clean_cmd() -> Result<String, String> {
    if platform::Platform::is_elevated() {
        audit_log("[SYSTEM] Deep Registry Clean requested with elevation.");
    }
    platform::Platform::deep_registry_clean()
}

#[tauri::command]
fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let pid = std::process::id();
    let config = Arc::new(RwLock::new(AppConfig::default()));
    let config_clone = Arc::clone(&config);

    let shared_handle = Arc::new(Mutex::new(None));
    let shared_handle_clone = Arc::clone(&shared_handle);

    let monitor = Arc::new(TouchpadMonitor::new(Arc::clone(&shared_handle)));
    let monitor_clone = Arc::clone(&monitor);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let _ = app.get_webview_window("main").map(|w| {
                let _ = w.show();
                let _ = w.set_focus();
            });
        }))
        .manage(monitor)
        .manage(config.clone())
        .invoke_handler(tauri::generate_handler![
            get_config,
            set_config,
            get_touch_status,
            refresh_inventory,
            lock_device_cmd,
            check_elevation_cmd,
            clear_audit_log_cmd,
            open_log_folder_cmd,
            get_log_path_cmd,
            get_paused_cmd,
            set_paused_cmd,
            set_startup_cmd,
            get_startup_status_cmd,
            get_aap_threshold,
            set_aap_threshold,
            deep_registry_clean_cmd,
            get_app_version
        ])
        .setup(move |app| {
            let mut log_dir = app.path().app_log_dir().expect("Failed to get log dir");
            let _ = std::fs::create_dir_all(&log_dir);
            log_dir.push("hardware_audit.txt");
            let _ = LOG_PATH.set(log_dir);

            start_audit_worker();

            audit_log(&format!(
                "[SYSTEM] !!!!!!!! APPLICATION STARTING (PID: {}) !!!!!!!!",
                pid
            ));

            let loaded_config = {
                let mut config_write = config.write().unwrap();
                let cfg = AppConfig::load(app.handle().clone());
                *config_write = cfg.clone();
                cfg
            };

            monitor_clone
                .state
                .release_delay_ms
                .store(loaded_config.release_delay_ms as u32, Ordering::SeqCst);

            *shared_handle.lock().unwrap() = Some(app.handle().clone());

            let quit_i = MenuItem::with_id(app, "quit", "終了", true, None::<&str>)?;
            let settings_i = MenuItem::with_id(app, "settings", "設定", true, None::<&str>)?;
            let pause_i = MenuItem::with_id(app, "pause", "⏸ 一時停止", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&settings_i, &pause_i, &quit_i])?;

            let _ = PAUSE_MENU_ITEM.set(pause_i.clone());

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("YATS")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => app.exit(0),
                    "settings" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show().ok();
                            let _ = window.set_focus().ok();
                        }
                    }
                    "pause" => {
                        let is_paused = IS_PAUSED.load(Ordering::SeqCst);
                        let new_state = !is_paused;
                        IS_PAUSED.store(new_state, Ordering::SeqCst);

                        if let Some(item) = PAUSE_MENU_ITEM.get() {
                            let label = if new_state {
                                "▶ 機能を再開"
                            } else {
                                "⏸ 一時停止"
                            };
                            let _ = item.set_text(label);
                        }

                        let _ = app.emit("pause-status", new_state).ok();
                        audit_log(&format!("[TRAY] Pause toggled to: {}", new_state));
                    }
                    _ => {}
                })
                .on_tray_icon_event({
                    let app_handle = app.handle().clone();
                    move |_, event| {
                        if let TrayIconEvent::Click {
                            button: MouseButton::Left,
                            ..
                        } = event
                        {
                            if let Some(window) = app_handle.get_webview_window("main") {
                                let _ = window.show().ok();
                                let _ = window.set_focus().ok();
                            }
                        }
                    }
                })
                .build(app)?;

            monitor_clone.scan_and_register();

            std::thread::spawn(move || {
                let hook = KeyboardHook::new(monitor_clone, config_clone, shared_handle_clone);
                hook.start();
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide().ok();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
