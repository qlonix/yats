use crate::config::{Action, AppConfig, MouseButton, ScrollConfig, WindowAction};
use crate::touchpad_monitor::TouchpadMonitor;
#[cfg(target_os = "linux")]
use enigo::{Button, Coordinate, Direction, Enigo, Mouse, Settings};
use rdev::{simulate, EventType};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, RwLock};
use tauri::AppHandle;
#[cfg(target_os = "windows")]
use winapi::um::winuser::{
    GetForegroundWindow, IsZoomed, PostMessageW, SendInput, ShowWindow, INPUT, INPUT_MOUSE,
    KEYEVENTF_KEYUP, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN,
    MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL,
    MOUSEINPUT, SC_CLOSE, SW_MAXIMIZE, SW_MINIMIZE, SW_RESTORE, WM_SYSCOMMAND,
};

pub static IS_PAUSED: AtomicBool = AtomicBool::new(false);
pub static IS_SIMULATING: AtomicBool = AtomicBool::new(false);

pub struct KeyboardHook {
    monitor: Arc<TouchpadMonitor>,
    config: Arc<RwLock<AppConfig>>,
    tx: Sender<RemapCommand>,
}

#[derive(Clone, Debug)]
pub enum RemapCommand {
    Execute(Action, bool),
    UpdateScroll(Option<ScrollConfig>),
}

#[cfg(target_os = "windows")]
const INPUT_KEYBOARD: u32 = 1;

// v0.4.7: ブラウザナビゲーション用仮想キー定数
const VK_BROWSER_BACK: u16 = 0xA6;
const VK_BROWSER_FORWARD: u16 = 0xA7;

#[cfg(target_os = "windows")]
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

