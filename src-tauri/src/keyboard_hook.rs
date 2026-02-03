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
    MOUSEINPUT, SC_CLOSE, SC_MOVE, SW_MAXIMIZE, SW_MINIMIZE, SW_RESTORE, WM_SYSCOMMAND,
};

pub static IS_PAUSED: AtomicBool = AtomicBool::new(false);

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

// v0.4.7: Browser Navigation VK Constants
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
            // Drag Support: Map KeyPress to MouseDown, KeyRelease to MouseUp
            if pressed {
                send_mouse_click(&mb, true);
            } else {
                send_mouse_click(&mb, false);
            }
        }
        Action::MouseDoubleClick(mb) => {
            // Double Click Drag:
            // Press -> Click, Release, Down (Hold)
            // Release -> Up
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
        Action::KeyMacro(keys) => {
            if pressed {
                for key in &keys {
                    let _ = simulate(&EventType::KeyPress(*key));
                }
                std::thread::sleep(std::time::Duration::from_millis(15));
                for key in keys.iter().rev() {
                    let _ = simulate(&EventType::KeyRelease(*key));
                }
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
                        WindowAction::Move => {
                            PostMessageW(hwnd, WM_SYSCOMMAND, SC_MOVE + 2, 0);
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
                    RemapCommand::Execute(act, pressed) => execute_action(act, pressed),
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

            if let Some(cfg) = &self.active_scroll {
                if let Some(anchor) = self.scroll_anchor {
                    unsafe {
                        winapi::um::winuser::SetCursorPos(anchor.x, anchor.y);
                    }
                }

                let raw_delta = self.monitor.consume_y_delta();
                if raw_delta != 0 {
                    let mut delta = raw_delta;

                    // Inversion
                    if cfg.invert {
                        delta = -delta;
                    }

                    // Acceleration (Logarithmic-ish)
                    // If accel is on, we square the delta sign-preserving or boost it
                    let mut scale = cfg.sensitivity as i32;
                    if cfg.acceleration {
                        // Simple accel: magnitude * sensitivity * boost
                        // Or just boost sensitivity non-linearly
                        // Let's assume standard accel is just higher multiplier for faster movement
                        if delta.abs() > 5 {
                            scale *= 2;
                        }
                    }

                    // Standard scaling
                    // Base sensitivity 100 = 1.0x
                    // 200 = 2.0x, etc.
                    // div 100 to normalize
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
        // Track pressed keys to ignore repeats
        let pressed_keys = Arc::new(Mutex::new(std::collections::HashSet::new()));

        if let Err(e) = rdev::grab(move |event| {
            if IS_PAUSED.load(Ordering::SeqCst) {
                return Some(event);
            }

            match event.event_type {
                rdev::EventType::KeyPress(key) => {
                    let mut keys = pressed_keys.lock().unwrap();
                    if keys.contains(&key) {
                        return None;
                    }

                    // Safety Guard: If touchpad is not touched, pass through the key (do nothing special)
                    // The user wants the key to "not trigger function" if no touch.
                    // Usually this means "act as normal key".
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
                    // If the key was not in our set, it means we didn't capture the Press
                    // (either because of Safety Guard or it wasn't mapped).
                    // In that case, we MUST pass the Release through to the OS.
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
