// Linux-specific platform implementations

use crate::config::{MouseButton, WindowAction};
use enigo::{Axis, Button, Direction, Enigo, Key, Keyboard, Mouse, Settings};

use std::sync::Mutex;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ClientMessageEvent, ConnectionExt, EventMask};
use x11rb::rust_connection::RustConnection;

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

struct DragState {
    conn: RustConnection,
    window: u32,
    root: u32,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    moveresize_atom: u32,
}

static DRAG_STATE: Mutex<Option<DragState>> = Mutex::new(None);

pub fn begin_window_drag() -> bool {
    let Ok((conn, screen_num)) = RustConnection::connect(None) else {
        crate::audit_log("[DRAG] X connection failed");
        return false;
    };
    let Some(screen) = conn.setup().roots.get(screen_num) else {
        return false;
    };
    let root = screen.root;

    let intern = |name: &[u8]| {
        conn.intern_atom(false, name)
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .map(|reply| reply.atom)
    };

    // ポインタ配下のウィンドウを root から辿る。先頭は root の子
    // （再ペアレント型 WM ではフレーム）で、末端ほど内側の子になる。
    let mut chain: Vec<u32> = Vec::new();
    let mut current = root;
    loop {
        let Some(reply) = conn
            .query_pointer(current)
            .ok()
            .and_then(|cookie| cookie.reply().ok())
        else {
            break;
        };
        if reply.child == 0 {
            break;
        }
        chain.push(reply.child);
        current = reply.child;
        if chain.len() >= 16 {
            break;
        }
    }
    if chain.is_empty() {
        return false;
    }

    // EWMH のクライアント一覧に載っている最も外側のウィンドウが対象。
    // _NET_WM_STATE などのクライアントメッセージはフレームではなく
    // クライアントウィンドウ宛てでないと WM に届かない。
    let client_list: Vec<u32> = intern(b"_NET_CLIENT_LIST")
        .and_then(|atom| {
            conn.get_property(false, root, atom, AtomEnum::WINDOW, 0, u32::MAX)
                .ok()
                .and_then(|cookie| cookie.reply().ok())
        })
        .and_then(|reply| reply.value32().map(|values| values.collect()))
        .unwrap_or_default();
    let mut window = chain.iter().copied().find(|w| client_list.contains(w));
    if window.is_none() {
        // EWMH 非対応 WM 向け: WM_STATE を持つウィンドウをクライアントとみなす。
        if let Some(wm_state_atom) = intern(b"WM_STATE") {
            window = chain.iter().copied().find(|&w| {
                conn.get_property(false, w, wm_state_atom, AtomEnum::ANY, 0, 0)
                    .ok()
                    .and_then(|cookie| cookie.reply().ok())
                    .map(|reply| reply.type_ != 0)
                    .unwrap_or(false)
            });
        }
    }
    let window = window.unwrap_or(chain[0]);

    if let Some(pid_atom) = intern(b"_NET_WM_PID") {
        if let Some(pid) = conn
            .get_property(false, window, pid_atom, AtomEnum::ANY, 0, 1)
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .and_then(|reply| reply.value32().and_then(|mut values| values.next()))
        {
            if pid == std::process::id() {
                return false;
            }
        }
    }

    // 位置は root 座標へ変換して取得する（get_geometry は親基準のため、
    // フレーム内のクライアントでは画面座標にならない）。
    let translate = |conn: &RustConnection| {
        conn.translate_coordinates(window, root, 0, 0)
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .map(|reply| (i32::from(reply.dst_x), i32::from(reply.dst_y)))
    };
    let Some(mut pos) = translate(&conn) else {
        return false;
    };
    let Some(geometry) = conn
        .get_geometry(window)
        .ok()
        .and_then(|cookie| cookie.reply().ok())
    else {
        return false;
    };
    let Some(moveresize_atom) = intern(b"_NET_MOVERESIZE_WINDOW") else {
        return false;
    };
    if let (Some(state), Some(horz), Some(vert)) = (
        intern(b"_NET_WM_STATE"),
        intern(b"_NET_WM_STATE_MAXIMIZED_HORZ"),
        intern(b"_NET_WM_STATE_MAXIMIZED_VERT"),
    ) {
        let maximized = conn
            .get_property(false, window, state, AtomEnum::ANY, 0, u32::MAX)
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .and_then(|reply| {
                reply
                    .value32()
                    .map(|mut atoms| atoms.any(|atom| atom == horz || atom == vert))
            })
            .unwrap_or(false);
        if maximized {
            let event = ClientMessageEvent::new(32, window, state, [0, horz, vert, 1, 0]);
            let _ = conn.send_event(
                false,
                root,
                EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
                event,
            );
            let _ = conn.flush();
            // 最大化解除が WM に反映されるのを少し待ってから位置を取り直す
            // （解除後の復元位置が開始位置になる）。
            std::thread::sleep(std::time::Duration::from_millis(50));
            if let Some(refreshed) = translate(&conn) {
                pos = refreshed;
            }
        }
    }

    *crate::lock_unpoison(&DRAG_STATE) = Some(DragState {
        conn,
        window,
        root,
        x: pos.0,
        y: pos.1,
        w: u32::from(geometry.width),
        h: u32::from(geometry.height),
        moveresize_atom,
    });
    crate::audit_log("[DRAG] begin");
    true
}

pub fn move_dragged_window(dx: i32, dy: i32) {
    if let Some(state) = crate::lock_unpoison(&DRAG_STATE).as_mut() {
        state.x += dx;
        state.y += dy;
        let flags = 1 | (1 << 8) | (1 << 9) | (1 << 10) | (1 << 11) | (1 << 12);
        let event = ClientMessageEvent::new(
            32,
            state.window,
            state.moveresize_atom,
            [flags, state.x as u32, state.y as u32, state.w, state.h],
        );
        let _ = state.conn.send_event(
            false,
            state.root,
            EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
            event,
        );
        let _ = state.conn.flush();
    }
}

pub fn end_window_drag() {
    *crate::lock_unpoison(&DRAG_STATE) = None;
    crate::audit_log("[DRAG] end");
}
