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
    let axes = device.supported_absolute_axes();
    let has_axis = |axis| axes.is_some_and(|axes| axes.contains(axis));
    let has_touch_major = has_axis(AbsoluteAxisType::ABS_MT_TOUCH_MAJOR);
    let (major_resolution, major_maximum) = if has_touch_major {
        device.get_abs_state().map_or((0, 0), |abs_state| {
            let major = &abs_state[AbsoluteAxisType::ABS_MT_TOUCH_MAJOR.0 as usize];
            (major.resolution, major.maximum)
        })
    } else {
        (0, 0)
    };
    let (minor_maximum, width_maximum) = device.get_abs_state().map_or((0, 0), |abs_state| {
        (
            abs_state[AbsoluteAxisType::ABS_MT_TOUCH_MINOR.0 as usize].maximum,
            abs_state[AbsoluteAxisType::ABS_MT_WIDTH_MAJOR.0 as usize].maximum,
        )
    });
    crate::audit_log(&format!(
        "[MONITOR] MT axes: tool_type={} touch_major={} (res={} max={}) touch_minor={} (max={}) width_major={} (max={}) pressure={}",
        has_axis(AbsoluteAxisType::ABS_MT_TOOL_TYPE),
        has_touch_major,
        major_resolution,
        major_maximum,
        has_axis(AbsoluteAxisType::ABS_MT_TOUCH_MINOR),
        minor_maximum,
        has_axis(AbsoluteAxisType::ABS_MT_WIDTH_MAJOR),
        width_maximum,
        has_axis(AbsoluteAxisType::ABS_MT_PRESSURE),
    ));

    let mut last_abs_x: Option<i32> = None;
    let mut last_abs_y: Option<i32> = None;
    let mut accum_x: i32 = 0;
    let mut accum_y: i32 = 0;

    let mut _last_dir_y: i32 = 0;

    // Track touch slots and palm state
    let mut current_slot: usize = 0;
    let mut slot_active = [false; 16];
    let mut st_contact = false;
    let mut slot_is_palm = [false; 16];
    let mut slot_major_palm = [false; 16];
    // 解像度を報告しないデバイス向けのパーム判定しきい値（TOUCH_MAJOR の生値）。
    // 実測では指先の接触は最大でも 450 前後、手のひらは 550 以上だった。
    const RAW_PALM_MAJOR_THRESHOLD: i32 = 500;

    const ABS_MT_SLOT: u16 = 0x2f;
    const ABS_MT_TOOL_TYPE: u16 = AbsoluteAxisType::ABS_MT_TOOL_TYPE.0;
    const ABS_MT_TOUCH_MAJOR: u16 = AbsoluteAxisType::ABS_MT_TOUCH_MAJOR.0;
    const ABS_MT_TRACKING_ID: u16 = 0x39;
    const BTN_TOUCH: u16 = 330;

    let mut grabbed = false;
    let mut pending_events = Vec::new();
    loop {
        // グラブの切り替え前にイベントイテレータのデバイス借用を終える。
        let fetched = device
            .fetch_events()
            .map(|events| pending_events.extend(events));
        if fetched.is_ok() {
            if state.cursor_freeze.load(Ordering::SeqCst) {
                if !grabbed {
                    match device.grab() {
                        Ok(()) => {
                            grabbed = true;
                            crate::audit_log("[FREEZE] touchpad grabbed");
                        }
                        Err(e) => crate::audit_log(&format!("[FREEZE] touchpad grab failed: {e}")),
                    }
                }
            } else if grabbed {
                match device.ungrab() {
                    Ok(()) => {
                        grabbed = false;
                        crate::audit_log("[FREEZE] touchpad ungrabbed");
                    }
                    Err(e) => crate::audit_log(&format!("[FREEZE] touchpad ungrab failed: {e}")),
                }
            }

            for ev in pending_events.drain(..) {
                let ev_type = ev.event_type();

                let value = ev.value();
                let contact_event = (ev_type == EventType::ABSOLUTE
                    && ev.code() == ABS_MT_TRACKING_ID
                    && value >= -1)
                    || (ev_type == EventType::KEY
                        && ev.code() == BTN_TOUCH
                        && matches!(value, 0 | 1));

                let mut palm_flipped = false;

                // Process slot/palm tracking first
                if ev_type == EventType::ABSOLUTE {
                    let code = ev.code();
                    if code == ABS_MT_SLOT {
                        let slot = ev.value() as usize;
                        if slot < slot_is_palm.len() {
                            current_slot = slot;
                        }
                    } else if code == ABS_MT_TRACKING_ID {
                        if value >= 0 {
                            slot_active[current_slot] = true;
                        } else if value == -1 {
                            slot_active[current_slot] = false;
                            slot_is_palm[current_slot] = false;
                            slot_major_palm[current_slot] = false;
                        }
                    } else if code == ABS_MT_TOOL_TYPE {
                        // MT_TOOL_PALM = 2
                        if current_slot < slot_is_palm.len() {
                            let is_palm = ev.value() == 2;
                            if is_palm && !slot_is_palm[current_slot] {
                                crate::audit_log(&format!(
                                    "[PALM] slot {current_slot} rejected by tool_type"
                                ));
                                palm_flipped = true;
                            }
                            slot_is_palm[current_slot] = is_palm;
                        }
                    } else if code == ABS_MT_TOUCH_MAJOR {
                        // 一度しきい値を超えたら、その接触が終わるまでパーム扱いを維持する
                        // （接触面積は指の重みで上下するため、値に追従させると判定が漏れる）。
                        if major_resolution > 0 {
                            let size_mm = f64::from(ev.value()) / f64::from(major_resolution);
                            if size_mm >= 15.0 && !slot_major_palm[current_slot] {
                                crate::audit_log(&format!(
                                    "[PALM] slot {current_slot} rejected by size ({size_mm:.1} mm)"
                                ));
                                slot_major_palm[current_slot] = true;
                                palm_flipped = true;
                            }
                        } else if ev.value() >= RAW_PALM_MAJOR_THRESHOLD
                            && !slot_major_palm[current_slot]
                        {
                            // 解像度を報告しないデバイス向け: 生値のしきい値で判定する
                            // （実測: 指先は最大でも 450 前後、手のひらは 550 以上）。
                            crate::audit_log(&format!(
                                "[PALM] slot {current_slot} rejected by size (raw {})",
                                ev.value()
                            ));
                            slot_major_palm[current_slot] = true;
                            palm_flipped = true;
                        }
                    }
                } else if contact_event {
                    st_contact = value == 1;
                }

                if ev_type == EventType::ABSOLUTE
                    || ev_type == EventType::RELATIVE
                    || ev_type == EventType::KEY
                {
                    // Check if current event is considered a palm touch
                    let is_palm = if ev_type == EventType::ABSOLUTE {
                        slot_is_palm[current_slot] || slot_major_palm[current_slot]
                    } else {
                        // For relative/key events, check if any active slot is marked as palm
                        slot_is_palm.iter().chain(&slot_major_palm).any(|&p| p)
                    };

                    if !is_palm || contact_event {
                        if let Ok(mut last_time) = state.last_touch_time.lock() {
                            let now = std::time::Instant::now();
                            if contact_event {
                                let any_active =
                                    slot_active.iter().any(|&active| active) || st_contact;
                                if let Ok(mut start_time) = state.contact_start_time.lock() {
                                    if any_active {
                                        start_time.get_or_insert(now);
                                    } else {
                                        *start_time = None;
                                    }
                                }
                            }
                            if !is_palm {
                                *last_time = now;
                                state.is_touched.store(true, Ordering::SeqCst);
                            }
                        }
                    }

                    // パームと確定した接触しか残っていない場合は、タッチ状態を
                    // 即座に解除する（watcher の期限切れを待つと、その間に
                    // 押されたキーが誤発火するため）。
                    if palm_flipped {
                        let any_non_palm = slot_active
                            .iter()
                            .zip(slot_is_palm.iter().zip(slot_major_palm.iter()))
                            .any(|(&active, (&p1, &p2))| active && !p1 && !p2);
                        if !any_non_palm && !st_contact {
                            state.is_touched.store(false, Ordering::SeqCst);
                            if let Ok(mut start_time) = state.contact_start_time.lock() {
                                *start_time = None;
                            }
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

    // デバイスを失った場合は接触状態を残さない（再スキャン後に、
    // 触っていないのに接触中扱いになるのを防ぐ）
    state.is_touched.store(false, Ordering::SeqCst);
    if let Ok(mut start_time) = state.contact_start_time.lock() {
        *start_time = None;
    }
}

pub fn scan_and_register(_state: &MonitorState) {
    crate::audit_log("[MONITOR] Device scan requested (Linux)");
}

/// マウスやトラックポイントなど、タッチパッド以外のポインティング
/// デバイスを監視する。REL_X/REL_Y のデルタをモニタの x_delta/y_delta
/// に供給し（スクロール等の移動量になる）、移動検出のタイムスタンプも
/// デバイスレートで打つ（X ポーリングより遅延がない）。
/// 接触検出モード（trigger_mode == 0）ではイベントを読み捨てるだけで
/// 何も供給しない。
pub fn run_pointer_monitor(state: Arc<MonitorState>) {
    crate::audit_log("[MONITOR] Starting Linux Pointer Monitor Worker");

    loop {
        let mut devices = Vec::new();
        if let Ok(dir) = std::fs::read_dir("/dev/input") {
            for entry in dir.flatten() {
                let path = entry.path();
                if path.to_string_lossy().contains("event") {
                    if let Ok(device) = Device::open(&path) {
                        if is_pointer_device(&device) {
                            crate::audit_log(&format!(
                                "[MONITOR] Found Pointer Device: '{}' at {:?}",
                                device.name().unwrap_or("Unknown"),
                                path
                            ));
                            devices.push(device);
                        }
                    }
                }
            }
        }

        if devices.is_empty() {
            std::thread::sleep(std::time::Duration::from_secs(5));
            continue;
        }

        let mut threads = Vec::new();
        for mut device in devices {
            let s_clone = Arc::clone(&state);
            threads.push(std::thread::spawn(move || {
                monitor_pointer_device(&mut device, s_clone);
            }));
        }

        for t in threads {
            let _ = t.join();
        }
        crate::audit_log("[MONITOR] Pointer device lost, rescanning...");
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}

fn is_pointer_device(device: &Device) -> bool {
    let name = device.name().unwrap_or("").to_lowercase();
    // 自前の仮想デバイス（YATS Virtual Scroll は REL_X/REL_Y と
    // ボタンを持つため、名前で確実に除外する）
    if name.contains("yats") || name.contains("virtual") {
        return false;
    }
    // ABS を持つデバイス（タッチパッド等）は接触検出側で扱う
    if device.supported_events().contains(EventType::ABSOLUTE) {
        return false;
    }
    let has_rel = device.supported_relative_axes().map_or(false, |axes| {
        axes.contains(evdev::RelativeAxisType::REL_X)
            && axes.contains(evdev::RelativeAxisType::REL_Y)
    });
    let has_btn_left = device
        .supported_keys()
        .map_or(false, |keys| keys.contains(evdev::Key::BTN_LEFT));
    has_rel && has_btn_left
}

fn monitor_pointer_device(device: &mut Device, state: Arc<MonitorState>) {
    loop {
        let events = match device.fetch_events() {
            Ok(it) => it.collect::<Vec<_>>(),
            Err(_) => break,
        };

        // 接触検出モードでは読み捨てる（モード切替時に古いデルタが
        // まとめて流れるのを防ぐため、常に読み続ける）
        if state.trigger_mode.load(Ordering::SeqCst) == 0 {
            continue;
        }

        for ev in events {
            if ev.event_type() == EventType::RELATIVE {
                match ev.code() {
                    0 => {
                        state.x_delta.fetch_add(ev.value(), Ordering::SeqCst);
                    }
                    1 => {
                        state.y_delta.fetch_add(ev.value(), Ordering::SeqCst);
                    }
                    _ => continue,
                }
                *crate::lock_unpoison(&state.last_movement_time) =
                    std::time::Instant::now();
                state.movement_seen.store(true, Ordering::SeqCst);
            }
        }
    }
}
