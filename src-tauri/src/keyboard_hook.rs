use crate::config::{Action, AppConfig, MouseButton, ScrollConfig, WindowAction};
use crate::touchpad_monitor::TouchpadMonitor;
use rdev::{simulate, EventType};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, RwLock};
use tauri::AppHandle;
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

const INPUT_KEYBOARD: u32 = 1;

// v0.4.7: ブラウザナビゲーション用仮想キー定数
const VK_BROWSER_BACK: u16 = 0xA6;
const VK_BROWSER_FORWARD: u16 = 0xA7;

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

fn send_mouse_scroll(delta: i32) {
    unsafe {
        let mut input: INPUT = std::mem::zeroed();
        input.type_ = INPUT_MOUSE;
        let mut mi: MOUSEINPUT = std::mem::zeroed();

        mi.dwFlags = MOUSEEVENTF_WHEEL;
        mi.mouseData = delta as u32;

        *input.u.mi_mut() = mi;
        SendInput(1, &mut input, std::mem::size_of::<INPUT>() as i32);
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
                unsafe {
                    let hwnd = GetForegroundWindow();
                    if hwnd.is_null() {
                        return;
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
    scroll_anchor: Option<winapi::shared::windef::POINT>,
}

impl HookWorker {
    pub fn new(rx: Receiver<RemapCommand>, monitor: Arc<TouchpadMonitor>) -> Self {
        Self {
            rx,
            active_scroll: None,
            monitor,
            scroll_anchor: None,
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
                            unsafe {
                                let mut pt = std::mem::zeroed();
                                winapi::um::winuser::GetCursorPos(&mut pt);
                                self.scroll_anchor = Some(pt);
                            }
                        } else {
                            self.scroll_anchor = None;
                        }
                    }
                }
            }

            // アクション開始時の「飛び」を防ぐため、常にデルタを消費
            let _dx = self.monitor.consume_x_delta();
            let _dy = self.monitor.consume_y_delta();

            if let Some(cfg) = &self.active_scroll {
                if let Some(anchor) = self.scroll_anchor {
                    unsafe {
                        winapi::um::winuser::SetCursorPos(anchor.x, anchor.y);
                    }
                }

                let raw_delta = self.monitor.consume_y_delta();
                if raw_delta != 0 {
                    let mut delta = raw_delta;

                    // 反転
                    if cfg.invert {
                        delta = -delta;
                    }

                    // 加速 (対数的)
                    // 加速ONの場合、符号を維持して二乗またはブースト
                    let mut scale = cfg.sensitivity as i32;
                    if cfg.acceleration {
                        // 単純加速: 大きさ * 感度 * ブースト
                        // または非線形で感度をブースト
                        // 標準加速は高速移動時により高い倍率がかかると仮定
                        if delta.abs() > 5 {
                            scale *= 2;
                        }
                    }

                    // 標準スケーリング
                    // 基本感度 100 = 1.0倍
                    // 200 = 2.0倍 など
                    // 100で割って正規化
                    let scaled_delta = (delta * scale) / 10;

                    if scaled_delta != 0 {
                        send_mouse_scroll(scaled_delta);
                    }
                }
            }

            std::thread::sleep(std::time::Duration::from_millis(5));
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
        std::thread::spawn(move || {
            let mut worker = HookWorker::new(rx, mon_clone);
            worker.run();
        });

        Self {
            monitor,
            config,
            tx,
        }
    }

    pub fn start(&self) {
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
            eprintln!("Error: {:?}", e);
        }
    }
}
