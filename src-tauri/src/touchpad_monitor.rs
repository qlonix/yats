use crate::platform::touchpad;
use crate::{audit_log, lock_unpoison};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

#[cfg(target_os = "windows")]
use crate::platform::touchpad::SendHwnd;

pub struct MonitorState {
    pub is_touched: AtomicBool,
    pub cursor_freeze: AtomicBool,
    pub last_touch_time: Mutex<std::time::Instant>,
    pub contact_start_time: Mutex<Option<std::time::Instant>>,
    pub x_delta: AtomicI32,
    pub y_delta: AtomicI32,
    pub release_delay_ms: AtomicU32,
    /// トリガー検出方式（Linux）: 0=接触検出, 1=移動検出, 2=自動（両方）
    pub trigger_mode: AtomicU8,
    /// 移動検出: 最後にポインタの移動を観測した時刻
    pub last_movement_time: Mutex<std::time::Instant>,
    pub movement_seen: AtomicBool,
    pub app_handle: Arc<Mutex<Option<AppHandle>>>,
    #[cfg(target_os = "windows")]
    pub hwnd: Mutex<Option<SendHwnd>>,
    /// Windows: デバイスごとの HID 接触状態（device_id → 接触中かどうか）。
    /// contact_start_time の開始・終了はこの遷移だけで管理する。
    #[cfg(target_os = "windows")]
    pub hid_contacts: Mutex<HashMap<usize, bool>>,
    /// Windows: PTP の接触が1つでもアクティブかどうか（接触検出用）
    #[cfg(target_os = "windows")]
    pub contact_active: AtomicBool,
    pub device_activity: Mutex<HashMap<usize, u32>>,
    pub locked_device: Mutex<Option<usize>>,
}

pub struct TouchpadMonitor {
    pub state: Arc<MonitorState>,
}

