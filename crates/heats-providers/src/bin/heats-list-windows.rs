use heats_core::platform::macos::ensure_screen_capture_access;
use heats_core::source::windows::scan_windows_raw;
use heats_core::source::DmenuItem;

fn main() {
    ensure_screen_capture_access();

    let entries = scan_windows_raw();
    for entry in entries {
        let item = DmenuItem {
            title: entry.owner,
            subtitle: Some(entry.title),
            icon_path: entry.bundle_path,
            data: Some(serde_json::json!({
                "pid": entry.pid,
                "wid": entry.wid,
            })),
        };
        println!("{}", serde_json::to_string(&item).unwrap());
    }
}
