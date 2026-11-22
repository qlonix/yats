use crate::config::{Action, AppConfig, ScrollConfig, WindowAction};
use crate::touchpad_monitor::TouchpadMonitor;
use crate::{lock_unpoison, platform, read_unpoison};
use rdev::{simulate, EventType};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, RwLock};
use tauri::{AppHandle, Emitter, Manager};

pub static IS_PAUSED: AtomicBool = AtomicBool::new(false);
pub static CURVE_MONITORING: AtomicBool = AtomicBool::new(false);
pub(crate) static CURVE_MONITORING_LOCK: Mutex<()> = Mutex::new(());
pub static IS_SIMULATING: AtomicBool = AtomicBool::new(false);

#[derive(Clone, serde::Serialize)]
struct ScrollActivity {
    input: f32,
    output: f32,
}

pub struct KeyboardHook {
    monitor: Arc<TouchpadMonitor>,
    config: Arc<RwLock<AppConfig>>,
    tx: Sender<RemapCommand>,
    last_typing_time: Arc<Mutex<Option<std::time::Instant>>>,
    typing_mode: Arc<Mutex<TypingMode>>,
}

/// タイピングモード（パームリジェクション）の状態。
/// 接触しながら一定時間タイプし続けたらモードに入り、その間はトリガー
/// キーも通常のキーとして通す。「考える間」で打鍵が数秒空いても Palm
/// Delay のような時間切れで抜けないことが目的。
/// 実機のタイピングでは置いた手の接触が細かく切れたり再開したりする
/// （フリッカー）ため、数える打鍵は「接触中の打鍵」だけとしつつ、接触が
/// ない打鍵では数え直しにしない。解除は次のいずれか:
/// - カーソルを大きく動かしてから押した（狙った操作とみなす）
/// - 接触が終わり、かつ直近に数えた打鍵から間が空いた
/// - 10 秒間打鍵がない（固着防止の上限）
/// 短いコマンド入力では入らないよう、突入条件は「間隔 1.5 秒以内の打鍵が
/// 8 回以上、かつエピソード開始から 3 秒以上」としている。
struct TypingMode {
    active: bool,
    /// モード突入時の接触の開始時刻。
    bound_contact: Option<std::time::Instant>,
    episode_start: Option<std::time::Instant>,
    /// 最後に「数えた」（接触中の）打鍵の時刻。
    episode_last_counted: Option<std::time::Instant>,
    episode_presses: usize,
    /// 種別を問わない最後の打鍵の時刻（10 秒の上限判定用）。
    last_press: Option<std::time::Instant>,
}

/// これ以上カーソルが動いていたら「狙った操作」とみなす移動量。
const TYPING_MODE_MOVE_THRESHOLD: i32 = 200;

impl TypingMode {
    fn new() -> Self {
        Self {
            active: false,
            bound_contact: None,
            episode_start: None,
            episode_last_counted: None,
            episode_presses: 0,
            last_press: None,
        }
    }

    fn reset_episode(&mut self) {
        self.episode_start = None;
        self.episode_last_counted = None;
        self.episode_presses = 0;
    }

    fn deactivate(&mut self, reason: &str) {
        self.active = false;
        self.bound_contact = None;
        self.reset_episode();
        crate::audit_log(&format!("[PALM] typing mode off ({reason})"));
    }

    /// キー押下のたびに呼び、タイピングモード中なら true を返す。
    /// `contact` は現在の接触の開始時刻（接触がなければ None）、
    /// `movement` は前回の打鍵以降のカーソル移動量。
    fn on_press(
        &mut self,
        now: std::time::Instant,
        contact: Option<std::time::Instant>,
        movement: i32,
    ) -> bool {
        // 前回「数えた」打鍵の時刻（この打鍵で上書きする前に控える）
        let prev_counted = self.episode_last_counted;

        if self.active {
            let timed_out = self
                .last_press
                .is_some_and(|t| now.duration_since(t).as_millis() > 10_000);
            if timed_out {
                self.deactivate("timeout");
            } else if movement >= TYPING_MODE_MOVE_THRESHOLD {
                self.deactivate(&format!("cursor moved {movement}"));
            } else if contact != self.bound_contact {
                // 接触が終わった／別の接触に変わった。ただし数えた打鍵が
                // 直近なら、フリッカーの最中とみなして継続する
                let composing = prev_counted
                    .is_some_and(|t| now.duration_since(t).as_millis() <= 1500);
                if composing {
                    if contact.is_some() {
                        self.bound_contact = contact;
                    }
                } else {
                    self.deactivate("contact ended");
                }
            }
        }

        self.last_press = Some(now);
        if contact.is_some() {
            self.episode_last_counted = Some(now);
        }
        if self.active {
            return true;
        }

        // 突入判定は「接触しながらの打鍵」だけを数える（接触なしの
        // タイピングの直後に触れて押した、という正当な操作を巻き込まない）。
        // 接触がない打鍵では数え直しにしない（フリッカー対策）。
        if contact.is_none() {
            return false;
        }
        match prev_counted {
            Some(last)
                if self.episode_presses > 0
                    && now.duration_since(last).as_millis() <= 1500 =>
            {
                self.episode_presses += 1;
            }
            _ => {
                self.episode_start = Some(now);
                self.episode_presses = 1;
            }
        }

        if self.episode_presses >= 8 {
            if let Some(start) = self.episode_start {
                if now.duration_since(start).as_millis() >= 3000 {
                    self.active = true;
                    self.bound_contact = contact;
                    self.reset_episode();
                    crate::audit_log("[PALM] typing mode on");
                    return true;
                }
            }
        }
        false
    }
}

