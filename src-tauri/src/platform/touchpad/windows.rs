// Windows-specific touchpad monitoring

use crate::touchpad_monitor::MonitorState;
use std::collections::HashSet;
use std::ptr::null_mut;
use std::sync::Arc;
use tauri::Emitter;
use winapi::shared::windef::HWND;
use winapi::um::libloaderapi::GetModuleHandleW;
use winapi::um::winuser::*;

use winapi::shared::ntdef::PVOID;
use winapi::shared::hidpi::*;
use winapi::shared::minwindef::{ULONG, USHORT, UINT};

#[derive(Clone, Copy)]
pub struct SendHwnd(pub HWND);
unsafe impl Send for SendHwnd {}
unsafe impl Sync for SendHwnd {}

pub unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    use std::sync::atomic::Ordering;

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
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut MonitorState;

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

                // Exclude injected input (hDevice == 0 usually indicates SendInput)
                if raw.header.hDevice.is_null() {
                    return DefWindowProcW(hwnd, msg, wparam, lparam);
                }

                if raw.header.dwType == RIM_TYPEMOUSE {
                    let m = raw.data.mouse();
                    if m.lLastX != 0 || m.lLastY != 0 {
                        event_detected = true;
                        state.x_delta.fetch_add(m.lLastX, Ordering::SeqCst);
                        state.y_delta.fetch_add(m.lLastY, Ordering::SeqCst);
                    }
                    if m.ulRawButtons != 0 {
                        event_detected = true;
                    }
                } else if raw.header.dwType == RIM_TYPEHID {
                    let hid = raw.data.hid();
                    if check_hid_touch(raw.header.hDevice, hid) {
                        event_detected = true;
                    }
                }

                if event_detected {
                    state.is_touched.store(true, Ordering::SeqCst);
                    if let Ok(mut last_time) = state.last_touch_time.lock() {
                        *last_time = std::time::Instant::now();
                    }
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

pub fn run_monitor(state: Arc<MonitorState>) {
    crate::audit_log("[MONITOR] Starting Windows Monitor Worker");

    unsafe {
        let h_instance = GetModuleHandleW(null_mut());
        let class_name: Vec<u16> = "TouchpadMonitorClass\0".encode_utf16().collect();
        let wnd_class = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(wnd_proc),
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

        SetWindowLongPtrW(
            hwnd,
            GWLP_USERDATA,
            Arc::into_raw(Arc::clone(&state)) as isize,
        );

        // Trigger device registration right after window creation
        scan_and_register(&state, hwnd);

        let mut msg = std::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) != 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

pub fn scan_and_register(_state: &MonitorState, hwnd: HWND) {
    unsafe {
        let mut unique_pairs = HashSet::new();
        unique_pairs.insert((0x01, 0x02)); // Mouse
        unique_pairs.insert((0x0D, 0x04)); // Touchscreen
        unique_pairs.insert((0x0D, 0x05)); // Touchpad

        crate::audit_log("[MONITOR] Registering for Raw Input");

        let mut rids = Vec::new();
        for (p, u) in unique_pairs {
            rids.push(RAWINPUTDEVICE {
                usUsagePage: p,
                usUsage: u,
                dwFlags: RIDEV_INPUTSINK,
                hwndTarget: hwnd,
            });
        }
        if RegisterRawInputDevices(
            rids.as_ptr(),
            rids.len() as u32,
            std::mem::size_of::<RAWINPUTDEVICE>() as u32,
        ) == 0
        {
            crate::audit_log("[MONITOR] FAILED to register Raw Input Devices!");
        } else {
            crate::audit_log("[MONITOR] Successfully registered Raw Input Devices");
        }
    }
}

unsafe fn check_hid_touch(device_handle: winapi::shared::ntdef::HANDLE, raw_hid: &RAWHID) -> bool {
    let mut preparsed_data_size: UINT = 0;
    if GetRawInputDeviceInfoW(device_handle, RIDI_PREPARSEDDATA, std::ptr::null_mut(), &mut preparsed_data_size) != 0 {
        return false;
    }
    if preparsed_data_size == 0 {
        return false;
    }

    let mut preparsed_data = vec![0u8; preparsed_data_size as usize];
    if GetRawInputDeviceInfoW(
        device_handle,
        RIDI_PREPARSEDDATA,
        preparsed_data.as_mut_ptr() as *mut _,
        &mut preparsed_data_size,
    ) == !0
    {
        return false;
    }

    let p_preparsed = preparsed_data.as_mut_ptr() as PVOID;
    let mut caps: HIDP_CAPS = std::mem::zeroed();
    if HidP_GetCaps(p_preparsed, &mut caps) != HIDP_STATUS_SUCCESS {
        return false;
    }

    let button_caps_length = caps.NumberInputButtonCaps;
    if button_caps_length == 0 {
        return false;
    }
    let mut button_caps = vec![std::mem::zeroed::<HIDP_BUTTON_CAPS>(); button_caps_length as usize];
    let mut actual_button_caps_length = button_caps_length;
    if HidP_GetButtonCaps(
        HidP_Input,
        button_caps.as_mut_ptr(),
        &mut actual_button_caps_length,
        p_preparsed,
    ) != HIDP_STATUS_SUCCESS
    {
        return false;
    }

    let report_ptr = &raw_hid.bRawData as *const u8;
    let report_len = raw_hid.dwSizeHid;
    let mut touch_detected = false;

    for r in 0..raw_hid.dwCount {
        let current_report = report_ptr.add((r * report_len) as usize) as *mut u8;
        let mut nodes_length = caps.NumberLinkCollectionNodes;
        
        if nodes_length > 0 {
            let mut nodes = vec![std::mem::zeroed::<HIDP_LINK_COLLECTION_NODE>(); nodes_length as usize];
            if HidP_GetLinkCollectionNodes(nodes.as_mut_ptr(), &mut nodes_length, p_preparsed) == HIDP_STATUS_SUCCESS {
                for collection_id in 1..=nodes_length as USHORT {
                    let mut ts_len = 1;
                    let mut ts_usage = [0u16; 1];
                    let ts_status = HidP_GetUsages(
                        HidP_Input,
                        0x0D,
                        collection_id,
                        ts_usage.as_mut_ptr(),
                        &mut ts_len,
                        p_preparsed,
                        current_report,
                        report_len,
                    );
                    
                    let is_tip_active = ts_status == HIDP_STATUS_SUCCESS && ts_len > 0 && ts_usage[0] == 0x42;
                    
                    if is_tip_active {
                        let mut conf_len = 1;
                        let mut conf_usage = [0u16; 1];
                        let conf_status = HidP_GetUsages(
                            HidP_Input,
                            0x0D,
                            collection_id,
                            conf_usage.as_mut_ptr(),
                            &mut conf_len,
                            p_preparsed,
                            current_report,
                            report_len,
                        );
                        
                        let is_conf_active = conf_status == HIDP_STATUS_SUCCESS && conf_len > 0 && conf_usage[0] == 0x47;
                        
                        let has_conf_cap = button_caps.iter().take(actual_button_caps_length as usize).any(|cap| {
                            cap.UsagePage == 0x0D && cap.LinkCollection == collection_id && 
                            (cap.IsRange == 0 && cap.u.NotRange().Usage == 0x47)
                        });
                        
                        if !has_conf_cap || is_conf_active {
                            touch_detected = true;
                            break;
                        }
                    }
                }
            }
        } else {
            let mut ts_len = 10;
            let mut ts_usages = vec![0u16; ts_len];
            let ts_status = HidP_GetUsages(
                HidP_Input,
                0x0D,
                0,
                ts_usages.as_mut_ptr(),
                &mut ts_len,
                p_preparsed,
                current_report,
                report_len,
            );
            if ts_status == HIDP_STATUS_SUCCESS && ts_len > 0 {
                touch_detected = true;
            }
        }
        
        if touch_detected {
            break;
        }
    }
    
    touch_detected
}
