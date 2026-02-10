use crate::audit_log;
use crate::platform::touchpad;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

#[cfg(target_os = "windows")]
use crate::platform::touchpad::SendHwnd;

pub struct MonitorState {
    pub is_touched: AtomicBool,
    pub last_touch_time: Mutex<std::time::Instant>,
    pub x_delta: AtomicI32,
    pub y_delta: AtomicI32,
    pub release_delay_ms: AtomicU32,
    pub app_handle: Arc<Mutex<Option<AppHandle>>>,
    #[cfg(target_os = "windows")]
    pub hwnd: Mutex<Option<SendHwnd>>,
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
            last_touch_time: Mutex::new(
                std::time::Instant::now() - std::time::Duration::from_secs(10),
            ),
            x_delta: AtomicI32::new(0),
            y_delta: AtomicI32::new(0),
            release_delay_ms: AtomicU32::new(150),
            app_handle,
            #[cfg(target_os = "windows")]
            hwnd: Mutex::new(None),
            device_activity: Mutex::new(HashMap::new()),
            locked_device: Mutex::new(None),
        });

        let state_clone = Arc::clone(&state);
        std::thread::Builder::new()
            .name("touchpad-monitor".to_string())
            .spawn(move || {
                // Wait for LOG_PATH to be initialized
                for _ in 0..50 {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }

                audit_log("[MONITOR] Monitor thread started after initialization delay");

                if let Err(e) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    touchpad::run_monitor(state_clone);
                })) {
                    audit_log(&format!("[MONITOR] PANIC in monitor thread: {:?}", e));
                }
            })
            .unwrap_or_else(|e| {
                eprintln!("[MONITOR] Failed to spawn monitor thread: {}", e);
                std::thread::spawn(|| {})
            });

        let state_watch = Arc::clone(&state);
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_millis(100));

            if let Ok(h) = state_watch.app_handle.try_lock() {
                if let Some(app) = &*h {
                    let _ = app.emit("monitor-heartbeat", true).ok();

                    let mut activity = state_watch.device_activity.lock().unwrap();

                    // Periodic cleanup to prevent map bloat
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

            let last_touch = { *state_watch.last_touch_time.lock().unwrap() };
            let delay = state_watch.release_delay_ms.load(Ordering::SeqCst) as u64;

            if std::time::Instant::now()
                .duration_since(last_touch)
                .as_millis()
                > delay as u128
            {
                if state_watch.is_touched.swap(false, Ordering::SeqCst) {
                    if let Ok(h) = state_watch.app_handle.try_lock() {
                        if let Some(app) = &*h {
                            let _ = app.emit("touchpad-status", false).ok();
                        }
                    }
                }
            }
        });

        Self { state }
    }

    pub fn scan_and_register(&self) {
        #[cfg(target_os = "windows")]
        {
            let hwnd_wrapped = { *self.state.hwnd.lock().unwrap() };
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
        *self.state.locked_device.lock().unwrap() = device_id;
        audit_log(&format!("[MONITOR] Locked to device: {:?}", device_id));
    }

    pub fn is_touched(&self) -> bool {
        self.state.is_touched.load(Ordering::SeqCst)
    }

    pub fn consume_y_delta(&self) -> i32 {
        self.state.y_delta.swap(0, Ordering::SeqCst)
    }

    pub fn consume_x_delta(&self) -> i32 {
        self.state.x_delta.swap(0, Ordering::SeqCst)
    }
}