#[derive(Clone, Debug)]
pub enum RemapCommand {
    Execute(Action, bool),
    UpdateScroll(Option<ScrollConfig>),
    UpdateDrag(bool),
}

const VK_BROWSER_BACK: u16 = 0xA6;
const VK_BROWSER_FORWARD: u16 = 0xA7;

fn execute_action(action: Action, pressed: bool, app: &Mutex<Option<AppHandle>>) {
    match action {
        Action::MouseClick(mb) => {
            if pressed {
                platform::send_mouse_click(&mb, true);
            } else {
                platform::send_mouse_click(&mb, false);
            }
        }
        Action::MouseDoubleClick(mb) => {
            if pressed {
                platform::send_mouse_click(&mb, true);
                platform::send_mouse_click(&mb, false);
                std::thread::sleep(std::time::Duration::from_millis(50));
                platform::send_mouse_click(&mb, true);
            } else {
                platform::send_mouse_click(&mb, false);
            }
        }
        Action::MouseScroll(_) => {}
        Action::KeyMacro(steps) => {
            if pressed {
                IS_SIMULATING.store(true, Ordering::SeqCst);
                for step in steps {
                    for key in step.iter().cloned() {
                        let _ = simulate(&EventType::KeyPress(key));
                    }
                    std::thread::sleep(std::time::Duration::from_millis(60));
                    for key in step.iter().rev().cloned() {
                        let _ = simulate(&EventType::KeyRelease(key));
                    }
                    std::thread::sleep(std::time::Duration::from_millis(40));
                }
                IS_SIMULATING.store(false, Ordering::SeqCst);
            }
        }
        Action::Window(win_act) if pressed => {
            if matches!(win_act, WindowAction::Close) && platform::is_active_window_own() {
                let app = lock_unpoison(app).clone();
                if let Some(app) = app {
                    let main_thread_app = app.clone();
                    let _ = app.run_on_main_thread(move || {
                        if let Some(window) = main_thread_app.get_webview_window("main") {
                            if window.hide().is_ok() {
                                crate::audit_log(
                                    "[HOOK] Close Window の対象が YATS 自身のため、外部クローズではなくウィンドウを隠した",
                                );
                            }
                        }
                    });
                    return;
                }
            }
            platform::execute_window_action(win_act);
        }
        Action::Window(_) => {}
        Action::WindowDrag => {}
        Action::BrowserBack => {
            platform::send_key_click(VK_BROWSER_BACK, pressed);
        }
        Action::BrowserForward => {
            platform::send_key_click(VK_BROWSER_FORWARD, pressed);
        }
    }
}

/// スクロール蓄積量からプラットフォームの送信単位への換算係数。
/// Windows のホイール量は WHEEL_DELTA (120) で 1 ノッチで、ノッチ未満の量も
/// 送れる。そのため Windows では 120 倍の解像度で蓄積し、細かい動きでも
/// 少しずつスクロールできるようにする。Linux も uinput の高解像度ホイール
/// （REL_WHEEL_HI_RES、120 で 1 ノッチ）で同じ粒度で送れるため 120 倍に
/// 統一する（送れない環境では platform 側でクリック単位に丸めて送る）。
#[cfg(any(target_os = "windows", target_os = "linux"))]
const SCROLL_SEND_SCALE: f32 = 120.0;
#[cfg(not(any(target_os = "windows", target_os = "linux")))]
const SCROLL_SEND_SCALE: f32 = 1.0;

pub struct HookWorker {
    rx: Receiver<RemapCommand>,
    active_scroll: Option<ScrollConfig>,
    monitor: Arc<TouchpadMonitor>,
    config: Arc<RwLock<AppConfig>>,
    app: Arc<Mutex<Option<AppHandle>>>,
    scroll_anchor: Option<(i32, i32)>,
    accumulator_y: f32,
    pending_y: f32,
    last_scroll_time: std::time::Instant,
    drag_active: bool,
    last_drag_time: std::time::Instant,
    last_activity_emit: std::time::Instant,
}

impl HookWorker {
    pub fn new(
        rx: Receiver<RemapCommand>,
        monitor: Arc<TouchpadMonitor>,
        config: Arc<RwLock<AppConfig>>,
        app: Arc<Mutex<Option<AppHandle>>>,
    ) -> Self {
        Self {
            rx,
            active_scroll: None,
            monitor,
            config,
            app,
            scroll_anchor: None,
            accumulator_y: 0.0,
            pending_y: 0.0,
            last_scroll_time: std::time::Instant::now(),
            drag_active: false,
            last_drag_time: std::time::Instant::now(),
            last_activity_emit: std::time::Instant::now(),
        }
    }

