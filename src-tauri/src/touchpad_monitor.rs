use crate::platform::touchpad;
use crate::{audit_log, lock_unpoison};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};
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
    /// 直近の移動イベント履歴（時刻, |delta|）。タイピング中の置き手と
    /// 意図した操作を「直前に動かしたか」で見分けるために使う（Linux のみ記録）。
    pub movement_events: Mutex<VecDeque<(std::time::Instant, i32)>>,
    pub release_delay_ms: AtomicU32,
    pub app_handle: Arc<Mutex<Option<AppHandle>>>,
    #[cfg(target_os = "windows")]
    pub hwnd: Mutex<Option<SendHwnd>>,
    /// Windows: デバイスごとの HID 接触状態（device_id → 接触中かどうか）。
    /// contact_start_time の開始・終了はこの遷移だけで管理する。
    #[cfg(target_os = "windows")]
    pub hid_contacts: Mutex<HashMap<usize, bool>>,
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
            movement_events: Mutex::new(VecDeque::new()),
            release_delay_ms: AtomicU32::new(150),
            app_handle,
            #[cfg(target_os = "windows")]
            hwnd: Mutex::new(None),
            #[cfg(target_os = "windows")]
            hid_contacts: Mutex::new(HashMap::new()),
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
        self.state.is_touched.load(Ordering::SeqCst)
    }

    pub fn contact_started_at(&self) -> Option<std::time::Instant> {
        *lock_unpoison(&self.state.contact_start_time)
    }

    /// from から to の間の移動量合計（デバイス単位）。
    /// 「最後のキー入力からキー押下までの間にカーソル移動があったか」を
    /// 測るために使う。履歴は直近 3.5 秒分を保持する。
    pub fn movement_between(
        &self,
        from: std::time::Instant,
        to: std::time::Instant,
    ) -> i32 {
        let now = std::time::Instant::now();
        let mut events = lock_unpoison(&self.state.movement_events);
        while let Some(&(time, _)) = events.front() {
            if now.duration_since(time).as_millis() > 3500 {
                events.pop_front();
            } else {
                break;
            }
        }
        events
            .iter()
            .filter(|&&(time, _)| time > from && time < to)
            .map(|&(_, amount)| amount)
            .sum()
    }

    pub fn consume_y_delta(&self) -> i32 {
        self.state.y_delta.swap(0, Ordering::SeqCst)
    }

    pub fn consume_x_delta(&self) -> i32 {
        self.state.x_delta.swap(0, Ordering::SeqCst)
    }
}
