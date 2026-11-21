// Windows-specific platform implementations

use crate::config::{MouseButton, WindowAction};
use std::process::Command;
use std::os::windows::process::CommandExt;
use std::ptr::null_mut;
use std::sync::Mutex;
use winapi::shared::windef::{HWND, POINT, RECT};
use winapi::um::winuser::*;
use winreg::enums::*;
use winreg::RegKey;

pub struct WindowsPlatform;

impl super::InputSimulation for WindowsPlatform {
    fn send_mouse_click(mb: &MouseButton, pressed: bool) {
        unsafe {
            let mut input: INPUT = std::mem::zeroed();
            input.type_ = INPUT_MOUSE;
            let mut mi: MOUSEINPUT = std::mem::zeroed();

            mi.dwFlags = match (mb, pressed) {
                (MouseButton::Left, true) => MOUSEEVENTF_LEFTDOWN,
                (MouseButton::Left, false) => MOUSEEVENTF_LEFTUP,
                (MouseButton::Right, true) => MOUSEEVENTF_RIGHTDOWN,
                (MouseButton::Right, false) => MOUSEEVENTF_RIGHTUP,
                (MouseButton::Middle, true) => MOUSEEVENTF_MIDDLEDOWN,
                (MouseButton::Middle, false) => MOUSEEVENTF_MIDDLEUP,
            };

            *input.u.mi_mut() = mi;
            SendInput(1, &mut input, std::mem::size_of::<INPUT>() as i32);
        }
    }

    fn send_key_click(vk: u16, pressed: bool) {
        unsafe {
            let mut input: INPUT = std::mem::zeroed();
            input.type_ = 1; // INPUT_KEYBOARD
            let mut ki: KEYBDINPUT = std::mem::zeroed();

            ki.wVk = vk;
            ki.dwFlags = if pressed { 0 } else { KEYEVENTF_KEYUP };
            ki.time = 0;
            ki.dwExtraInfo = 0xDEADBEEF;

            *input.u.ki_mut() = ki;
            SendInput(1, &mut input, std::mem::size_of::<INPUT>() as i32);
        }
    }

    fn send_mouse_scroll(delta: i32) {
        unsafe {
            let mut input: INPUT = std::mem::zeroed();
            input.type_ = INPUT_MOUSE;
            let mut mi: MOUSEINPUT = std::mem::zeroed();

            mi.dwFlags = MOUSEEVENTF_WHEEL;
            // delta は呼び出し側で WHEEL_DELTA (120 = 1 ノッチ) 単位に換算済み。
            // ノッチ未満の量もそのまま送ることで、細かいスクロールができる。
            mi.mouseData = delta as u32;

            *input.u.mi_mut() = mi;
            SendInput(1, &mut input, std::mem::size_of::<INPUT>() as i32);
        }
    }
}

impl super::WindowManagement for WindowsPlatform {
    fn execute_window_action(action: WindowAction) {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.is_null() {
                return;
            }

            // Window guard: exclude system shell windows
            let mut class_name = [0u16; 256];
            let len = GetClassNameW(hwnd, class_name.as_mut_ptr(), 256);
            if len > 0 {
                let name = String::from_utf16_lossy(&class_name[..len as usize]);
                if name == "Progman" || name == "WorkerW" || name == "Shell_TrayWnd" {
                    return;
                }
            }

            match action {
                WindowAction::Close => {
                    PostMessageW(hwnd, WM_SYSCOMMAND, SC_CLOSE, 0);
                }
                WindowAction::Minimize => {
                    ShowWindow(hwnd, SW_MINIMIZE);
                }
                WindowAction::Maximize => {
                    let is_maximized = IsZoomed(hwnd) != 0;
                    if is_maximized {
                        ShowWindow(hwnd, SW_RESTORE);
                    } else {
                        ShowWindow(hwnd, SW_MAXIMIZE);
                    }
                }
            }
        }
    }
}

