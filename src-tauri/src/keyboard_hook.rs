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
                        let mut addition = delta * gain * speed * LINEAR_SCALE;

                        #[cfg(target_os = "linux")]
                        {
                            // Speed clamping (addition per 8ms -> output per sec)
                            // output = addition
                            // speed_per_sec = addition / 0.008
                            let current_out_speed = addition.abs() / 0.008;
                            if current_out_speed < _min_scroll && current_out_speed > 0.0 {
                                addition = addition.signum() * _min_scroll * 0.008;
                            } else if current_out_speed > _max_scroll {
                                addition = addition.signum() * _max_scroll * 0.008;
                            }
                        }

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
                                    let _ = tx_inner.send(RemapCommand::UpdateScroll(Some(cfg)));
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
}
