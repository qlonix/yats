// Linux-specific platform implementations

use crate::config::{MouseButton, WindowAction};
use enigo::{Axis, Button, Direction, Enigo, Key, Keyboard, Mouse, Settings};

use std::sync::Mutex;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    AtomEnum, ClientMessageEvent, ConfigureWindowAux, ConnectionExt, EventMask,
};
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
        // delta は 1/120 ノッチ単位（Windows の WHEEL_DELTA と同じ粒度）。
        // 送信経路はスクロール開始時にポインタ下のウィンドウで決める
        // （prepare_scroll_route）。ブラウザは仮想ホイールの細かい
        // valuator を自分で読んで滑らかにスクロールするが、GTK 系は
        // その経路では一切スクロールしない（実機確認済み）ため、
        // それ以外は従来どおりコアポインタのクリックで送る。
        let route = *crate::lock_unpoison(&SCROLL_ROUTE);
        if route == Some(ScrollRoute::Device) && send_scroll_hires(delta) {
            return;
        }
        send_scroll_clicks(delta);
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

/// スクロールの送信経路。Device = 仮想ホイール（ブラウザ向け）、
/// Clicks = コアポインタのボタンイベント（それ以外）。
#[derive(Clone, Copy, PartialEq)]
enum ScrollRoute {
    Device,
    Clicks,
}
static SCROLL_ROUTE: Mutex<Option<ScrollRoute>> = Mutex::new(None);

/// スクロール開始時に、ポインタ下のウィンドウの WM_CLASS を見て
/// 送信経路を決める。ブラウザ系（valuator を自分で読む）だけを
/// 仮想ホイール経路にし、それ以外はクリック経路にする。
pub fn prepare_scroll_route() {
    let class = window_class_under_pointer().unwrap_or_default();
    let browser = ["firefox", "zen", "chrom", "brave", "vivaldi", "opera", "edge"]
        .iter()
        .any(|token| class.contains(token));
    let route = if browser {
        ScrollRoute::Device
    } else {
        ScrollRoute::Clicks
    };
    crate::audit_log(&format!(
        "[SCROLL] route={} (class={})",
        if browser { "device" } else { "clicks" },
        if class.is_empty() { "-" } else { &class }
    ));
    *crate::lock_unpoison(&SCROLL_ROUTE) = Some(route);
}

pub fn clear_scroll_route() {
    *crate::lock_unpoison(&SCROLL_ROUTE) = None;
}