    pub fn run(&mut self) {
        loop {
            while let Ok(cmd) = self.rx.try_recv() {
                match cmd {
                    RemapCommand::Execute(act, pressed) => {
                        execute_action(act, pressed, &self.app);
                    }
                    RemapCommand::UpdateScroll(cfg) => {
                        self.active_scroll = cfg;
                        if self.active_scroll.is_some() {
                            self.scroll_anchor = platform::get_cursor_pos();
                            if let Some(anchor) = self.scroll_anchor {
                                platform::cursor_freeze_begin(&self.monitor, anchor);
                            }
                        } else {
                            platform::cursor_freeze_end(&self.monitor);
                            self.scroll_anchor = None;
                        }
                        self.accumulator_y = 0.0;
                        self.pending_y = 0.0;
                        self.last_scroll_time = std::time::Instant::now();
                    }
                    RemapCommand::UpdateDrag(start) => {
                        if start {
                            self.drag_active = platform::begin_window_drag();
                            self.last_drag_time = std::time::Instant::now();
                        } else {
                            if self.drag_active {
                                platform::end_window_drag();
                            }
                            self.drag_active = false;
                        }
                    }
                }
            }

            if let Some(key_cfg) = &self.active_scroll {
                if let Some((x, y)) = self.scroll_anchor {
                    platform::set_cursor_pos(x, y);
                }

                self.pending_y += self.monitor.consume_y_delta() as f32;
                let now = std::time::Instant::now();
                let interval = now.duration_since(self.last_scroll_time);
                if interval >= std::time::Duration::from_millis(8) {
                    if self.pending_y != 0.0 {
                        let cfg = read_unpoison(&self.config);
                        let elapsed = interval.as_secs_f32().max(0.001);
                        self.accumulator_y += compute_scroll_addition(
                            self.pending_y,
                            elapsed,
                            cfg.scroll_sensitivity,
                            cfg.scroll_invert,
                            cfg.scroll_speed,
                            cfg.linux_min_scroll_speed,
                            cfg.linux_max_scroll_speed,
                            cfg.linux_use_scroll_curve,
                            &cfg.linux_scroll_curve,
                            key_cfg,
                        ) * SCROLL_SEND_SCALE;
                        if cfg.linux_use_scroll_curve
                            && CURVE_MONITORING.load(Ordering::SeqCst)
                            && now.duration_since(self.last_activity_emit)
                                >= std::time::Duration::from_millis(50)
                        {
                            let input = self.pending_y.abs() / elapsed;
                            let output = interpolate_curve(&cfg.linux_scroll_curve, input);
                            drop(cfg);
                            if let Some(app) = lock_unpoison(&self.app).as_ref() {
                                let _monitoring_guard = lock_unpoison(&CURVE_MONITORING_LOCK);
                                if CURVE_MONITORING.load(Ordering::SeqCst) {
                                    let _ = app
                                        .emit("scroll-activity", ScrollActivity { input, output });
                                    self.last_activity_emit = std::time::Instant::now();
                                }
                            }
                        }
                    }
                    self.pending_y = 0.0;
                    let output = self.accumulator_y.trunc() as i32;
                    if output != 0 {
                        platform::send_mouse_scroll(output);
                        self.accumulator_y -= output as f32;
                    }
                    self.last_scroll_time = now;
                }
            } else if self.drag_active {
                // 移動量はタッチパッドの生デルタではなくカーソルの実移動量に
                // 合わせる（platform 側でカーソル位置の差分を取る）。
                let now = std::time::Instant::now();
                if now.duration_since(self.last_drag_time) >= std::time::Duration::from_millis(8) {
                    platform::move_dragged_window();
                    self.last_drag_time = now;
                }
            } else {
                self.monitor.consume_x_delta();
                self.monitor.consume_y_delta();
                self.accumulator_y = 0.0;
                self.pending_y = 0.0;
            }

            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
}

impl KeyboardHook {
    pub fn new(
        monitor: Arc<TouchpadMonitor>,
        config: Arc<RwLock<AppConfig>>,
        app: Arc<Mutex<Option<AppHandle>>>,
    ) -> Self {
        let (tx, rx) = channel();
        let mon_clone = Arc::clone(&monitor);
        let cfg_clone = Arc::clone(&config);
        std::thread::Builder::new()
            .name("hook-worker".into())
            .spawn(move || {
                let mut worker = HookWorker::new(rx, mon_clone, cfg_clone, app);
                worker.run();
            })
            .expect("Failed to spawn hook worker thread");

        Self {
            monitor,
            config,
            tx,
            last_typing_time: Arc::new(Mutex::new(None)),
            typing_mode: Arc::new(Mutex::new(TypingMode::new())),
        }
    }

    pub fn start(&self) {
        #[cfg(not(target_os = "linux"))]
        {
            crate::audit_log("[HOOK] Starting keyboard hook (rdev::grab)...");
            let tx = self.tx.clone();
            let config_ref = Arc::clone(&self.config);
            let monitor_ref = Arc::clone(&self.monitor);
            let last_typing_ref = Arc::clone(&self.last_typing_time);
            let typing_mode_ref = Arc::clone(&self.typing_mode);

            loop {
                let tx_inner = tx.clone();
                let config_inner = Arc::clone(&config_ref);
                let monitor_inner = Arc::clone(&monitor_ref);
                let last_typing_inner = Arc::clone(&last_typing_ref);
                let typing_mode_inner = Arc::clone(&typing_mode_ref);
                let pressed_keys = Arc::new(Mutex::new(std::collections::HashSet::new()));

                crate::audit_log("[HOOK] Calling rdev::grab...");
                if let Err(e) = rdev::grab(move |event| {
                    if IS_PAUSED.load(Ordering::SeqCst) || IS_SIMULATING.load(Ordering::SeqCst) {
                        return Some(event);
                    }

                    match event.event_type {
                        rdev::EventType::KeyPress(key) => {
                            let mut keys = lock_unpoison(&pressed_keys);
                            if keys.contains(&key) {
                                return None;
                            }

                            let now = std::time::Instant::now();
                            let palm_reason = {
                                let cfg = read_unpoison(&config_inner);
                                if cfg.palm_rejection
                                    && lock_unpoison(&typing_mode_inner).on_press(
                                        now,
                                        monitor_inner.contact_started_at(),
                                        monitor_inner.take_movement(),
                                    )
                                {
                                    Some("typing mode")
                                } else {
                                    palm_rejection_reason(
                                        &cfg,
                                        *lock_unpoison(&last_typing_inner),
                                        now,
                                    )
                                }
                            };
                            let is_palm_rejected = palm_reason.is_some();

                            let touched = monitor_inner.is_touched();
                            let action = {
                                let cfg = read_unpoison(&config_inner);
                                cfg.mappings.get(&key).cloned()
                            };

                            let should_trigger = action.is_some() && touched && !is_palm_rejected;

                            if should_trigger {
                                // 意図した操作が発火したら、タイピングの
                                // エピソードはそこで終わったものとする
                                lock_unpoison(&typing_mode_inner).reset_episode();
                                keys.insert(key);
                                if let Some(action) = action {
                                    crate::audit_log(&format!("[TRIGGER] key={key:?} fired"));
                                    match action {
                                        Action::MouseScroll(cfg) => {
                                            monitor_inner.consume_y_delta();
                                            let _ = tx_inner
                                                .send(RemapCommand::UpdateScroll(Some(cfg)));
                                        }
                                        Action::WindowDrag => {
                                            monitor_inner.consume_x_delta();
                                            monitor_inner.consume_y_delta();
                                            let _ = tx_inner.send(RemapCommand::UpdateDrag(true));
                                        }
                                        _ => {
                                            let _ =
                                                tx_inner.send(RemapCommand::Execute(action, true));
                                        }
                                    }
                                    return None;
                                }
                            } else {
                                if action.is_some() && touched {
                                    if let Some(reason) = palm_reason {
                                        crate::audit_log(&format!(
                                            "[PALM] key={key:?} rejected ({reason})"
                                        ));
                                    }
                                }
                                *lock_unpoison(&last_typing_inner) = Some(now);
                            }
                        }
                        rdev::EventType::KeyRelease(key) => {
                            let mut keys = lock_unpoison(&pressed_keys);
                            if !keys.remove(&key) {
                                *lock_unpoison(&last_typing_inner) =
                                    Some(std::time::Instant::now());
                                return Some(event);
                            }

                            let action = {
                                let cfg = read_unpoison(&config_inner);
                                cfg.mappings.get(&key).cloned()
                            };

                            if let Some(action) = action {
                                match action {
                                    Action::MouseScroll(_) => {
                                        let _ = tx_inner.send(RemapCommand::UpdateScroll(None));
                                    }
                                    Action::WindowDrag => {
                                        let _ = tx_inner.send(RemapCommand::UpdateDrag(false));
                                    }
                                    _ => {
                                        let _ = tx_inner.send(RemapCommand::Execute(action, false));
                                    }
                                }
                                return None;
                            }
                        }
                        _ => {}
                    }
                    Some(event)
                }) {
                    crate::audit_log(&format!(
                        "[HOOK] rdev::grab error: {:?}. Retrying in 3s...",
                        e
                    ));
                    eprintln!("[HOOK] Error: {:?}. Retrying...", e);
                }
                crate::audit_log("[HOOK] Keyboard hook exited, restarting in 3s...");
                std::thread::sleep(std::time::Duration::from_secs(3));
            }
        }

        #[cfg(target_os = "linux")]
        {
            crate::audit_log("[HOOK] Starting Linux Keyboard Hook (evdev + uinput)...");
            let tx = self.tx.clone();
            let config_ref = Arc::clone(&self.config);
            let monitor_ref = Arc::clone(&self.monitor);

            let last_typing_ref = Arc::clone(&self.last_typing_time);
            let typing_mode_ref = Arc::clone(&self.typing_mode);

            loop {
                // Find all keyboards
                let mut devices = Vec::new();
                if let Ok(dir) = std::fs::read_dir("/dev/input") {
                    for entry in dir.flatten() {
                        let path = entry.path();
                        if path.to_string_lossy().contains("event") {
                            if let Ok(mut device) = evdev::Device::open(&path) {
                                let caps = device.supported_events();
                                let name = device.name().unwrap_or("Unknown").to_lowercase();

                                // Only true keyboards: must have KEY events, support standard typing keys (like KEY_A and KEY_ENTER),
                                // and name must not contain touchpad or mouse.
                                let is_keyboard = caps.contains(evdev::EventType::KEY)
                                    && !name.contains("touchpad")
                                    && !name.contains("mouse")
                                    && device.supported_keys().map_or(false, |keys| {
                                        keys.contains(evdev::Key::KEY_A)
                                            && keys.contains(evdev::Key::KEY_ENTER)
                                    });

                                if is_keyboard {
                                    // Grab it!
                                    if device.grab().is_ok() {
                                        crate::audit_log(&format!(
                                            "[HOOK] Grabbed keyboard: {}",
                                            name
                                        ));
                                        devices.push(device);
                                    }
                                }
                            }
                        }
                    }
                }

                if devices.is_empty() {
                    crate::audit_log("[HOOK] No keyboard found to grab. Retrying in 5s...");
                    std::thread::sleep(std::time::Duration::from_secs(5));
                    continue;
                }

                // Create a single virtual uinput device for all keyboards
                let uinput_builder = match evdev::uinput::VirtualDeviceBuilder::new() {
                    Ok(builder) => builder.name("YATS Virtual Keyboard"),
                    Err(e) => {
                        crate::audit_log(&format!(
                            "[HOOK] Failed to init uinput builder: {}. Please check /dev/uinput permissions.",
                            e
                        ));
                        eprintln!("[HOOK] /dev/uinput permission denied. Please add udev rules.");
                        // Drop grabbed devices gracefully before sleeping
                        drop(devices);
                        std::thread::sleep(std::time::Duration::from_secs(5));
                        continue;
                    }
                };

                // Copy capabilities from all grabbed devices
                let mut keys = evdev::AttributeSet::<evdev::Key>::default();
                for d in &devices {
                    if let Some(set) = d.supported_keys() {
                        for k in set.iter() {
                            keys.insert(k);
                        }
                    }
                }

                let uinput_builder = match uinput_builder.with_keys(&keys) {
                    Ok(builder) => builder,
                    Err(e) => {
                        crate::audit_log(&format!(
                            "[HOOK] Failed to add keys to uinput builder: {e}"
                        ));
                        drop(devices);
                        std::thread::sleep(std::time::Duration::from_secs(5));
                        continue;
                    }
                };

                let virtual_device = match uinput_builder.build() {
                    Ok(dev) => dev,
                    Err(e) => {
                        crate::audit_log(&format!("[HOOK] Failed to build uinput device: {}", e));
                        eprintln!("[HOOK] uinput build failed: {}", e);
                        drop(devices);
                        std::thread::sleep(std::time::Duration::from_secs(5));
                        continue;
                    }
                };

                let mut threads = Vec::new();
                let virtual_device = Arc::new(Mutex::new(virtual_device));

                for mut device in devices {
                    let tx_inner = tx.clone();
                    let config_inner = Arc::clone(&config_ref);
                    let monitor_inner = Arc::clone(&monitor_ref);
                    let last_typing_inner = Arc::clone(&last_typing_ref);
                    let typing_mode_inner = Arc::clone(&typing_mode_ref);
                    let v_dev = Arc::clone(&virtual_device);

                    threads.push(std::thread::spawn(move || {
                        let mut pressed_keys = std::collections::HashMap::new();
                        loop {
                            let events = match device.fetch_events() {
                                Ok(it) => it.collect::<Vec<_>>(),
                                Err(_) => break,
                            };

                            for ev in events {
                                if ev.event_type() == evdev::EventType::KEY {
                                    let code = ev.code(); // u16
                                    let value = ev.value(); // i32: 0, 1, or 2

                                    // Map evdev code to rdev Key
                                    if let Some(rkey) = evdev_to_rdev_key(code) {
                                        match value {
                                            1 => {
                                                // Press
                                                let now = std::time::Instant::now();
                                                let palm_reason = {
                                                    let cfg = read_unpoison(&config_inner);
                                                    if cfg.palm_rejection
                                                        && lock_unpoison(&typing_mode_inner)
                                                            .on_press(
                                                                now,
                                                                monitor_inner
                                                                    .contact_started_at(),
                                                                monitor_inner
                                                                    .take_movement(),
                                                            )
                                                    {
                                                        Some("typing mode")
                                                    } else {
                                                        palm_rejection_reason(
                                                            &cfg,
                                                            *lock_unpoison(
                                                                &last_typing_inner,
                                                            ),
                                                            now,
                                                        )
                                                    }
                                                };
                                                let is_palm_rejected = palm_reason.is_some();

                                                let is_remap = {
                                                    let cfg = read_unpoison(&config_inner);
                                                    cfg.mappings.contains_key(&rkey)
                                                };

                                                let touched = monitor_inner.is_touched();
                                                let paused = IS_PAUSED.load(Ordering::SeqCst);
                                                let simulating =
                                                    IS_SIMULATING.load(Ordering::SeqCst);

                                                let should_trigger = is_remap
                                                    && touched
                                                    && !paused
                                                    && !simulating
                                                    && !is_palm_rejected;

                                                if should_trigger {
                                                    // 意図した操作が発火したら、タイピングの
                                                    // エピソードはそこで終わったものとする
                                                    lock_unpoison(&typing_mode_inner)
                                                        .reset_episode();
                                                    let action = {
                                                        let cfg = read_unpoison(&config_inner);
                                                        cfg.mappings.get(&rkey).cloned()
                                                    };
                                                    if let Some(action) = action {
                                                        crate::audit_log(&format!(
                                                            "[TRIGGER] key={rkey:?} fired"
                                                        ));
                                                        pressed_keys.insert(rkey, action.clone());
                                                        match action {
                                                            Action::MouseScroll(cfg) => {
                                                                monitor_inner.consume_y_delta();
                                                                let _ = tx_inner.send(
                                                                    RemapCommand::UpdateScroll(
                                                                        Some(cfg),
                                                                    ),
                                                                );
                                                            }
                                                            Action::WindowDrag => {
                                                                monitor_inner.consume_x_delta();
                                                                monitor_inner.consume_y_delta();
                                                                let _ = tx_inner
                                                                    .send(RemapCommand::UpdateDrag(true));
                                                            }
                                                            _ => {
                                                                let _ = tx_inner.send(
                                                                    RemapCommand::Execute(
                                                                        action, true,
                                                                    ),
                                                                );
                                                            }
                                                        }
                                                        // Consume event (do not pass through)
                                                        continue;
                                                    }
                                                } else {
                                                    if is_remap && touched && !paused && !simulating
                                                    {
                                                        if let Some(reason) = palm_reason {
                                                            crate::audit_log(&format!(
                                                                "[PALM] key={rkey:?} rejected ({reason})"
                                                            ));
                                                        }
                                                    }
                                                    *lock_unpoison(&last_typing_inner) = Some(now);
                                                }
                                            }
                                            0 => {
                                                // Release
                                                if let Some(action) = pressed_keys.remove(&rkey) {
                                                    match action {
                                                        Action::MouseScroll(_) => {
                                                            let _ = tx_inner.send(
                                                                RemapCommand::UpdateScroll(None),
                                                            );
                                                        }
                                                        Action::WindowDrag => {
                                                            let _ = tx_inner
                                                                .send(RemapCommand::UpdateDrag(false));
                                                        }
                                                        _ => {
                                                            let _ = tx_inner.send(
                                                                RemapCommand::Execute(
                                                                    action, false,
                                                                ),
                                                            );
                                                        }
                                                    }
                                                    // Consume event (do not pass through)
                                                    continue;
                                                } else {
                                                    *lock_unpoison(&last_typing_inner) =
                                                        Some(std::time::Instant::now());
                                                }
                                            }
                                            2 => {
                                                // Repeat
                                                if pressed_keys.contains_key(&rkey) {
                                                    // Consume event (do not pass through)
                                                    continue;
                                                } else {
                                                    *lock_unpoison(&last_typing_inner) =
                                                        Some(std::time::Instant::now());
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                                // Forward to virtual device
                                let mut v = lock_unpoison(&v_dev);
                                let _ = v.emit(&[ev]);
                            }
                        }
                    }));
                }

                for t in threads {
                    let _ = t.join();
                }
                crate::audit_log("[HOOK] Keyboards lost or error, rescanning...");
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
        }
    }
}

/// パームリジェクションで抑止する場合はその理由を返す（抑止しない場合は None）。
/// ここでは「直前のタイピングによる抑止 (recent typing)」だけを行う。
/// 以前あった stale contact / typing contact のルールは、「打鍵の直後に
/// そのまま操作する」正当な操作まで広く食べてしまうため廃止した
/// （2026-11-22 の実機フィードバック）。大きな接触の抑止はモニタ側の
/// サイズ判定が担う。
fn palm_rejection_reason(
    cfg: &AppConfig,
    last_typing: Option<std::time::Instant>,
    now: std::time::Instant,
) -> Option<&'static str> {
    if !cfg.palm_rejection {
        return None;
    }
    let last_time = last_typing?;
    if now.duration_since(last_time).as_millis() < cfg.palm_rejection_delay_ms as u128 {
        return Some("recent typing");
    }
    None
}

fn compute_scroll_addition(
    pending_y: f32,
    elapsed: f32,
    global_sens: u32,
    global_invert: bool,
    global_speed: u32,
    min_out: f32,
    max_out: f32,
    use_curve: bool,
    curve: &[(f32, f32)],
    key_cfg: &ScrollConfig,
) -> f32 {
    let invert = global_invert ^ key_cfg.invert;
    let signed = if invert { -pending_y } else { pending_y };
    if signed == 0.0 {
        return 0.0;
    }
    let key_mult = key_cfg.sensitivity as f32 / 100.0;

    if use_curve
        && curve
            .iter()
            .filter(|(x, y)| x.is_finite() && y.is_finite())
            .take(2)
            .count()
            == 2
    {
        let input_speed = signed.abs() / elapsed;
        // カーブ使用時は Scroll Speed の 25（表示 50%）を等倍とする。
        let speed_mult = global_speed as f32 / 25.0;
        let output_speed = interpolate_curve(curve, input_speed) * speed_mult * key_mult;
        return signed.signum() * output_speed * elapsed;
    }

    #[cfg(target_os = "linux")]
    const LINEAR_SCALE: f32 = 1.0;
    #[cfg(not(target_os = "linux"))]
    const LINEAR_SCALE: f32 = 40.0;

    let gain = global_sens as f32 / 100.0;
    let speed = global_speed as f32 / 100.0;
    let mut addition = signed * gain * speed * LINEAR_SCALE * key_mult;
    if addition != 0.0 {
        let output_speed = addition.abs() / elapsed;
        if max_out > 0.0 && output_speed > max_out {
            addition = addition.signum() * max_out * elapsed;
        } else if min_out > 0.0 && output_speed < min_out {
            addition = addition.signum() * min_out * elapsed;
        }
    }
    addition
}

fn interpolate_curve(curve: &[(f32, f32)], input: f32) -> f32 {
    let mut points: Vec<_> = curve
        .iter()
        .copied()
        .filter(|(x, y)| x.is_finite() && y.is_finite())
        .collect();
    points.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    points.dedup_by(|next, prev| next.0 - prev.0 < 0.001);

    match points.as_slice() {
        [] => return input,
        [(_, y)] => return *y,
        _ => {}
    }

    // Fritsch–Carlson 法による単調三次スプライン補間
    let n = points.len();
    let (x, y): (Vec<_>, Vec<_>) = points.into_iter().unzip();

    if input <= x[0] {
        return y[0];
    }
    if input >= x[n - 1] {
        return y[n - 1];
    }

    // 区間の割線勾配と各点の接線勾配を求める
    let mut d = Vec::with_capacity(n - 1);
    for i in 0..n - 1 {
        d.push((y[i + 1] - y[i]) / (x[i + 1] - x[i]));
    }

    let mut m = vec![0.0; n];
    m[0] = d[0];
    for i in 1..n - 1 {
        m[i] = (d[i - 1] + d[i]) / 2.0;
    }
    m[n - 1] = d[n - 2];

    // 単調性を保つように接線勾配を調整する
    for i in 0..n - 1 {
        if d[i] == 0.0 {
            m[i] = 0.0;
            m[i + 1] = 0.0;
        } else {
            let a = m[i] / d[i];
            let b = m[i + 1] / d[i];
            let h = (a * a + b * b).sqrt();
            if h > 3.0 {
                let t = 3.0 / h;
                m[i] = t * a * d[i];
                m[i + 1] = t * b * d[i];
            }
        }
    }

    // 入力を含む区間で補間する
    for i in 0..n - 1 {
        let h = x[i + 1] - x[i];
        if h >= 0.001 && input >= x[i] && input <= x[i + 1] {
            let t = (input - x[i]) / h;
            let output = y[i] * (1.0 + 2.0 * t) * (1.0 - t).powi(2)
                + h * m[i] * t * (1.0 - t).powi(2)
                + y[i + 1] * t.powi(2) * (3.0 - 2.0 * t)
                + h * m[i + 1] * t.powi(2) * (t - 1.0);
            return if output.is_finite() { output } else { y[n - 1] };
        }
    }

    y[n - 1]
}

#[cfg(target_os = "linux")]
fn evdev_to_rdev_key(code: u16) -> Option<rdev::Key> {
    use rdev::Key;
    match code {
        1 => Some(Key::Escape),
        2 => Some(Key::Num1),
        3 => Some(Key::Num2),
        4 => Some(Key::Num3),
        5 => Some(Key::Num4),
        6 => Some(Key::Num5),
        7 => Some(Key::Num6),
        8 => Some(Key::Num7),
        9 => Some(Key::Num8),
        10 => Some(Key::Num9),
        11 => Some(Key::Num0),
        12 => Some(Key::Minus),
        13 => Some(Key::Equal),
        14 => Some(Key::Backspace),
        15 => Some(Key::Tab),
        16 => Some(Key::KeyQ),
        17 => Some(Key::KeyW),
        18 => Some(Key::KeyE),
        19 => Some(Key::KeyR),
        20 => Some(Key::KeyT),
        21 => Some(Key::KeyY),
        22 => Some(Key::KeyU),
        23 => Some(Key::KeyI),
        24 => Some(Key::KeyO),
        25 => Some(Key::KeyP),
        26 => Some(Key::LeftBracket),
        27 => Some(Key::RightBracket),
        28 => Some(Key::Return),
        29 => Some(Key::ControlLeft),
        30 => Some(Key::KeyA),
        31 => Some(Key::KeyS),
        32 => Some(Key::KeyD),
        33 => Some(Key::KeyF),
        34 => Some(Key::KeyG),
        35 => Some(Key::KeyH),
        36 => Some(Key::KeyJ),
        37 => Some(Key::KeyK),
        38 => Some(Key::KeyL),
        39 => Some(Key::SemiColon),
        40 => Some(Key::Quote),
        41 => Some(Key::BackQuote),
        42 => Some(Key::ShiftLeft),
        43 => Some(Key::BackSlash),
        44 => Some(Key::KeyZ),
        45 => Some(Key::KeyX),
        46 => Some(Key::KeyC),
        47 => Some(Key::KeyV),
        48 => Some(Key::KeyB),
        49 => Some(Key::KeyN),
        50 => Some(Key::KeyM),
        51 => Some(Key::Comma),
        52 => Some(Key::Dot),
        53 => Some(Key::Slash),
        54 => Some(Key::ShiftRight),
        56 => Some(Key::Alt),
        57 => Some(Key::Space),
        58 => Some(Key::CapsLock),
        59 => Some(Key::F1),
        60 => Some(Key::F2),
        61 => Some(Key::F3),
        62 => Some(Key::F4),
        63 => Some(Key::F5),
        64 => Some(Key::F6),
        65 => Some(Key::F7),
        66 => Some(Key::F8),
        67 => Some(Key::F9),
        68 => Some(Key::F10),
        103 => Some(Key::UpArrow),
        105 => Some(Key::LeftArrow),
        106 => Some(Key::RightArrow),
        108 => Some(Key::DownArrow),
        125 => Some(Key::MetaLeft),
        126 => Some(Key::MetaRight),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key_cfg(invert: bool) -> ScrollConfig {
        ScrollConfig {
            sensitivity: 100,
            invert,
            acceleration: false,
        }
    }

    #[test]
    fn curve_output_scales_with_elapsed() {
        // 入力速度 2000（16 / 0.008秒）→ カーブ出力 80/s → 0.64 ノッチ
        let curve = vec![(0.0, 0.0), (2000.0, 80.0)];
        let addition = compute_scroll_addition(
            16.0, 0.008, 1, false, 25, 0.0, 0.0, true, &curve, &key_cfg(false),
        );
        assert!((addition - 0.64).abs() < 0.01, "addition={addition}");
    }

    #[test]
    fn invert_is_xor_of_global_and_key() {
        let curve = vec![(0.0, 0.0), (2000.0, 80.0)];
        let base = compute_scroll_addition(
            16.0, 0.008, 1, false, 25, 0.0, 0.0, true, &curve, &key_cfg(false),
        );
        let global_only = compute_scroll_addition(
            16.0, 0.008, 1, true, 25, 0.0, 0.0, true, &curve, &key_cfg(false),
        );
        let both = compute_scroll_addition(
            16.0, 0.008, 1, true, 25, 0.0, 0.0, true, &curve, &key_cfg(true),
        );
        assert!(base > 0.0);
        assert_eq!(global_only, -base);
        assert_eq!(both, base);
    }

    #[test]
    fn curve_output_is_not_clamped_after_speed_multiplier() {
        // Scroll Speed 50 で出力は 2 倍の 160/s になり、Max=100/s を超える
        let curve = vec![(0.0, 0.0), (2000.0, 80.0)];
        let addition = compute_scroll_addition(
            16.0, 0.008, 1, false, 50, 0.0, 100.0, true, &curve, &key_cfg(false),
        );
        assert!((addition - 1.28).abs() < 0.01, "addition={addition}");
    }

    #[test]
    fn linear_output_is_clamped_to_max_speed() {
        let addition = compute_scroll_addition(
            16.0, 0.008, 100, false, 25, 0.0, 100.0, false, &[], &key_cfg(false),
        );
        assert!((addition - 0.8).abs() < 0.01, "addition={addition}");
    }

    #[test]
    fn duplicate_x_points_do_not_produce_nan() {
        let curve = vec![(0.0, 1.0), (100.0, 2.0), (100.0, 3.0), (200.0, 4.0)];
        let addition = compute_scroll_addition(
            1.2, 0.008, 1, false, 25, 0.0, 0.0, true, &curve, &key_cfg(false),
        );
        assert!(addition.is_finite(), "addition={addition}");
    }
}
