use crate::audit_log;
use std::collections::HashMap;
#[cfg(target_os = "windows")]
use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
#[cfg(target_os = "windows")]
use winapi::shared::windef::HWND;
#[cfg(target_os = "windows")]
use winapi::um::libloaderapi::GetModuleHandleW;
#[cfg(target_os = "windows")]
use winapi::um::winuser::{
    CreateWindowExW, DefWindowProcW, GetRawInputData, RegisterClassW, RegisterRawInputDevices,
    HRAWINPUT, RAWINPUT, RAWINPUTDEVICE, RAWINPUTHEADER, RID_INPUT, RIM_TYPEHID, RIM_TYPEMOUSE,
    WM_INPUT, WNDCLASSW,
};

#[cfg(target_os = "linux")]
use evdev::{AbsoluteAxisType, Device, EventType};

#[cfg(target_os = "windows")]
#[derive(Clone, Copy)]
struct SendHwnd(HWND);
#[cfg(target_os = "windows")]
unsafe impl Send for SendHwnd {}
#[cfg(target_os = "windows")]
unsafe impl Sync for SendHwnd {}

pub struct MonitorState {
    pub is_touched: AtomicBool,
    pub last_touch_time: Mutex<std::time::Instant>,
    pub x_delta: AtomicI32,
    pub y_delta: AtomicI32,
    pub release_delay_ms: AtomicU32,
    pub app_handle: Arc<Mutex<Option<AppHandle>>>,
    #[cfg(target_os = "windows")]
    hwnd: Mutex<Option<SendHwnd>>,
    pub device_activity: Mutex<HashMap<usize, u32>>, // id -> カウント
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
            release_delay_ms: AtomicU32::new(150), // v1.1.0: 安定性のためのバランス (50ms -> 150ms)
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
                // Wait for LOG_PATH to be initialized (set in Tauri's setup hook)
                for _ in 0..50 {
                    // Try to log - if it works, LOG_PATH is ready
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }

                audit_log("[MONITOR] Monitor thread started after initialization delay");

