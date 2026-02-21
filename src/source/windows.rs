use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

use core_foundation::base::TCFType;
use core_foundation::number::{CFNumber, CFNumberRef};
use core_foundation::string::{CFString, CFStringRef};

use crate::icon;
use crate::platform;
use crate::source::{IconData, Source, SourceItem};

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

/// Source that lists on-screen windows via CGWindowListCopyWindowInfo.
pub struct WindowsSource;

impl WindowsSource {
    pub fn new() -> Self {
        Self
    }

    fn scan_windows() -> Vec<SourceItem> {
        let mut items = Vec::new();
        let self_pid = std::process::id() as i64;
        let mut icon_cache: HashMap<i64, Option<IconData>> = HashMap::new();

        unsafe {
            let options = K_CG_WINDOW_LIST_ON_SCREEN_ONLY | K_CG_WINDOW_LIST_EXCLUDE_DESKTOP;
            let array = CGWindowListCopyWindowInfo(options, 0);
            if array.is_null() {
                return items;
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

                // Icon from bundle path (cached per PID)
                let icon = icon_cache
                    .entry(pid)
                    .or_insert_with(|| {
                        platform::macos::bundle_path_for_pid(pid as i32)
                            .and_then(|p| icon::load_app_icon(&PathBuf::from(p)))
                    })
                    .clone();

                items.push(SourceItem {
                    title: owner,
                    subtitle: Some(title),
                    exec_path: format!("window:pid={}:wid={}", pid, wid),
                    source_name: "windows".to_string(),
                    icon,
                });
            }

            CFRelease(array);
        }

        items.sort_by(|a, b| a.title.cmp(&b.title));
        items
    }
}

impl Source for WindowsSource {
    fn name(&self) -> &str {
        "windows"
    }

    fn load(&self) -> Pin<Box<dyn Future<Output = Vec<SourceItem>> + Send>> {
        Box::pin(async { tokio::task::spawn_blocking(Self::scan_windows).await.unwrap() })
    }

    fn execute(&self, item: &SourceItem) -> Result<(), Box<dyn std::error::Error>> {
        // exec_path format: "window:pid={pid}:wid={wid}"
        let pid = item
            .exec_path
            .split(':')
            .find_map(|part| part.strip_prefix("pid="))
            .and_then(|s| s.parse::<i32>().ok())
            .ok_or("invalid window exec_path")?;

        platform::macos::focus_window(pid);
        Ok(())
    }
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
