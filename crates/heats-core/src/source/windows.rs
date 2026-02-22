use core_foundation::base::TCFType;
use core_foundation::number::{CFNumber, CFNumberRef};
use core_foundation::string::{CFString, CFStringRef};

use crate::platform;

use std::ffi::c_void;

extern "C" {
    fn CGWindowListCopyWindowInfo(option: u32, relativeToWindow: u32) -> *const c_void;
    fn CFArrayGetCount(array: *const c_void) -> isize;
    fn CFArrayGetValueAtIndex(array: *const c_void, idx: isize) -> *const c_void;
    fn CFDictionaryGetValue(dict: *const c_void, key: *const c_void) -> *const c_void;
    fn CFRelease(cf: *const c_void);
}

const K_CG_WINDOW_LIST_ON_SCREEN_ONLY: u32 = 1 << 0;
const K_CG_WINDOW_LIST_EXCLUDE_DESKTOP: u32 = 1 << 4;

/// A raw window entry (no icon loading)
pub struct WindowEntry {
    pub owner: String,
    pub title: String,
    pub pid: i64,
    pub wid: i64,
    pub bundle_path: Option<String>,
}

/// Scan on-screen windows via CGWindowListCopyWindowInfo.
/// Returns raw data without loading icons.
pub fn scan_windows_raw() -> Vec<WindowEntry> {
    let mut entries = Vec::new();
    let self_pid = std::process::id() as i64;

    unsafe {
        let options = K_CG_WINDOW_LIST_ON_SCREEN_ONLY | K_CG_WINDOW_LIST_EXCLUDE_DESKTOP;
        let array = CGWindowListCopyWindowInfo(options, 0);
        if array.is_null() {
            return entries;
        }

        let key_layer = CFString::new("kCGWindowLayer");
        let key_pid = CFString::new("kCGWindowOwnerPID");
        let key_name = CFString::new("kCGWindowName");
        let key_owner = CFString::new("kCGWindowOwnerName");
        let key_number = CFString::new("kCGWindowNumber");

        let count = CFArrayGetCount(array);
        for i in 0..count {
            let dict = CFArrayGetValueAtIndex(array, i);
            if dict.is_null() {
                continue;
            }

            // Filter: layer == 0 (normal windows only)
            let layer = dict_get_number(dict, &key_layer).unwrap_or(-1);
            if layer != 0 {
                continue;
            }

            // Filter: exclude own PID
            let pid = dict_get_number(dict, &key_pid).unwrap_or(0);
            if pid == self_pid {
                continue;
            }

            // Filter: must have a non-empty window name
            let title = match dict_get_string(dict, &key_name) {
                Some(s) if !s.is_empty() => s,
                _ => continue,
            };

            let owner = dict_get_string(dict, &key_owner).unwrap_or_default();
            let wid = dict_get_number(dict, &key_number).unwrap_or(0);

            let bundle_path = platform::macos::bundle_path_for_pid(pid as i32);

            entries.push(WindowEntry {
                owner,
                title,
                pid,
                wid,
                bundle_path,
            });
        }

        CFRelease(array);
    }

    entries.sort_by(|a, b| a.owner.cmp(&b.owner));
    entries
}

unsafe fn dict_get_number(dict: *const c_void, key: &CFString) -> Option<i64> {
    let val = CFDictionaryGetValue(dict, key.as_concrete_TypeRef() as *const c_void);
    if val.is_null() {
        return None;
    }
    let num = CFNumber::wrap_under_get_rule(val as CFNumberRef);
    num.to_i64()
}

unsafe fn dict_get_string(dict: *const c_void, key: &CFString) -> Option<String> {
    let val = CFDictionaryGetValue(dict, key.as_concrete_TypeRef() as *const c_void);
    if val.is_null() {
        return None;
    }
    let s = CFString::wrap_under_get_rule(val as CFStringRef);
    Some(s.to_string())
}