#[cfg(target_os = "linux")]
fn send_mouse_click(mb: &MouseButton, pressed: bool) {
    let mut enigo = Enigo::new(&Settings::default()).unwrap();
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

#[cfg(target_os = "windows")]
fn send_key_click(vk: u16, pressed: bool) {
    unsafe {
        let mut input: INPUT = std::mem::zeroed();
        input.type_ = INPUT_KEYBOARD;
        let mut ki: winapi::um::winuser::KEYBDINPUT = std::mem::zeroed();

        ki.wVk = vk;
        ki.dwFlags = if pressed { 0 } else { KEYEVENTF_KEYUP };
        ki.time = 0;
        ki.dwExtraInfo = 0xDEADBEEF;

        *input.u.ki_mut() = ki;
        SendInput(1, &mut input, std::mem::size_of::<INPUT>() as i32);
    }
}

#[cfg(target_os = "linux")]
fn send_key_click(vk: u16, pressed: bool) {
    use rdev::{simulate, EventType, Key};
    let keys = match vk {
        0xA6 => vec![Key::Alt, Key::LeftArrow],
        0xA7 => vec![Key::Alt, Key::RightArrow],
        _ => return,
    };
    if pressed {
        for &k in &keys {
            let _ = simulate(&EventType::KeyPress(k));
        }
    } else {
        for &k in keys.iter().rev() {
            let _ = simulate(&EventType::KeyRelease(k));
        }
    }
}

#[cfg(target_os = "windows")]
fn send_mouse_scroll(delta: i32) {
    unsafe {
        let mut input: INPUT = std::mem::zeroed();
        input.type_ = INPUT_MOUSE;
        let mut mi: MOUSEINPUT = std::mem::zeroed();

        mi.dwFlags = MOUSEEVENTF_WHEEL;
        // v1.2.2: Micro-Scrolling 対応のため生値を受け取るように変更
        // 呼び出し側で適切な値 (例: 20, 30, 120) を計算して渡す
        mi.mouseData = delta as u32;

        *input.u.mi_mut() = mi;
        SendInput(1, &mut input, std::mem::size_of::<INPUT>() as i32);
    }
}

#[cfg(target_os = "linux")]
fn send_mouse_scroll(delta: i32) {
    use rdev::{simulate, EventType};
    // Use rdev for scroll as enigo scroll can be unreliable
    // delta is expected to be raw wheel delta, we convert to scroll events
    let scroll_count = delta / 30; // Adjust divisor for sensitivity
    if scroll_count != 0 {
        for _ in 0..scroll_count.abs() {
            let delta_y = if scroll_count > 0 { 1 } else { -1 };
            let _ = simulate(&EventType::Wheel {
                delta_x: 0,
                delta_y: delta_y as i64,
            });
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
}

fn execute_action(action: Action, pressed: bool) {
    match action {
        Action::MouseClick(mb) => {
            // ドラッグ対応: キープレスを MouseDown に、キーリリースを MouseUp にマッピング
            if pressed {
                send_mouse_click(&mb, true);
            } else {
                send_mouse_click(&mb, false);
            }
        }
        Action::MouseDoubleClick(mb) => {
            // ダブルクリックドラッグ:
            // 押下 -> クリック、リリース、ダウン（保持）
            // リリース -> アップ
            if pressed {
                send_mouse_click(&mb, true);
                send_mouse_click(&mb, false);
                std::thread::sleep(std::time::Duration::from_millis(50));
                send_mouse_click(&mb, true);
            } else {
                send_mouse_click(&mb, false);
            }
        }
        Action::MouseScroll(_) => {} // Handled elsewhere
        Action::KeyMacro(steps) => {
            if pressed {
                IS_SIMULATING.store(true, Ordering::SeqCst);
                for step in steps {
                    // コード内の全キーを押下
                    for key in step.iter().cloned() {
                        let _ = simulate(&EventType::KeyPress(key));
                    }
                    std::thread::sleep(std::time::Duration::from_millis(60));
                    // 全キーを逆順で解放
                    for key in step.iter().rev().cloned() {
                        let _ = simulate(&EventType::KeyRelease(key));
                    }
                    // 解放後の待機
                    std::thread::sleep(std::time::Duration::from_millis(40));
                }
                IS_SIMULATING.store(false, Ordering::SeqCst);
            }
        }
        Action::Window(win_act) => {
            if pressed {
                #[cfg(target_os = "windows")]
                unsafe {
                    let hwnd = GetForegroundWindow();
                    if hwnd.is_null() {
                        return;
                    }

                    // v2.9.0: ウィンドウ・ガード
                    // デスクトップやタスクバーなどのシステムシェルを操作対象から除外する
                    let mut class_name = [0u16; 256];
                    let len =
                        winapi::um::winuser::GetClassNameW(hwnd, class_name.as_mut_ptr(), 256);
                    if len > 0 {
                        let name = String::from_utf16_lossy(&class_name[..len as usize]);
                        // Progman/WorkerW はデスクトップ、Shell_TrayWnd はタスクバー
                        if name == "Progman" || name == "WorkerW" || name == "Shell_TrayWnd" {
                            return;
                        }
                    }

                    match win_act {
                        WindowAction::Close => {
                            // SC_CLOSE is more robust for window closing
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

                #[cfg(target_os = "linux")]
                {
                    use std::process::Command;
                    // xdotool requires piping: get window ID then perform action
                    let cmd = match win_act {
                        WindowAction::Close => "xdotool getactivewindow windowclose",
                        WindowAction::Minimize => "xdotool getactivewindow windowminimize",
                        WindowAction::Maximize => {
                            "wmctrl -r :ACTIVE: -b toggle,maximized_vert,maximized_horz"
                        }
                    };
                    // Use sh -c to execute as shell command
                    let _ = Command::new("sh").args(&["-c", cmd]).status();
                }
            }
        }
        Action::BrowserBack => {
            send_key_click(VK_BROWSER_BACK, pressed);
        }
        Action::BrowserForward => {
            send_key_click(VK_BROWSER_FORWARD, pressed);
        }
    }
}

pub struct HookWorker {
    rx: Receiver<RemapCommand>,
    active_scroll: Option<ScrollConfig>,
    monitor: Arc<TouchpadMonitor>,
    config: Arc<RwLock<AppConfig>>, // v2.3.0: Read global settings
    #[cfg(target_os = "windows")]
    scroll_anchor: Option<winapi::shared::windef::POINT>,
    #[cfg(target_os = "linux")]
    scroll_anchor: Option<(i32, i32)>,
    accumulator_y: f32,                   // v2.0.2: Fine-grained accumulator
    last_scroll_time: std::time::Instant, // v2.0.2: Batching dispatcher
}

impl HookWorker {
    pub fn new(
        rx: Receiver<RemapCommand>,
        monitor: Arc<TouchpadMonitor>,
        config: Arc<RwLock<AppConfig>>,
    ) -> Self {
        Self {
            rx,
            active_scroll: None,
            monitor,
            config,
            scroll_anchor: None,
            accumulator_y: 0.0,
            last_scroll_time: std::time::Instant::now(),
        }
    }

    pub fn run(&mut self) {
        loop {
            while let Ok(cmd) = self.rx.try_recv() {
                match cmd {
                    RemapCommand::Execute(act, pressed) => {
                        execute_action(act, pressed);
                    }
                    RemapCommand::UpdateScroll(cfg) => {
                        self.active_scroll = cfg;
                        if self.active_scroll.is_some() {
                            #[cfg(target_os = "windows")]
                            unsafe {
                                let mut pt = std::mem::zeroed();
                                winapi::um::winuser::GetCursorPos(&mut pt);
                                self.scroll_anchor = Some(pt);
                            }
                            #[cfg(target_os = "linux")]
                            {
                                let enigo = Enigo::new(&Settings::default()).unwrap();
                                let (x, y) = enigo.location().unwrap_or((0, 0));
                                self.scroll_anchor = Some((x, y));
                            }
                        } else {
                            self.scroll_anchor = None;
                        }
                    }
                }
            }

            // 1. Process commands as before...

            // 2. Main Logic
            if let Some(_cfg) = &self.active_scroll {
                if let Some(anchor) = self.scroll_anchor {
                    #[cfg(target_os = "windows")]
                    unsafe {
                        winapi::um::winuser::SetCursorPos(anchor.x, anchor.y);
                    }
                    #[cfg(target_os = "linux")]
                    {
                        let mut enigo = Enigo::new(&Settings::default()).unwrap();
                        let _ = enigo.move_mouse(anchor.0, anchor.1, Coordinate::Abs);
                    }
                }

                // v2.1.1: Double-consumption bug fixed.
                // 以前はループの冒頭で _dy を消費し、ここで再び consume していたため、
                // 大半の入力が 0 になり、スクロールが「重く」「カクつく」原因になっていた。
                let raw_delta = self.monitor.consume_y_delta();
                if raw_delta != 0 {
                    // v2.3.0: Global Scroll Settings & Range Shift
                    // 以前はキー毎に感度を持っていましたが、全キー共通のグローバル設定に変更しました。
                    // また、感度のレンジをさらに調整：旧 50% (Sens 50) が新 100% になるようにシフト（2倍細かく）。

                    let (global_sens, global_invert) = {
                        let cfg_lock = self.config.read().unwrap();
                        (cfg_lock.scroll_sensitivity, cfg_lock.scroll_invert)
                    };

                    let mut delta = raw_delta;
                    // v2.3.0: Invert prioritize global setting, fallback to per-key if needed (but UI now forces global)
                    if global_invert {
                        delta = -delta;
                    }

                    // Gain = global_sens / 100.0
                    // v2.7.0: Reverted to linear gain.
                    // UI handles the logarithmic mapping for the slider position.
                    let gain = (global_sens as f32) / 100.0;

                    const LINEAR_SCALE: f32 = 40.0;
                    self.accumulator_y += (delta as f32) * gain * LINEAR_SCALE;
                }

                // 3. Batching Dispatcher (Smooth 125Hz)
                let now = std::time::Instant::now();
                if now.duration_since(self.last_scroll_time).as_millis() >= 8 {
                    let output = self.accumulator_y.trunc() as i32;
                    if output != 0 {
                        send_mouse_scroll(output);
                        self.accumulator_y -= output as f32;
                    }
                    self.last_scroll_time = now;
                }
            } else {
                // スクロール中でない時はデルタを捨てて蓄積を防ぐ
                self.monitor.consume_x_delta();
                self.monitor.consume_y_delta();
                self.accumulator_y = 0.0; // アキュムレータもリセット
            }

            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
}

impl KeyboardHook {
    pub fn new(
        monitor: Arc<TouchpadMonitor>,
        config: Arc<RwLock<AppConfig>>,
        _app: Arc<Mutex<Option<AppHandle>>>,
    ) -> Self {
        let (tx, rx) = channel();
        let mon_clone = Arc::clone(&monitor);
        let cfg_clone = Arc::clone(&config);
        std::thread::spawn(move || {
            let mut worker = HookWorker::new(rx, mon_clone, cfg_clone);
            worker.run();
        });

        Self {
            monitor,
            config,
            tx,
        }
    }

    pub fn start(&self) {
        crate::audit_log("[HOOK] Starting keyboard hook (rdev::grab)...");
        let tx = self.tx.clone();
        let config_ref = Arc::clone(&self.config);
        let monitor_ref = Arc::clone(&self.monitor);
        // リピート無視のため押下キーを追跡
        let pressed_keys = Arc::new(Mutex::new(std::collections::HashSet::new()));

        if let Err(e) = rdev::grab(move |event| {
            if IS_PAUSED.load(Ordering::SeqCst) || IS_SIMULATING.load(Ordering::SeqCst) {
                return Some(event);
            }

            match event.event_type {
                rdev::EventType::KeyPress(key) => {
                    let mut keys = pressed_keys.lock().unwrap();
                    if keys.contains(&key) {
                        return None;
                    }

                    // 安全ガード: タッチパッドに触れていない場合はキーをスルー（何もしない）
                    // タッチがない場合、ユーザーは「機能をトリガーしない」ことを望んでいる
                    // 通常、これは「通常のキーとして動作する」ことを意味する
                    if !monitor_ref.is_touched() {
                        return Some(event);
                    }

                    keys.insert(key);

                    let action = {
                        let cfg = config_ref.read().unwrap();
                        cfg.mappings.get(&key).cloned()
                    };

                    if let Some(action) = action {
                        match action {
                            Action::MouseScroll(cfg) => {
                                monitor_ref.consume_y_delta();
                                let _ = tx.send(RemapCommand::UpdateScroll(Some(cfg)));
                            }
                            _ => {
                                let _ = tx.send(RemapCommand::Execute(action, true));
                            }
                        }
                        return None;
                    }
                }
                rdev::EventType::KeyRelease(key) => {
                    let mut keys = pressed_keys.lock().unwrap();
                    // セットにキーがない場合、Pressをキャプチャしていないことを意味する
                    // （安全ガードのため、またはマッピングされていなかったため）
                    // その場合、ReleaseをOSにスルーしなければならない
                    if !keys.remove(&key) {
                        return Some(event);
                    }

                    let action = {
                        let cfg = config_ref.read().unwrap();
                        cfg.mappings.get(&key).cloned()
                    };

                    if let Some(action) = action {
                        match action {
                            Action::MouseScroll(_) => {
                                let _ = tx.send(RemapCommand::UpdateScroll(None));
                            }
                            _ => {
                                let _ = tx.send(RemapCommand::Execute(action, false));
                            }
                        }
                        return None;
                    }
                }
                _ => {}
            }
            Some(event)
        }) {
            crate::audit_log(&format!("[HOOK] rdev::grab error: {:?}", e));
            eprintln!("[HOOK] Error: {:?}", e);
        }
        crate::audit_log("[HOOK] Keyboard hook exited");
    }
}