impl super::SystemInfo for WindowsPlatform {
    fn is_elevated() -> bool {
        // Simple file system check for elevation
        std::fs::metadata("C:\\Windows\\System32\\config\\SAM").is_ok()
    }

    fn get_startup_status() -> bool {
        // Check shortcut in Startup folder (preferred method)
        if let Ok(app_data) = std::env::var("APPDATA") {
            let mut startup_path = std::path::PathBuf::from(app_data);
            startup_path.push(r"Microsoft\Windows\Start Menu\Programs\Startup");
            startup_path.push("YATS.lnk");
            if startup_path.exists() {
                return true;
            }
        }

        // Fallback to Registry Run key (legacy method)
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(run_key) = hkcu.open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Run") {
            run_key.get_value::<String, _>("YATS").is_ok()
        } else {
            false
        }
    }

    fn set_startup(enabled: bool) -> Result<(), String> {
        let app_data = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        let mut startup_path = std::path::PathBuf::from(app_data);
        startup_path.push(r"Microsoft\Windows\Start Menu\Programs\Startup");
        startup_path.push("YATS.lnk");

        if enabled {
            let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
            let exe_str = exe_path.to_str().ok_or("Invalid exe path")?;
            let lnk_str = startup_path.to_str().ok_or("Invalid shortcut path")?;

            let script = format!(
                "$s=(New-Object -ComObject WScript.Shell).CreateShortcut('{}');$s.TargetPath='{}';$s.Save()",
                lnk_str, exe_str
            );

            Command::new("powershell")
                .args(&["-Command", &script])
                .creation_flags(0x08000000) // CREATE_NO_WINDOW
                .status()
                .map_err(|e| e.to_string())?;
        } else {
            if startup_path.exists() {
                std::fs::remove_file(startup_path).map_err(|e| e.to_string())?;
            }

            // Also clean up legacy registry entry if exists
            let hkcu = RegKey::predef(HKEY_CURRENT_USER);
            if let Ok(run_key) = hkcu.open_subkey_with_flags(
                "Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                KEY_WRITE,
            ) {
                let _ = run_key.delete_value("YATS");
            }
        }
        Ok(())
    }

    fn get_aap_threshold() -> Result<i32, String> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let key = match hkcu.open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\PrecisionTouchPad") {
            Ok(k) => k,
            Err(_) => return Ok(2), // Default fallback
        };

        match key.get_value::<u32, _>("AAPThreshold") {
            Ok(v) => Ok(v as i32),
            Err(_) => Ok(2), // Default fallback
        }
    }

    fn set_aap_threshold(value: i32) -> Result<(), String> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let key = hkcu
            .open_subkey_with_flags(
                "Software\\Microsoft\\Windows\\CurrentVersion\\PrecisionTouchPad",
                KEY_WRITE,
            )
            .map_err(|e| format!("Failed to open PrecisionTouchPad key: {}", e))?;

        key.set_value("AAPThreshold", &(value as u32))
            .map_err(|e| format!("Failed to set AAPThreshold: {}", e))
    }

    fn deep_registry_clean() -> Result<String, String> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let mut report = String::new();

        // 1. Precision Touchpad Status
        if let Ok((key, _)) = hkcu.create_subkey(
            "Software\\Microsoft\\Windows\\CurrentVersion\\PrecisionTouchPad\\Status",
        ) {
            if key.delete_value("Enabled").is_ok() {
                report.push_str("Deleted: Status\\Enabled\n");
            }
        }

        if let Ok(key) = hkcu.open_subkey_with_flags(
            "Software\\Microsoft\\Windows\\CurrentVersion\\PrecisionTouchPad",
            KEY_WRITE,
        ) {
            if key.delete_value("LeaveOnLevel").is_ok() {
                report.push_str("Deleted: LeaveOnLevel\n");
            }
        }

        // 2. Synaptics / ELAN / System-wide settings (from lib.rs)
        // These often require admin rights, but we try anyway (is_elevated check is done in command)
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

        // Synaptics
        if let Ok(key) = hklm.open_subkey_with_flags(
            "SOFTWARE\\Synaptics\\SynTP\\Defaults",
            KEY_WRITE,
        ) {
            if key.set_value("PalmKms", &0u32).is_ok() {
                report.push_str("Synced: Synaptics PalmKms -> 0\n");
            }
            if key.set_value("SimultaneousTimeThreshold", &0u32).is_ok() {
                report.push_str("Synced: Synaptics SimultaneousTimeThreshold -> 0\n");
            }
        }

        // ELAN
        if let Ok(key) = hklm.open_subkey_with_flags(
            "SYSTEM\\CurrentControlSet\\Control\\Elantech\\SmartPad",
            KEY_WRITE,
        ) {
            if key.set_value("DisableWhenType_Enable", &0u32).is_ok() {
                report.push_str("Synced: ELAN DisableWhenType_Enable -> 0\n");
            }
        }

        if report.is_empty() {
            report.push_str("No registry entries found to clean.");
        }

        Ok(report)
    }
}

