// Linux-specific platform implementations

use crate::config::{MouseButton, WindowAction};
use enigo::{Axis, Button, Direction, Enigo, Key, Keyboard, Mouse, Settings};

pub struct LinuxPlatform;

impl super::InputSimulation for LinuxPlatform {
    fn send_mouse_click(mb: &MouseButton, pressed: bool) {
        if let Ok(mut enigo) = Enigo::new(&Settings::default()) {
            let button = match mb {
                MouseButton::Left => Button::Left,
                MouseButton::Right => Button::Right,
                MouseButton::Middle => Button::Middle,
            };
            let direction = if pressed {
                Direction::Press
            } else {
                Direction::Release
            };
            let _ = enigo.button(button, direction);
        }
    }

    fn send_key_click(vk: u16, pressed: bool) {
        let arrow = match vk {
            0xA6 => Key::LeftArrow,
            0xA7 => Key::RightArrow,
            _ => return,
        };
        // 解放イベントが届くまでキーの押下状態を維持する。
        let settings = Settings {
            release_keys_when_dropped: false,
            ..Settings::default()
        };
        if let Ok(mut enigo) = Enigo::new(&settings) {
            if pressed {
                let _ = enigo.key(Key::Alt, Direction::Press);
                let _ = enigo.key(arrow, Direction::Press);
            } else {
                let _ = enigo.key(arrow, Direction::Release);
                let _ = enigo.key(Key::Alt, Direction::Release);
            }
        }
    }

    fn send_mouse_scroll(delta: i32) {
        if delta == 0 {
            return;
        }
        if let Ok(mut enigo) = Enigo::new(&Settings::default()) {
            // enigo の正方向は下（ボタン5）、従来の rdev の正方向は上（ボタン4）。
            // 従来と同じ方向にスクロールするよう符号を反転する。
            let _ = enigo.scroll(delta.saturating_neg(), Axis::Vertical);
        }
    }
}

impl super::WindowManagement for LinuxPlatform {
    fn execute_window_action(action: WindowAction) {
        use std::process::Command;
        let cmd = match action {
            WindowAction::Close => "xdotool getactivewindow windowclose",
            WindowAction::Minimize => "xdotool getactivewindow windowminimize",
            WindowAction::Maximize => "wmctrl -r :ACTIVE: -b toggle,maximized_vert,maximized_horz",
        };
        let _ = Command::new("sh").args(&["-c", cmd]).status();
    }
}

impl super::SystemInfo for LinuxPlatform {
    fn is_elevated() -> bool {
        use nix::unistd::Uid;
        Uid::effective().is_root()
    }

    fn get_startup_status() -> bool {
        if let Some(home) = std::env::var("HOME").ok() {
            let desktop_file = format!("{}/.config/autostart/yats.desktop", home);
            std::path::Path::new(&desktop_file).exists()
        } else {
            false
        }
    }

    fn set_startup(enabled: bool) -> Result<(), String> {
        let home = std::env::var("HOME").map_err(|_| "HOME not set".to_string())?;
        let autostart_dir = format!("{}/.config/autostart", home);
        let desktop_file = format!("{}/yats.desktop", autostart_dir);

        if enabled {
            std::fs::create_dir_all(&autostart_dir)
                .map_err(|e| format!("Failed to create autostart directory: {}", e))?;

            let exe_path =
                std::env::current_exe().map_err(|e| format!("Failed to get exe path: {}", e))?;
            let exe_str = exe_path
                .to_str()
                .ok_or("Failed to convert path to string")?;

            let content = format!(
                "[Desktop Entry]\n\
                 Type=Application\n\
                 Name=YATS\n\
                 Exec={}\n\
                 Hidden=false\n\
                 NoDisplay=false\n\
                 X-GNOME-Autostart-enabled=true\n",
                exe_str
            );

            std::fs::write(&desktop_file, content)
                .map_err(|e| format!("Failed to write desktop file: {}", e))?;
        } else {
            if std::path::Path::new(&desktop_file).exists() {
                std::fs::remove_file(&desktop_file)
                    .map_err(|e| format!("Failed to remove desktop file: {}", e))?;
            }
        }
        Ok(())
    }

    fn get_aap_threshold() -> Result<i32, String> {
        Err("AAP threshold is Windows-only".to_string())
    }

    fn set_aap_threshold(_value: i32) -> Result<(), String> {
        Err("AAP threshold is Windows-only".to_string())
    }

    fn deep_registry_clean() -> Result<String, String> {
        Err("Registry cleaning is Windows-only".to_string())
    }
}

// Re-export for convenience
pub use LinuxPlatform as Platform;

// Input simulation convenience functions
pub fn send_mouse_click(mb: &MouseButton, pressed: bool) {
    <LinuxPlatform as super::InputSimulation>::send_mouse_click(mb, pressed);
}

pub fn send_key_click(vk: u16, pressed: bool) {
    <LinuxPlatform as super::InputSimulation>::send_key_click(vk, pressed);
}

pub fn send_mouse_scroll(delta: i32) {
    <LinuxPlatform as super::InputSimulation>::send_mouse_scroll(delta);
}

pub fn execute_window_action(action: WindowAction) {
    <LinuxPlatform as super::WindowManagement>::execute_window_action(action);
}
