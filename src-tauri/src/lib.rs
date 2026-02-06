mod config;
mod keyboard_hook;
mod touchpad_monitor;

use crate::config::AppConfig;
use crate::keyboard_hook::{KeyboardHook, IS_PAUSED};
use crate::touchpad_monitor::TouchpadMonitor;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
#[cfg(target_os = "windows")]
use std::process::Command;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};
#[cfg(target_os = "windows")]
use winreg::enums::*;
#[cfg(target_os = "windows")]
use winreg::RegKey;

static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();
static LOG_TX: OnceLock<Sender<String>> = OnceLock::new();
static PAUSE_MENU_ITEM: OnceLock<CheckMenuItem<tauri::Wry>> = OnceLock::new();

#[cfg(target_os = "windows")]
pub fn is_elevated() -> bool {
    std::fs::metadata("C:\\Windows\\System32\\config\\SAM").is_ok()
}

#[cfg(target_os = "linux")]
pub fn is_elevated() -> bool {
    nix::unistd::Uid::effective().is_root()
}

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
        "[SYSTEM] Saving config: Sens={}, Invert={}, Mappings={}",
        new_config.scroll_sensitivity,
        new_config.scroll_invert,
        new_config.mappings.len()
    ));
    *config.write().unwrap() = new_config.clone();

    // モニター遅延を同期
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
    is_elevated()
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
        let _ = item.set_checked(paused);
    }
    let _ = app.emit("pause-status", paused).ok();
    audit_log(&format!("[SYSTEM] Pause state changed to: {}", paused));
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn set_startup_cmd(enabled: bool) -> Result<(), String> {
    audit_log(&format!("[SYSTEM] Startup toggle requested: {}", enabled));

    // 1. レジストリのクリーンアップ (古い方式の削除)
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let reg_path = r"Software\Microsoft\Windows\CurrentVersion\Run";
    if let Ok(key) = hkcu.open_subkey_with_flags(reg_path, winreg::enums::KEY_SET_VALUE) {
        let _ = key.delete_value("YATS");
    }

    // 2. スタートアップフォルダへのショートカット処理
    // %APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup
    let app_data = std::env::var("APPDATA").map_err(|e| e.to_string())?;
    let mut startup_path = std::path::PathBuf::from(app_data);
    startup_path.push(r"Microsoft\Windows\Start Menu\Programs\Startup");
    startup_path.push("YATS.lnk");

    if enabled {
        let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
        let exe_str = exe_path.to_str().ok_or("Invalid exe path")?;
        let lnk_str = startup_path.to_str().ok_or("Invalid shortcut path")?;

        // PowerShellを使用してショートカットを作成
        let script = format!(
            "$s=(New-Object -ComObject WScript.Shell).CreateShortcut('{}');$s.TargetPath='{}';$s.Save()",
            lnk_str, exe_str
        );

        use std::os::windows::process::CommandExt;
        Command::new("powershell")
            .args(&["-Command", &script])
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .status()
            .map_err(|e| e.to_string())?;

        audit_log("[SYSTEM] Startup shortcut created.");
    } else {
        if startup_path.exists() {
            std::fs::remove_file(startup_path).map_err(|e| e.to_string())?;
            audit_log("[SYSTEM] Startup shortcut removed.");
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
#[tauri::command]
fn set_startup_cmd(enabled: bool) -> Result<(), String> {
    audit_log(&format!(
        "[SYSTEM] Startup toggle requested (Linux): {}",
        enabled
    ));

    let home = std::env::var("HOME").map_err(|e| e.to_string())?;
    let mut autostart_dir = std::path::PathBuf::from(home);
    autostart_dir.push(".config");
    autostart_dir.push("autostart");

    if !autostart_dir.exists() {
        std::fs::create_dir_all(&autostart_dir).map_err(|e| e.to_string())?;
    }

    let mut desktop_file = autostart_dir.clone();
    desktop_file.push("yats.desktop");

    if enabled {
        let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
        let content = format!(
            "[Desktop Entry]\nType=Application\nName=YATS\nExec={}\nHidden=false\nNoDisplay=false\nX-GNOME-Autostart-enabled=true\n",
            exe_path.to_string_lossy()
        );
        std::fs::write(&desktop_file, content).map_err(|e| e.to_string())?;
        audit_log("[SYSTEM] Startup .desktop file created.");
    } else {
        if desktop_file.exists() {
            std::fs::remove_file(desktop_file).map_err(|e| e.to_string())?;
            audit_log("[SYSTEM] Startup .desktop file removed.");
        }
    }
    Ok(())
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn get_startup_status_cmd() -> bool {
    let app_data = match std::env::var("APPDATA") {
        Ok(v) => v,
        Err(_) => return false,
    };
    let mut startup_path = std::path::PathBuf::from(app_data);
    startup_path.push(r"Microsoft\Windows\Start Menu\Programs\Startup");
    startup_path.push("YATS.lnk");

    startup_path.exists()
}

#[cfg(target_os = "linux")]
#[tauri::command]
fn get_startup_status_cmd() -> bool {
    let home = match std::env::var("HOME") {
        Ok(v) => v,
        Err(_) => return false,
    };
    let mut desktop_file = std::path::PathBuf::from(home);
    desktop_file.push(".config");
    desktop_file.push("autostart");
    desktop_file.push("yats.desktop");

    desktop_file.exists()
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn get_aap_threshold() -> Result<i32, String> {
    let output = Command::new("reg")
        .args(&[
            "query",
            "HKEY_CURRENT_USER\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\PrecisionTouchPad",
            "/v",
            "AAPThreshold",
        ])
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Ok(2);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if line.contains("AAPThreshold") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                let val_str = parts[parts.len() - 1];
                if val_str.starts_with("0x") {
                    if let Ok(val) = i32::from_str_radix(&val_str[2..], 16) {
                        return Ok(val);
                    }
                }
            }
        }
    }
    Ok(2)
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn set_aap_threshold(value: i32) -> Result<(), String> {
    audit_log(&format!(
        "[SYSTEM] Changing AAPThreshold Registry to {}",
        value
    ));
    Command::new("reg")
        .args(&[
            "add",
            "HKEY_CURRENT_USER\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\PrecisionTouchPad",
            "/v",
            "AAPThreshold",
            "/t",
            "REG_DWORD",
            "/d",
            &value.to_string(),
            "/f",
        ])
        .status()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn deep_registry_clean_cmd() -> Result<String, String> {
    if !is_elevated() {
        return Err("管理者権限が必要です。".to_string());
    }
    let mut results = Vec::new();
    let sync_keys = [
        (
            "HKEY_LOCAL_MACHINE\\SOFTWARE\\Synaptics\\SynTP\\Defaults",
            "PalmKms",
        ),
        (
            "HKEY_LOCAL_MACHINE\\SOFTWARE\\Synaptics\\SynTP\\Defaults",
            "SimultaneousTimeThreshold",
        ),
    ];
    for (path, val) in sync_keys {
        let _ = Command::new("reg")
            .args(&["add", path, "/v", val, "/t", "REG_DWORD", "/d", "0", "/f"])
            .status();
        results.push(format!("Synced {}", val));
    }
    let elan_path = "HKEY_LOCAL_MACHINE\\SYSTEM\\CurrentControlSet\\Control\\Elantech\\SmartPad";
    let _ = Command::new("reg")
        .args(&[
            "add",
            elan_path,
            "/v",
            "DisableWhenType_Enable",
            "/t",
            "REG_DWORD",
            "/d",
            "0",
            "/f",
        ])
        .status();
    results.push("Synced ELAN SmartPad".to_string());
    audit_log(&format!(
        "[SYSTEM] Deep Registry Clean performed: {:?}",
        results
    ));
    Ok(results.join(", "))
}

#[cfg(target_os = "linux")]
#[tauri::command]
fn get_aap_threshold() -> Result<i32, String> {
    Ok(0)
}
#[cfg(target_os = "linux")]
#[tauri::command]
fn set_aap_threshold(_value: i32) -> Result<(), String> {
    Ok(())
}
#[cfg(target_os = "linux")]
#[tauri::command]
fn deep_registry_clean_cmd() -> Result<String, String> {
    Ok("Not supported on Linux".to_string())
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
            deep_registry_clean_cmd
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

            // 初期ロードからモニター遅延を同期
            monitor_clone
                .state
                .release_delay_ms
                .store(loaded_config.release_delay_ms as u32, Ordering::SeqCst);

            *shared_handle.lock().unwrap() = Some(app.handle().clone());

            let quit_i = MenuItem::with_id(app, "quit", "終了", true, None::<&str>)?;
            let settings_i = MenuItem::with_id(app, "settings", "設定", true, None::<&str>)?;
            let pause_i =
                CheckMenuItem::with_id(app, "pause", "機能の一時停止", true, false, None::<&str>)?;
            let menu = Menu::with_items(app, &[&settings_i, &pause_i, &quit_i])?;

            // 同期用にハンドルを保存
            let _ = PAUSE_MENU_ITEM.set(pause_i.clone());

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("YATS")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| {
                    match event.id.as_ref() {
                        "quit" => app.exit(0),
                        "settings" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show().ok();
                                let _ = window.set_focus().ok();
                            }
                        }
                        "pause" => {
                            let is_checked = IS_PAUSED.load(Ordering::SeqCst);
                            let new_state = !is_checked;
                            IS_PAUSED.store(new_state, Ordering::SeqCst);

                            // 同期: メニュー項目のチェックマークを視覚的に更新
                            if let Some(item) = PAUSE_MENU_ITEM.get() {
                                let _ = item.set_checked(new_state);
                            }

                            let _ = app.emit("pause-status", new_state).ok();
                            audit_log(&format!("[TRAY] Pause toggled to: {}", new_state));
                        }
                        _ => {}
                    }
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

            // 起動時に自動スキャン
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