impl TouchpadMonitor {
    pub fn new(app_handle: Arc<Mutex<Option<AppHandle>>>) -> Self {
        audit_log("[MONITOR] TouchpadMonitor::new() called - initializing");
        let state = Arc::new(MonitorState {
            is_touched: AtomicBool::new(false),
            cursor_freeze: AtomicBool::new(false),
            last_touch_time: Mutex::new(
                std::time::Instant::now() - std::time::Duration::from_secs(10),
            ),
            contact_start_time: Mutex::new(None),
            x_delta: AtomicI32::new(0),
            y_delta: AtomicI32::new(0),
            release_delay_ms: AtomicU32::new(150),
            trigger_mode: AtomicU8::new(0),
            last_movement_time: Mutex::new(std::time::Instant::now()),
            movement_seen: AtomicBool::new(false),
            app_handle,
            #[cfg(target_os = "windows")]
            hwnd: Mutex::new(None),
            #[cfg(target_os = "windows")]
            hid_contacts: Mutex::new(HashMap::new()),
            #[cfg(target_os = "windows")]
            contact_active: AtomicBool::new(false),
            device_activity: Mutex::new(HashMap::new()),
            locked_device: Mutex::new(None),
        });

        let state_clone = Arc::clone(&state);
        std::thread::Builder::new()
            .name("touchpad-monitor".to_string())
            .spawn(move || {
                // 監査ログの出力先が設定されるまで待つ
                for _ in 0..50 {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }

                audit_log("[MONITOR] Monitor thread started after initialization delay");

                loop {
                    let state = Arc::clone(&state_clone);
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        touchpad::run_monitor(state);
                    }));
                    if result.is_ok() {
                        break;
                    }
                    audit_log("[MONITOR] PANIC in monitor thread; restarting in 2s");
                    std::thread::sleep(std::time::Duration::from_secs(2));
                }
            })
            .unwrap_or_else(|e| {
                eprintln!("[MONITOR] Failed to spawn monitor thread: {}", e);
                std::thread::spawn(|| {})
            });

        let state_watch = Arc::clone(&state);
        std::thread::Builder::new()
            .name("monitor-watcher".into())
            .spawn(move || {
                let mut ticks = 0;
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    ticks += 1;
                    if ticks == 600 {
                        audit_log("[HEARTBEAT] watcher alive");
                        ticks = 0;
                    }

                    if let Ok(h) = state_watch.app_handle.try_lock() {
                        if let Some(app) = &*h {
                            let _ = app.emit("monitor-heartbeat", true).ok();

                            let mut activity = lock_unpoison(&state_watch.device_activity);

                            // デバイスの活動記録が増えすぎないようにする
                            if activity.len() > 50 {
                                activity.clear();
                            }

                            let mut sorted: Vec<_> = activity.clone().into_iter().collect();
                            sorted.sort_by(|a, b| b.1.cmp(&a.1));
                            let top: Vec<String> = sorted
                                .iter()
                                .take(5)
                                .map(|(id, count)| format!("0x{:x} ({})", id, count))
                                .collect();
                            let _ = app.emit("top-devices", top).ok();
                        }
                    }

                    let released = {
                        let last_touch = lock_unpoison(&state_watch.last_touch_time);
                        let delay = state_watch.release_delay_ms.load(Ordering::SeqCst);
                        let elapsed = last_touch.elapsed();
                        // 接触終了イベントを取りこぼした場合に備える。
                        if elapsed >= std::time::Duration::from_secs(5) {
                            *lock_unpoison(&state_watch.contact_start_time) = None;
                        }
                        elapsed.as_millis() > delay as u128
                            && state_watch.is_touched.swap(false, Ordering::SeqCst)
                    };
                    if released {
                        if let Ok(h) = state_watch.app_handle.try_lock() {
                            if let Some(app) = &*h {
                                let _ = app.emit("touchpad-status", false).ok();
                            }
                        }
                    }
                }
            })
            .expect("Failed to spawn monitor watcher thread");

        // 移動検出用にポインタ位置を監視する（Linux のみ）。
        // 接触検出モードでは何もしない。
        #[cfg(target_os = "linux")]
        {
            let state_poll = Arc::clone(&state);
            std::thread::Builder::new()
                .name("pointer-poller".into())
                .spawn(move || {
                    let mut last_pos: Option<(i32, i32)> = None;
                    loop {
                        std::thread::sleep(std::time::Duration::from_millis(20));
                        if state_poll.trigger_mode.load(Ordering::SeqCst) == 0 {
                            continue;
                        }
                        if let Some(pos) = crate::platform::pointer_position() {
                            if last_pos != Some(pos) {
                                last_pos = Some(pos);
                                *lock_unpoison(&state_poll.last_movement_time) =
                                    std::time::Instant::now();
                                state_poll.movement_seen.store(true, Ordering::SeqCst);
                            }
                        }
                    }
                })
                .ok();
        }

        Self { state }
    }

    pub fn scan_and_register(&self) {
        #[cfg(target_os = "windows")]
        {
            let hwnd_wrapped = { *lock_unpoison(&self.state.hwnd) };
            if let Some(hwnd) = hwnd_wrapped {
                touchpad::scan_and_register(&self.state, hwnd.0);
            }
        }

        #[cfg(target_os = "linux")]
        {
            touchpad::scan_and_register(&self.state);
        }
    }

    pub fn lock_device(&self, device_id: Option<usize>) {
        *lock_unpoison(&self.state.locked_device) = device_id;
        audit_log(&format!("[MONITOR] Locked to device: {:?}", device_id));
    }

    pub fn is_touched(&self) -> bool {
        match self.state.trigger_mode.load(Ordering::SeqCst) {
            // 移動検出
            1 => self.movement_state(),
            // 自動: 接触検出と移動検出のどちらか
            2 => self.contact_state() || self.movement_state(),
            // 接触検出
            _ => self.contact_state(),
        }
    }

    /// 接触検出の状態。Linux は evdev モニタの接触フラグ、Windows は
    /// PTP の接触状態（Confidence ありの接触が1つでもあるか）。
    #[cfg(target_os = "linux")]
    fn contact_state(&self) -> bool {
        self.state.is_touched.load(Ordering::SeqCst)
    }

    #[cfg(target_os = "windows")]
    fn contact_state(&self) -> bool {
        self.state.contact_active.load(Ordering::SeqCst)
    }

    /// 移動検出の状態。Linux はポインタ位置の監視（Release Delay 以内）、
    /// Windows は Raw Input のマウス活動で立てる従来のフラグ。
    #[cfg(target_os = "linux")]
    fn movement_state(&self) -> bool {
        self.movement_recent()
    }

    #[cfg(target_os = "windows")]
    fn movement_state(&self) -> bool {
        self.state.is_touched.load(Ordering::SeqCst)
    }

    #[cfg(target_os = "linux")]
    fn movement_recent(&self) -> bool {
        if !self.state.movement_seen.load(Ordering::SeqCst) {
            return false;
        }
        let delay = self.state.release_delay_ms.load(Ordering::SeqCst) as u128;
        lock_unpoison(&self.state.last_movement_time)
            .elapsed()
            .as_millis()
            <= delay
    }

    /// 現在の接触が始まった時刻。接触がなければ None。
    /// 接触ごとに異なる値になるため、「同じ接触が続いているか」の判定にも使える。
    pub fn contact_started_at(&self) -> Option<std::time::Instant> {
        *lock_unpoison(&self.state.contact_start_time)
    }

    /// 前回の呼び出し以降に蓄積したカーソル移動量（|x| + |y|）を取り出す。
    /// 「狙ってスライドしてから押した」意図的な操作と、タイピングの
    /// 続きの打鍵とを区別するために使う。
    pub fn take_movement(&self) -> i32 {
        let x = self.state.x_delta.swap(0, Ordering::SeqCst);
        let y = self.state.y_delta.swap(0, Ordering::SeqCst);
        x.abs() + y.abs()
    }

    pub fn consume_y_delta(&self) -> i32 {
        self.state.y_delta.swap(0, Ordering::SeqCst)
    }

    pub fn consume_x_delta(&self) -> i32 {
        self.state.x_delta.swap(0, Ordering::SeqCst)
    }
}
