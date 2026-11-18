// Platform-specific implementations
// This module provides a unified interface for platform-specific operations

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::*;
#[cfg(target_os = "windows")]
pub use windows::*;

use crate::config::{MouseButton, WindowAction};

/// Platform-specific input simulation
pub trait InputSimulation {
    fn send_mouse_click(mb: &MouseButton, pressed: bool);
    fn send_key_click(vk: u16, pressed: bool);
    fn send_mouse_scroll(delta: i32);
}

/// Platform-specific window management
pub trait WindowManagement {
    fn execute_window_action(action: WindowAction);
}

/// Platform-specific system information
pub trait SystemInfo {
    fn is_elevated() -> bool;
    fn get_startup_status() -> bool;
    fn set_startup(enabled: bool) -> Result<(), String>;
    fn get_aap_threshold() -> Result<i32, String>;
    fn set_aap_threshold(value: i32) -> Result<(), String>;
    fn deep_registry_clean() -> Result<String, String>;
}

/// Get the current cursor position
#[cfg(target_os = "windows")]
pub fn get_cursor_pos() -> Option<(i32, i32)> {
    unsafe {
        let mut pt: winapi::shared::windef::POINT = std::mem::zeroed();
        if winapi::um::winuser::GetCursorPos(&mut pt) != 0 {
            Some((pt.x, pt.y))
        } else {
            None
        }
    }
}

#[cfg(target_os = "linux")]
pub fn get_cursor_pos() -> Option<(i32, i32)> {
    use enigo::{Enigo, Mouse, Settings};
    Enigo::new(&Settings::default())
        .ok()
        .and_then(|e| e.location().ok())
}

/// アクティブウィンドウが自プロセスに属するか判定する。
#[cfg(target_os = "windows")]
pub fn is_active_window_own() -> bool {
    use winapi::um::winuser::{GetForegroundWindow, GetWindowThreadProcessId};

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return false;
        }
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, &mut pid) != 0 && pid == std::process::id()
    }
}

#[cfg(target_os = "linux")]
pub fn is_active_window_own() -> bool {
    let Ok(output) = std::process::Command::new("xdotool")
        .args(["getactivewindow", "getwindowpid"])
        .output()
    else {
        return false;
    };
    output.status.success()
        && String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse::<u32>()
            == Ok(std::process::id())
}

/// Set the cursor position
#[cfg(target_os = "windows")]
pub fn set_cursor_pos(x: i32, y: i32) {
    unsafe {
        winapi::um::winuser::SetCursorPos(x, y);
    }
}

#[cfg(target_os = "linux")]
pub fn set_cursor_pos(x: i32, y: i32) {
    use enigo::{Coordinate, Enigo, Mouse, Settings};
    if let Ok(mut enigo) = Enigo::new(&Settings::default()) {
        let _ = enigo.move_mouse(x, y, Coordinate::Abs);
    }
}

// Platform-specific touchpad monitoring
pub mod touchpad;