/// ポインタ配下のクライアントウィンドウの WM_CLASS を小文字で返す
/// （インスタンス名とクラス名の連結）。取得できなければ None。
fn window_class_under_pointer() -> Option<String> {
    let (conn, screen_num) = RustConnection::connect(None).ok()?;
    let screen = conn.setup().roots.get(screen_num)?;
    let root = screen.root;
    let intern = |name: &[u8]| {
        conn.intern_atom(false, name)
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .map(|reply| reply.atom)
    };
    let mut chain: Vec<u32> = Vec::new();
    let mut current = root;
    loop {
        let reply = conn
            .query_pointer(current)
            .ok()
            .and_then(|cookie| cookie.reply().ok())?;
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
        return None;
    }
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
    let reply = conn
        .get_property(false, window, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, 64)
        .ok()
        .and_then(|cookie| cookie.reply().ok())?;
    let text = String::from_utf8_lossy(&reply.value).replace('\0', " ");
    let text = text.trim().to_lowercase();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

/// 整数ノッチ分だけ enigo のクリックで送り、端数は次回に持ち越す。
fn send_scroll_clicks(delta: i32) {
    let whole = {
        let mut rem = crate::lock_unpoison(&SCROLL_FALLBACK_REM);
        let total = *rem + delta;
        let whole = total / 120;
        *rem = total - whole * 120;
        whole
    };
    if whole != 0 {
        if let Ok(mut enigo) = Enigo::new(&Settings::default()) {
            // enigo の正方向は下（ボタン5）、従来の rdev の正方向は上（ボタン4）。
            // 従来と同じ方向にスクロールするよう符号を反転する。
            let _ = enigo.scroll(whole.saturating_neg(), Axis::Vertical);
        }
    }
}

/// 高解像度スクロール送信用の uinput 仮想マウス（初回スクロール時に作る）。
static SCROLL_DEVICE: Mutex<Option<evdev::uinput::VirtualDevice>> = Mutex::new(None);
static SCROLL_DEVICE_FAILED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
/// フォールバック（クリック送信）用に持ち越す 1/120 ノッチ単位の端数。
static SCROLL_FALLBACK_REM: Mutex<i32> = Mutex::new(0);
/// 高解像度ホイールの累計。1 ノッチ（120）を超えるたびに、離散イベント
/// （REL_WHEEL）も併せて送るために使う。
static SCROLL_HIRES_ACC: Mutex<i32> = Mutex::new(0);

fn create_scroll_device() -> Option<evdev::uinput::VirtualDevice> {
    let mut keys = evdev::AttributeSet::<evdev::Key>::default();
    keys.insert(evdev::Key::BTN_LEFT);
    keys.insert(evdev::Key::BTN_RIGHT);
    keys.insert(evdev::Key::BTN_MIDDLE);
    let mut axes = evdev::AttributeSet::<evdev::RelativeAxisType>::default();
    axes.insert(evdev::RelativeAxisType::REL_X);
    axes.insert(evdev::RelativeAxisType::REL_Y);
    axes.insert(evdev::RelativeAxisType::REL_WHEEL);
    axes.insert(evdev::RelativeAxisType::REL_HWHEEL);
    axes.insert(evdev::RelativeAxisType::REL_WHEEL_HI_RES);
    axes.insert(evdev::RelativeAxisType::REL_HWHEEL_HI_RES);
    let dev = evdev::uinput::VirtualDeviceBuilder::new()
        .ok()?
        .name("YATS Virtual Scroll")
        .with_keys(&keys)
        .ok()?
        .with_relative_axes(&axes)
        .ok()?
        .build()
        .ok()?;
    crate::audit_log("[SCROLL] hi-res scroll device ready");
    Some(dev)
}

/// delta（1/120 ノッチ単位）を REL_WHEEL_HI_RES で送る。libinput 経由で
/// XI2 の滑らかなスクロール軸としてアプリに届く。送れなければ false。
fn send_scroll_hires(delta: i32) -> bool {
    use std::sync::atomic::Ordering;
    let mut guard = crate::lock_unpoison(&SCROLL_DEVICE);
    if guard.is_none() {
        if SCROLL_DEVICE_FAILED.load(Ordering::SeqCst) {
            return false;
        }
        match create_scroll_device() {
            Some(dev) => *guard = Some(dev),
            None => {
                SCROLL_DEVICE_FAILED.store(true, Ordering::SeqCst);
                crate::audit_log("[SCROLL] hi-res scroll device unavailable, using clicks");
                return false;
            }
        }
    }
    let Some(dev) = guard.as_mut() else {
        return false;
    };
    // 細かい刻みの HI_RES をそのまま送り、累計が 1 ノッチを超えたら
    // REL_WHEEL（離散）も同じフレームで併せて送る（実機の hi-res
    // ホイールと同じ作法）。この経路は valuator を自分で読むブラウザ
    // 専用で、滑らかさはこの細かい刻みから来る。
    let detents = {
        let mut acc = crate::lock_unpoison(&SCROLL_HIRES_ACC);
        *acc += delta;
        let detents = *acc / 120;
        *acc -= detents * 120;
        detents
    };
    let mut events = Vec::with_capacity(3);
    if detents != 0 {
        events.push(evdev::InputEvent::new(
            evdev::EventType::RELATIVE,
            evdev::RelativeAxisType::REL_WHEEL.0,
            detents,
        ));
    }
    events.push(evdev::InputEvent::new(
        evdev::EventType::RELATIVE,
        evdev::RelativeAxisType::REL_WHEEL_HI_RES.0,
        delta,
    ));
    events.push(evdev::InputEvent::new(
        evdev::EventType::SYNCHRONIZATION,
        evdev::Synchronization::SYN_REPORT.0,
        0,
    ));
    dev.emit(&events).is_ok()
}

struct DragState {
    conn: RustConnection,
    window: u32,
    root: u32,
    x: i32,
    y: i32,
    last_cx: i32,
    last_cy: i32,
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

    let Some(pointer) = conn
        .query_pointer(root)
        .ok()
        .and_then(|cookie| cookie.reply().ok())
    else {
        return false;
    };

    *crate::lock_unpoison(&DRAG_STATE) = Some(DragState {
        conn,
        window,
        root,
        x: pos.0,
        y: pos.1,
        last_cx: i32::from(pointer.root_x),
        last_cy: i32::from(pointer.root_y),
    });
    crate::audit_log("[DRAG] begin");
    true
}

pub fn move_dragged_window() {
    if let Some(state) = crate::lock_unpoison(&DRAG_STATE).as_mut() {
        // 移動量はカーソルの実移動量に合わせる（タッチパッドの生デルタは
        // デバイス座標系なので、カーソルより速く動いてしまう）。
        let Some(pointer) = state
            .conn
            .query_pointer(state.root)
            .ok()
            .and_then(|cookie| cookie.reply().ok())
        else {
            return;
        };
        let cx = i32::from(pointer.root_x);
        let cy = i32::from(pointer.root_y);
        let dx = cx - state.last_cx;
        let dy = cy - state.last_cy;
        if (dx, dy) == (0, 0) {
            return;
        }
        state.last_cx = cx;
        state.last_cy = cy;
        state.x += dx;
        state.y += dy;
        // 移動は ConfigureRequest で行う。_NET_MOVERESIZE_WINDOW だと
        // WM の対話移動ポリシーで画面内に制限され、サイズも送ってしまう。
        // ConfigureRequest はアプリ自身の移動と同じ経路で、WM も通常は
        // 画面外へのはみ出しを許容する。サイズは送らないので、最大化
        // 解除後の復元サイズがそのまま維持される。
        let aux = ConfigureWindowAux::new().x(state.x).y(state.y);
        let _ = state.conn.configure_window(state.window, &aux);
        let _ = state.conn.flush();
    }
}

pub fn end_window_drag() {
    *crate::lock_unpoison(&DRAG_STATE) = None;
    crate::audit_log("[DRAG] end");
}