                if let Err(e) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    Self::run_monitor(state_clone);
                })) {
                    audit_log(&format!("[MONITOR] PANIC in monitor thread: {:?}", e));
                }
            })
            .unwrap_or_else(|e| {
                // Can't use audit_log here as LOG_PATH might not be set
                eprintln!("[MONITOR] Failed to spawn monitor thread: {}", e);
                std::thread::spawn(|| {})
            });

        let state_watch = Arc::clone(&state);
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_millis(100)); // レスポンシブな遅延のためにチェック頻度を高くする

            if let Ok(h) = state_watch.app_handle.try_lock() {
                if let Some(app) = &*h {
                    let _ = app.emit("monitor-heartbeat", true).ok();

                    let mut activity = state_watch.device_activity.lock().unwrap();

                    // v1.2.2: 劣化対策 - マップの定期クリーンアップ
                    // デバイスIDが無限に増える（ドライバの挙動等）場合、マップが肥大化してループが遅くなり、
                    // 結果としてスクロールが「徐々に鈍くなり止まる」現象を引き起こす可能性がある。
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

    #[cfg(target_os = "windows")]
    pub fn scan_and_register(&self) {
        unsafe {
            let hwnd_wrapped = { *self.state.hwnd.lock().unwrap() };
            let hwnd = match hwnd_wrapped {
                Some(h) => h.0,
                None => return,
            };

            let mut unique_pairs = std::collections::HashSet::new();
            unique_pairs.insert((0x01, 0x02)); // マウス
                                               // unique_pairs.insert((0x01, 0x06)); // キーボード (診断用!) - 自己トリガー防止のため削除
            unique_pairs.insert((0x0D, 0x04)); // タッチスクリーン
            unique_pairs.insert((0x0D, 0x05)); // タッチパッド

            audit_log("[MONITOR] Registering for Raw Input");

            let mut rids = Vec::new();
            for (p, u) in unique_pairs {
                rids.push(RAWINPUTDEVICE {
                    usUsagePage: p,
                    usUsage: u,
                    dwFlags: winapi::um::winuser::RIDEV_INPUTSINK,
                    hwndTarget: hwnd,
                });
            }
            if RegisterRawInputDevices(
                rids.as_ptr(),
                rids.len() as u32,
                std::mem::size_of::<RAWINPUTDEVICE>() as u32,
            ) == 0
            {
                audit_log("[MONITOR] FAILED to register Raw Input Devices!");
            } else {
                audit_log("[MONITOR] Successfully registered Raw Input Devices");
            }
        }
    }

    #[cfg(target_os = "linux")]
    pub fn scan_and_register(&self) {
        audit_log("[MONITOR] Device scan requested (Linux)");
        // Linux implementation will find and open devices in run_monitor
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

    #[cfg(target_os = "windows")]
    fn run_monitor(state: Arc<MonitorState>) {
        unsafe {
            let h_instance = GetModuleHandleW(null_mut());
            let class_name: Vec<u16> = "TouchpadMonitorClass\0".encode_utf16().collect();
            let wnd_class = WNDCLASSW {
                style: 0,
                lpfnWndProc: Some(Self::wnd_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: h_instance,
                hIcon: null_mut(),
                hCursor: null_mut(),
                hbrBackground: null_mut(),
                lpszMenuName: null_mut(),
                lpszClassName: class_name.as_ptr(),
            };
            RegisterClassW(&wnd_class);

            let hwnd = CreateWindowExW(
                0,
                class_name.as_ptr(),
                class_name.as_ptr(),
                0,
                0,
                0,
                0,
                0,
                null_mut(),
                null_mut(),
                h_instance,
                null_mut(),
            );

            if hwnd.is_null() {
                return;
            }
            {
                *state.hwnd.lock().unwrap() = Some(SendHwnd(hwnd));
            }

            winapi::um::winuser::SetWindowLongPtrW(
                hwnd,
                winapi::um::winuser::GWLP_USERDATA,
                Arc::into_raw(Arc::clone(&state)) as isize,
            );

            // ウィンドウ作成直後にマッピングスキャンをトリガー
            let s_ptr = Arc::into_raw(Arc::clone(&state));
            let s = Arc::from_raw(s_ptr);
            let m = TouchpadMonitor { state: s };
            m.scan_and_register();

            let mut msg = std::mem::zeroed();
            while winapi::um::winuser::GetMessageW(&mut msg, null_mut(), 0, 0) != 0 {
                winapi::um::winuser::TranslateMessage(&msg);
                winapi::um::winuser::DispatchMessageW(&msg);
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn run_monitor(state: Arc<MonitorState>) {
        audit_log("[MONITOR] Starting Linux Monitor Worker");

        loop {
            // Find touchpad devices
            let mut devices = Vec::new();
            if let Ok(dir) = std::fs::read_dir("/dev/input") {
                for entry in dir.flatten() {
                    let path = entry.path();
                    if path.to_string_lossy().contains("event") {
                        match Device::open(&path) {
                            Ok(device) => {
                                let name = device.name().unwrap_or("Unknown").to_lowercase();

                                // Exclusion keywords: skip devices that are clearly not touchpads
                                let exclude_keywords = [
                                    "keyboard",
                                    "kbd",
                                    "mouse",
                                    "button",
                                    "power",
                                    "speaker",
                                    "headphone",
                                    "video",
                                    "camera",
                                    "webcam",
                                    "consumer",
                                    "wireless",
                                    "virtual",
                                ];
                                let is_excluded = exclude_keywords.iter().any(|k| name.contains(k));

                                // Touchpad-specific keywords
                                let touchpad_keywords = [
                                    "touchpad",
                                    "glidepoint",
                                    "trackpad",
                                    "synaptics",
                                    "elan",
                                    "dtp",
                                    "alps",
                                    "focaltech",
                                    "hantick",
                                    "goodix",
                                    "i2c-hid", // Many modern touchpads use i2c
                                    "bcm5974", // Apple MacBook touchpad driver
                                ];
                                let name_match = touchpad_keywords.iter().any(|k| name.contains(k));

                                // Check for absolute axis support with multi-touch (more specific to touchpads)
                                let has_abs =
                                    device.supported_events().contains(EventType::ABSOLUTE)
                                        && device.supported_absolute_axes().map_or(false, |axes| {
                                            // Prefer multi-touch (ABS_MT_*) as it's more specific to touchpads
                                            axes.contains(AbsoluteAxisType::ABS_MT_POSITION_X)
                                                || (axes.contains(AbsoluteAxisType::ABS_X)
                                                    && axes.contains(AbsoluteAxisType::ABS_Y))
                                        });

                                // Only accept if:
                                // 1. Name matches AND not excluded, OR
                                // 2. Name matches (touchpad keyword is strong enough)
                                let is_touchpad = !is_excluded
                                    && (name_match || (has_abs && name.contains("touch")));

                                if is_touchpad {
                                    audit_log(&format!(
                                        "[MONITOR] Found Touchpad: '{}' at {:?} (name_match={}, has_abs={})",
                                        device.name().unwrap_or("Unknown"), path, name_match, has_abs
                                    ));
                                    devices.push(device);
                                } else if has_abs && !is_excluded {
                                    // Log devices that have ABS but weren't selected (for debugging)
                                    audit_log(&format!(
                                        "[MONITOR] Skipped device with ABS: '{}' at {:?}",
                                        device.name().unwrap_or("Unknown"),
                                        path
                                    ));
                                }
                            }
                            Err(e) => {
                                audit_log(&format!(
                                    "[MONITOR] Cannot open {:?}: {} (permission issue?)",
                                    path, e
                                ));
                            }
                        }
                    }
                }
            } else {
                audit_log("[MONITOR] Cannot read /dev/input directory. Permission denied?");
            }

            if devices.is_empty() {
                audit_log("[MONITOR] No touchpad found. Ensure user is in 'input' group. Retrying in 5s...");
                std::thread::sleep(std::time::Duration::from_secs(5));
                continue;
            }

            // Monitor events from all found touchpads
            let mut threads = Vec::new();
            for mut device in devices {
                let s_clone = Arc::clone(&state);
                threads.push(std::thread::spawn(move || {
                    // Track last known absolute position for delta computation
                    let mut last_abs_x: Option<i32> = None;
                    let mut last_abs_y: Option<i32> = None;
                    // Accumulator for smoothing small movements
                    let mut accum_x: i32 = 0;
                    let mut accum_y: i32 = 0;
                    // Direction tracking for hysteresis (-1, 0, 1)
                    let mut last_dir_y: i32 = 0;

                    loop {
                        if let Ok(events) = device.fetch_events() {
                            for ev in events {
                                if ev.event_type() == EventType::ABSOLUTE
                                    || ev.event_type() == EventType::RELATIVE
                                    || ev.event_type() == EventType::KEY
                                {
                                    s_clone.is_touched.store(true, Ordering::SeqCst);
                                    if let Ok(mut last_time) = s_clone.last_touch_time.lock() {
                                        *last_time = std::time::Instant::now();
                                    }

                                    // Handle ABSOLUTE axis events (typical for touchpads)
                                    if ev.event_type() == EventType::ABSOLUTE {
                                        // Constants for filtering
                                        const JUMP_THRESHOLD: i32 = 100; // Detect finger repositioning
                                        const DEADZONE: i32 = 5; // Ignore noise

                                        match ev.code() {
                                            0 => {
                                                // ABS_X
                                                if let Some(last_x) = last_abs_x {
                                                    let delta = ev.value() - last_x;
                                                    if delta.abs() >= JUMP_THRESHOLD {
                                                        // Large jump = finger lift/place, reset tracking
                                                        accum_x = 0;
                                                        accum_y = 0;
                                                        last_dir_y = 0;
                                                    } else if delta.abs() > DEADZONE {
                                                        // Normal movement - simple proportional scroll
                                                        let scaled = delta / 8;
                                                        if scaled != 0 {
                                                            s_clone.x_delta.fetch_add(
                                                                scaled,
                                                                Ordering::SeqCst,
                                                            );
                                                        }
                                                    }
                                                }
                                                last_abs_x = Some(ev.value());
                                            }
                                            1 => {
                                                // ABS_Y
                                                if let Some(last_y) = last_abs_y {
                                                    let delta = ev.value() - last_y;
                                                    if delta.abs() >= JUMP_THRESHOLD {
                                                        // Large jump = finger lift/place, reset tracking
                                                        accum_x = 0;
                                                        accum_y = 0;
                                                        last_dir_y = 0;
                                                    } else if delta.abs() > DEADZONE {
                                                        // Normal movement - simple proportional scroll
                                                        let scaled = delta / 8;
                                                        if scaled != 0 {
                                                            s_clone.y_delta.fetch_add(
                                                                scaled,
                                                                Ordering::SeqCst,
                                                            );
                                                        }
                                                    }
                                                }
                                                last_abs_y = Some(ev.value());
                                            }
                                            _ => {}
                                        }
                                    }

                                    // Handle RELATIVE movement (some touchpads may send this)
                                    if ev.event_type() == EventType::RELATIVE {
                                        match ev.code() {
                                            0 => {
                                                // REL_X
                                                s_clone
                                                    .x_delta
                                                    .fetch_add(ev.value(), Ordering::SeqCst);
                                            }
                                            1 => {
                                                // REL_Y
                                                s_clone
                                                    .y_delta
                                                    .fetch_add(ev.value(), Ordering::SeqCst);
                                            }
                                            _ => {}
                                        }
                                    }

                                    // Reset absolute tracking on finger lift (KEY event with BTN_TOUCH = 0)
                                    if ev.event_type() == EventType::KEY
                                        && ev.code() == 330
                                        && ev.value() == 0
                                    {
                                        // BTN_TOUCH released - reset all tracking state
                                        last_abs_x = None;
                                        last_abs_y = None;
                                        accum_x = 0;
                                        accum_y = 0;
                                        last_dir_y = 0;
                                    }

                                    // UI Update
                                    if let Ok(h) = s_clone.app_handle.try_lock() {
                                        if let Some(app) = &*h {
                                            let _ = app.emit("touchpad-status", true).ok();
                                        }
                                    }
                                }
                            }
                        } else {
                            break; // Device disconnected or error
                        }
                    }
                }));
            }

            // Wait for threads (they should only finish if device is lost)
            for t in threads {
                let _ = t.join();
            }
            audit_log("[MONITOR] Device lost, rescanning...");
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
    }

    #[cfg(target_os = "windows")]
    unsafe extern "system" fn wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: usize,
        lparam: isize,
    ) -> isize {
        if msg == WM_INPUT {
            let mut dw_size: u32 = 0;
            GetRawInputData(
                lparam as HRAWINPUT,
                RID_INPUT,
                null_mut(),
                &mut dw_size,
                std::mem::size_of::<RAWINPUTHEADER>() as u32,
            );
            let mut buffer = vec![0u8; dw_size as usize];
            if GetRawInputData(
                lparam as HRAWINPUT,
                RID_INPUT,
                buffer.as_mut_ptr() as *mut _,
                &mut dw_size,
                std::mem::size_of::<RAWINPUTHEADER>() as u32,
            ) == dw_size
            {
                let raw = &*(buffer.as_ptr() as *const RAWINPUT);
                let state_ptr = winapi::um::winuser::GetWindowLongPtrW(
                    hwnd,
                    winapi::um::winuser::GWLP_USERDATA,
                ) as *mut MonitorState;

                if !state_ptr.is_null() {
                    let state = &*state_ptr;
                    let mut event_detected = false;
                    let device_id = raw.header.hDevice as usize;

                    {
                        let mut activity = state.device_activity.lock().unwrap();
                        let count = activity.entry(device_id).or_insert(0);
                        *count += 1;
                    }

                    let locked = { *state.locked_device.lock().unwrap() };
                    if let Some(target) = locked {
                        if device_id != target {
                            return DefWindowProcW(hwnd, msg, wparam, lparam);
                        }
                    }

                    // 注入された入力を除外 (hDevice == 0 は通常 SendInput を示す)
                    // これにより、シミュレートされたアクションがモニターをトリガーする自己ループを防ぐ
                    if raw.header.hDevice.is_null() {
                        return DefWindowProcW(hwnd, msg, wparam, lparam);
                    }

                    if raw.header.dwType == RIM_TYPEMOUSE {
                        let m = raw.data.mouse();
                        // v1.1.0: 再構築 - 基本的な移動をすべて受け入れる
                        // ノイズフィルタリングは行わず、全ての移動イベントを信頼する
                        if m.lLastX != 0 || m.lLastY != 0 {
                            event_detected = true;
                            // 指右移動 = 正のX、指上移動 = 負のY（画面空間）
                            state.x_delta.fetch_add(m.lLastX, Ordering::SeqCst);
                            // タッチパッドの座標系に合わせて加算
                            state.y_delta.fetch_add(m.lLastY, Ordering::SeqCst);
                        }
                        if m.ulRawButtons != 0 {
                            event_detected = true;
                        }
                    } else if raw.header.dwType == RIM_TYPEHID {
                        // HID (タッチパッド/デジタイザ) のアクティビティ
                        event_detected = true;
                    }

                    if event_detected {
                        state.is_touched.store(true, Ordering::SeqCst);
                        if let Ok(mut last_time) = state.last_touch_time.lock() {
                            *last_time = std::time::Instant::now();
                        }
                        // 頻繁な emit を避けるため、状態が変化したときのみ emit するように改善検討余地あり
                        if let Ok(h) = state.app_handle.try_lock() {
                            if let Some(app) = &*h {
                                let _ = app.emit("touchpad-status", true).ok();
                            }
                        }
                    }
                }
            }
        }
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}