// Re-export for convenience
pub use WindowsPlatform as Platform;

// Input simulation convenience functions
pub fn send_mouse_click(mb: &MouseButton, pressed: bool) {
    <WindowsPlatform as super::InputSimulation>::send_mouse_click(mb, pressed);
}

pub fn send_key_click(vk: u16, pressed: bool) {
    <WindowsPlatform as super::InputSimulation>::send_key_click(vk, pressed);
}

pub fn send_mouse_scroll(delta: i32) {
    <WindowsPlatform as super::InputSimulation>::send_mouse_scroll(delta);
}

pub fn execute_window_action(action: WindowAction) {
    <WindowsPlatform as super::WindowManagement>::execute_window_action(action);
}

struct DragState {
    hwnd: usize,
    x: i32,
    y: i32,
}

static DRAG_STATE: Mutex<Option<DragState>> = Mutex::new(None);

pub fn begin_window_drag() -> bool {
    unsafe {
        let mut point: POINT = std::mem::zeroed();
        if GetCursorPos(&mut point) == 0 {
            return false;
        }
        let hwnd = WindowFromPoint(point);
        if hwnd.is_null() {
            return false;
        }
        let hwnd = GetAncestor(hwnd, GA_ROOT);
        if hwnd.is_null() || IsWindowVisible(hwnd) == 0 {
            return false;
        }

        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == std::process::id() {
            return false;
        }

        let mut class_name = [0u16; 256];
        let len = GetClassNameW(hwnd, class_name.as_mut_ptr(), class_name.len() as i32);
        if len > 0 {
            let name = String::from_utf16_lossy(&class_name[..len as usize]);
            if matches!(
                name.as_str(),
                "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
            ) {
                return false;
            }
        }

        let mut rect: RECT = std::mem::zeroed();
        if GetWindowRect(hwnd, &mut rect) == 0 {
            return false;
        }
        if IsZoomed(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
            if GetWindowRect(hwnd, &mut rect) == 0 {
                return false;
            }
        }

        *crate::lock_unpoison(&DRAG_STATE) = Some(DragState {
            hwnd: hwnd as usize,
            x: rect.left,
            y: rect.top,
        });
        crate::audit_log("[DRAG] begin");
        true
    }
}

pub fn move_dragged_window(dx: i32, dy: i32) {
    if let Some(state) = crate::lock_unpoison(&DRAG_STATE).as_mut() {
        state.x += dx;
        state.y += dy;
        unsafe {
            SetWindowPos(
                state.hwnd as HWND,
                null_mut(),
                state.x,
                state.y,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }
}

pub fn end_window_drag() {
    *crate::lock_unpoison(&DRAG_STATE) = None;
    crate::audit_log("[DRAG] end");
}
