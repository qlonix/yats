use crate::config::{Action, AppConfig, ScrollConfig};
use crate::platform;
use crate::touchpad_monitor::TouchpadMonitor;
use rdev::{simulate, EventType};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, RwLock};
use tauri::AppHandle;

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

const VK_BROWSER_BACK: u16 = 0xA6;
const VK_BROWSER_FORWARD: u16 = 0xA7;

fn execute_action(action: Action, pressed: bool) {
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
        Action::Window(win_act) => {
            if pressed {
                platform::execute_window_action(win_act);
            }
        }
        Action::BrowserBack => {
            platform::send_key_click(VK_BROWSER_BACK, pressed);
        }
        Action::BrowserForward => {
            platform::send_key_click(VK_BROWSER_FORWARD, pressed);
        }
    }
}

pub struct HookWorker {
    rx: Receiver<RemapCommand>,
    active_scroll: Option<ScrollConfig>,
    monitor: Arc<TouchpadMonitor>,
    config: Arc<RwLock<AppConfig>>,
    scroll_anchor: Option<(i32, i32)>,
    accumulator_y: f32,
    total_movement_y: f32,
    is_scrolling: bool,
    last_scroll_time: std::time::Instant,
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
            total_movement_y: 0.0,
            is_scrolling: false,
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
                            self.scroll_anchor = platform::get_cursor_pos();
                        } else {
                            self.scroll_anchor = None;
                        }
                        self.total_movement_y = 0.0;
                        self.is_scrolling = false;
                        self.accumulator_y = 0.0;
                    }
                }
            }

            if let Some(_cfg) = &self.active_scroll {
                if let Some((x, y)) = self.scroll_anchor {
                    platform::set_cursor_pos(x, y);
                }

                let raw_delta = self.monitor.consume_y_delta();
                // Batching dispatcher
                let now = std::time::Instant::now();
                let _dt = now.duration_since(self.last_scroll_time).as_secs_f32();

                if raw_delta != 0 {
                    let (
                        global_sens,
                        global_invert,
                        global_speed,
                        _min_dist,
                        _min_speed,
                        _min_scroll,
                        _max_scroll,
                        _use_curve,
                        _curve,
                    ) = {
                        let cfg_lock = self.config.read().unwrap();
                        (
                            cfg_lock.scroll_sensitivity,
                            cfg_lock.scroll_invert,
                            cfg_lock.scroll_speed,
                            cfg_lock.linux_min_distance,
                            cfg_lock.linux_min_speed,
                            cfg_lock.linux_min_scroll_speed,
                            cfg_lock.linux_max_scroll_speed,
                            cfg_lock.linux_use_scroll_curve,
                            cfg_lock.linux_scroll_curve.clone(),
                        )
                    };

                    let mut delta = raw_delta as f32;
                    if global_invert {
                        delta = -delta;
                    }

                    #[cfg(target_os = "linux")]
                    {
                        self.total_movement_y += delta;
                        let current_speed = (delta / _dt).abs();

                        if !self.is_scrolling {
                            if self.total_movement_y.abs() >= _min_dist as f32
                                || current_speed >= _min_speed
                            {
                                self.is_scrolling = true;
                            }
                        }
                    }

                    #[cfg(not(target_os = "linux"))]
                    {
                        self.is_scrolling = true;
                    }

                    if self.is_scrolling {
                        let gain = (global_sens as f32) / 100.0;
                        let speed = (global_speed as f32) / 100.0;
                        const LINEAR_SCALE: f32 = 40.0;

                        #[cfg(target_os = "linux")]
                        let addition = {
                            if _use_curve && !_curve.is_empty() {
                                // Curve based scaling
                                let input_speed = (delta / _dt).abs();
                                let output_speed = interpolate_curve(&_curve, input_speed);

                                // output_total = output_speed * dt
                                // Maintain direction
                                delta.signum() * output_speed * _dt
                            } else {
                                // Linear based scaling (existing logic)
                                let mut temp_addition = delta * gain * speed * LINEAR_SCALE;
                                let current_out_speed = temp_addition.abs() / 0.008;
                                if current_out_speed < _min_scroll && current_out_speed > 0.0 {
                                    temp_addition = temp_addition.signum() * _min_scroll * 0.008;
                                } else if current_out_speed > _max_scroll {
                                    temp_addition = temp_addition.signum() * _max_scroll * 0.008;
                                }
                                temp_addition
                            }
                        };

                        #[cfg(not(target_os = "linux"))]
                        let addition = delta * gain * speed * LINEAR_SCALE;

                        self.accumulator_y += addition;
                    }
                }

                if now.duration_since(self.last_scroll_time).as_millis() >= 8 {
                    let output = self.accumulator_y.trunc() as i32;
                    if output != 0 {
                        platform::send_mouse_scroll(output);
                        self.accumulator_y -= output as f32;
                    }
                    self.last_scroll_time = now;
                }
            } else {
                self.monitor.consume_x_delta();
                self.monitor.consume_y_delta();
                self.accumulator_y = 0.0;
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
        #[cfg(not(target_os = "linux"))]
        {
            crate::audit_log("[HOOK] Starting keyboard hook (rdev::grab)...");
            let tx = self.tx.clone();
            let config_ref = Arc::clone(&self.config);
            let monitor_ref = Arc::clone(&self.monitor);

            loop {
                let tx_inner = tx.clone();
                let config_inner = Arc::clone(&config_ref);
                let monitor_inner = Arc::clone(&monitor_ref);
                let pressed_keys = Arc::new(Mutex::new(std::collections::HashSet::new()));

                crate::audit_log("[HOOK] Calling rdev::grab...");
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

                            if !monitor_inner.is_touched() {
                                return Some(event);
                            }

                            keys.insert(key);

                            let action = {
                                let cfg = config_inner.read().unwrap();
                                cfg.mappings.get(&key).cloned()
                            };

                            if let Some(action) = action {
                                match action {
                                    Action::MouseScroll(cfg) => {
                                        monitor_inner.consume_y_delta();
                                        let _ =
                                            tx_inner.send(RemapCommand::UpdateScroll(Some(cfg)));
                                    }
                                    _ => {
                                        let _ = tx_inner.send(RemapCommand::Execute(action, true));
                                    }
                                }
                                return None;
                            }
                        }
                        rdev::EventType::KeyRelease(key) => {
                            let mut keys = pressed_keys.lock().unwrap();
                            if !keys.remove(&key) {
                                return Some(event);
                            }

                            let action = {
                                let cfg = config_inner.read().unwrap();
                                cfg.mappings.get(&key).cloned()
                            };

                            if let Some(action) = action {
                                match action {
                                    Action::MouseScroll(_) => {
                                        let _ = tx_inner.send(RemapCommand::UpdateScroll(None));
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

                                // Only true keyboards (must have KEY events, but NOT RELATIVE or ABSOLUTE pointing axes)
                                if caps.contains(evdev::EventType::KEY)
                                    && !caps.contains(evdev::EventType::RELATIVE)
                                    && !caps.contains(evdev::EventType::ABSOLUTE)
                                    && !name.contains("touchpad")
                                    && !name.contains("mouse")
                                {
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
                let mut uinput_builder = evdev::uinput::VirtualDeviceBuilder::new()
                    .unwrap_or_else(|e| {
                        crate::audit_log(&format!("[HOOK] Failed to init uinput builder: {}", e));
                        panic!("uinput init fail");
                    })
                    .name("YATS Virtual Keyboard");

                // Copy capabilities from all grabbed devices
                let mut keys = evdev::AttributeSet::<evdev::Key>::default();
                for d in &devices {
                    if let Some(set) = d.supported_keys() {
                        for k in set.iter() {
                            keys.insert(k);
                        }
                    }
                }

                uinput_builder = uinput_builder.with_keys(&keys);

                let mut virtual_device = uinput_builder.build().unwrap_or_else(|e| {
                    crate::audit_log(&format!("[HOOK] Failed to build uinput device: {}", e));
                    panic!("uinput build fail");
                });

                let mut threads = Vec::new();
                let virtual_device = Arc::new(Mutex::new(virtual_device));

                for mut device in devices {
                    let tx_inner = tx.clone();
                    let config_inner = Arc::clone(&config_ref);
                    let monitor_inner = Arc::clone(&monitor_ref);
                    let v_dev = Arc::clone(&virtual_device);

                    threads.push(std::thread::spawn(move || {
                        let mut pressed_keys = std::collections::HashSet::new();
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
                                        let is_remap = {
                                            let cfg = config_inner.read().unwrap();
                                            cfg.mappings.contains_key(&rkey)
                                        };

                                        let touched = monitor_inner.is_touched();
                                        let paused = IS_PAUSED.load(Ordering::SeqCst);
                                        let simulating = IS_SIMULATING.load(Ordering::SeqCst);

                                        if is_remap && touched && !paused && !simulating {
                                            if value == 1 {
                                                // Press
                                                pressed_keys.insert(rkey);
                                                let action = {
                                                    let cfg = config_inner.read().unwrap();
                                                    cfg.mappings.get(&rkey).cloned()
                                                };
                                                if let Some(action) = action {
                                                    match action {
                                                        Action::MouseScroll(cfg) => {
                                                            monitor_inner.consume_y_delta();
                                                            let _ = tx_inner.send(
                                                                RemapCommand::UpdateScroll(Some(
                                                                    cfg,
                                                                )),
                                                            );
                                                        }
                                                        _ => {
                                                            let _ = tx_inner.send(
                                                                RemapCommand::Execute(action, true),
                                                            );
                                                        }
                                                    }
                                                }
                                            } else if value == 0 {
                                                // Release
                                                if pressed_keys.remove(&rkey) {
                                                    let action = {
                                                        let cfg = config_inner.read().unwrap();
                                                        cfg.mappings.get(&rkey).cloned()
                                                    };
                                                    if let Some(action) = action {
                                                        match action {
                                                            Action::MouseScroll(_) => {
                                                                let _ = tx_inner.send(
                                                                    RemapCommand::UpdateScroll(
                                                                        None,
                                                                    ),
                                                                );
                                                            }
                                                            _ => {
                                                                let _ = tx_inner.send(
                                                                    RemapCommand::Execute(
                                                                        action, false,
                                                                    ),
                                                                );
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            // Consume event (do not pass through)
                                            continue;
                                        }
                                    }
                                }
                                // Forward to virtual device
                                let mut v = v_dev.lock().unwrap();
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

#[cfg(target_os = "linux")]
fn interpolate_curve(curve: &[(f32, f32)], input: f32) -> f32 {
    if curve.is_empty() {
        return input;
    }
    if curve.len() == 1 {
        return curve[0].1;
    }

    // Monotone Cubic Spline (Fritsch-Carlson)
    let n = curve.len();
    let mut x = Vec::with_capacity(n);
    let mut y = Vec::with_capacity(n);
    for p in curve {
        x.push(p.0);
        y.push(p.1);
    }

    if input <= x[0] {
        return y[0];
    }
    if input >= x[n - 1] {
        return y[n - 1];
    }

    // Secants and slopes
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

    // Monotonicity adjustment
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

    // Interpolation
    for i in 0..n - 1 {
        if input >= x[i] && input <= x[i + 1] {
            let h = x[i + 1] - x[i];
            let t = (input - x[i]) / h;
            return y[i] * (1.0 + 2.0 * t) * (1.0 - t).powi(2)
                + h * m[i] * t * (1.0 - t).powi(2)
                + y[i + 1] * t.powi(2) * (3.0 - 2.0 * t)
                + h * m[i + 1] * t.powi(2) * (t - 1.0);
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
