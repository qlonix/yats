// Linux-specific touchpad monitoring

use crate::touchpad_monitor::MonitorState;
use evdev::{AbsoluteAxisType, Device, EventType};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::Emitter;

pub fn run_monitor(state: Arc<MonitorState>) {
    crate::audit_log("[MONITOR] Starting Linux Monitor Worker");

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

                            // Exclusion keywords
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
                                "i2c-hid",
                                "bcm5974",
                            ];
                            let name_match = touchpad_keywords.iter().any(|k| name.contains(k));

                            // Check for absolute axis support
                            let has_abs = device.supported_events().contains(EventType::ABSOLUTE)
                                && device.supported_absolute_axes().map_or(false, |axes| {
                                    axes.contains(AbsoluteAxisType::ABS_MT_POSITION_X)
                                        || (axes.contains(AbsoluteAxisType::ABS_X)
                                            && axes.contains(AbsoluteAxisType::ABS_Y))
                                });

                            let is_touchpad =
                                !is_excluded && (name_match || (has_abs && name.contains("touch")));

                            if is_touchpad {
                                crate::audit_log(&format!(
                                    "[MONITOR] Found Touchpad: '{}' at {:?}",
                                    device.name().unwrap_or("Unknown"),
                                    path
                                ));
                                devices.push(device);
                            }
                        }
                        Err(e) => {
                            crate::audit_log(&format!("[MONITOR] Cannot open {:?}: {}", path, e));
                        }
                    }
                }
            }
        } else {
            crate::audit_log("[MONITOR] Cannot read /dev/input directory");
        }

        if devices.is_empty() {
            crate::audit_log(
                "[MONITOR] No touchpad found. Ensure user is in 'input' group. Retrying in 5s...",
            );
            std::thread::sleep(std::time::Duration::from_secs(5));
            continue;
        }

        // Monitor events from all found touchpads
        let mut threads = Vec::new();
        for mut device in devices {
            let s_clone = Arc::clone(&state);
            threads.push(std::thread::spawn(move || {
                monitor_device(&mut device, s_clone);
            }));
        }

        // Wait for threads
        for t in threads {
            let _ = t.join();
        }
        crate::audit_log("[MONITOR] Device lost, rescanning...");
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}

fn monitor_device(device: &mut Device, state: Arc<MonitorState>) {
    let mut last_abs_x: Option<i32> = None;
    let mut last_abs_y: Option<i32> = None;
    let mut accum_x: i32 = 0;
    let mut accum_y: i32 = 0;
    let mut _last_dir_y: i32 = 0;

    // Track touch slots and palm state
    let mut current_slot: usize = 0;
    let mut slot_is_palm = [false; 16];

    const ABS_MT_SLOT: u16 = 0x2f;
    const ABS_MT_TOOL_TYPE: u16 = 0x37;
    const ABS_MT_TRACKING_ID: u16 = 0x39;

    loop {
        if let Ok(events) = device.fetch_events() {
            for ev in events {
                let ev_type = ev.event_type();

                // Process slot/palm tracking first
                if ev_type == EventType::ABSOLUTE {
                    let code = ev.code();
                    if code == ABS_MT_SLOT {
                        let slot = ev.value() as usize;
                        if slot < slot_is_palm.len() {
                            current_slot = slot;
                        }
                    } else if code == ABS_MT_TRACKING_ID {
                        if ev.value() == -1 {
                            // Touch released for this slot
                            if current_slot < slot_is_palm.len() {
                                slot_is_palm[current_slot] = false;
                            }
                        }
                    } else if code == ABS_MT_TOOL_TYPE {
                        // MT_TOOL_PALM = 2
                        if current_slot < slot_is_palm.len() {
                            slot_is_palm[current_slot] = ev.value() == 2;
                        }
                    }
                }

                if ev_type == EventType::ABSOLUTE
                    || ev_type == EventType::RELATIVE
                    || ev_type == EventType::KEY
                {
                    // Check if current event is considered a palm touch
                    let is_palm = if ev_type == EventType::ABSOLUTE {
                        current_slot < slot_is_palm.len() && slot_is_palm[current_slot]
                    } else {
                        // For relative/key events, check if any active slot is marked as palm
                        slot_is_palm.iter().any(|&p| p)
                    };

                    if !is_palm {
                        state.is_touched.store(true, Ordering::SeqCst);
                        if let Ok(mut last_time) = state.last_touch_time.lock() {
                            *last_time = std::time::Instant::now();
                        }
                    }

                    // Handle ABSOLUTE axis events
                    if ev_type == EventType::ABSOLUTE {
                        if is_palm {
                            continue;
                        }

                        const JUMP_THRESHOLD: i32 = 100;
                        // タッチパッド移動の最小検出閾値（1=即座に反応）
                        const ACCUM_THRESHOLD: i32 = 1;

                        match ev.code() {
                            0 => {
                                // ABS_X
                                if let Some(last_x) = last_abs_x {
                                    let delta = ev.value() - last_x;
                                    if delta.abs() >= JUMP_THRESHOLD {
                                        // 指が離れて別の場所に置かれたか、大きなノイズ
                                        accum_x = 0;
                                        accum_y = 0;
                                        _last_dir_y = 0;
                                    } else {
                                        accum_x += delta;
                                        if accum_x.abs() >= ACCUM_THRESHOLD {
                                            let output = if accum_x.abs() < ACCUM_THRESHOLD * 2 {
                                                if accum_x > 0 {
                                                    1
                                                } else {
                                                    -1
                                                }
                                            } else {
                                                // Windows版と同じスケールに合わせる
                                                accum_x.clamp(-120, 120)
                                            };
                                            state.x_delta.fetch_add(output, Ordering::SeqCst);
                                            accum_x = 0;
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
                                        accum_x = 0;
                                        accum_y = 0;
                                        _last_dir_y = 0;
                                    } else {
                                        accum_y += delta;
                                        if accum_y.abs() >= ACCUM_THRESHOLD {
                                            let output = if accum_y.abs() < ACCUM_THRESHOLD * 2 {
                                                if accum_y > 0 {
                                                    1
                                                } else {
                                                    -1
                                                }
                                            } else {
                                                accum_y.clamp(-120, 120)
                                            };
                                            state.y_delta.fetch_add(output, Ordering::SeqCst);
                                            accum_y = 0;
                                        }
                                    }
                                }
                                last_abs_y = Some(ev.value());
                            }
                            _ => {}
                        }
                    }

                    // Handle RELATIVE movement
                    if ev_type == EventType::RELATIVE {
                        if is_palm {
                            continue;
                        }

                        match ev.code() {
                            0 => {
                                // REL_X
                                state.x_delta.fetch_add(ev.value(), Ordering::SeqCst);
                            }
                            1 => {
                                // REL_Y
                                state.y_delta.fetch_add(ev.value(), Ordering::SeqCst);
                            }
                            _ => {}
                        }
                    }

                    // Reset tracking on finger lift
                    if ev_type == EventType::KEY && ev.code() == 330 && ev.value() == 0 {
                        last_abs_x = None;
                        last_abs_y = None;
                        accum_x = 0;
                        accum_y = 0;
                        _last_dir_y = 0;
                    }

                    // UI Update
                    if !is_palm {
                        if let Ok(h) = state.app_handle.try_lock() {
                            if let Some(app) = &*h {
                                let _ = app.emit("touchpad-status", true).ok();
                            }
                        }
                    }
                }
            }
        } else {
            break; // Device disconnected
        }
    }
}

pub fn scan_and_register(_state: &MonitorState) {
    crate::audit_log("[MONITOR] Device scan requested (Linux)");
}
