use std::process::Command;

use core_graphics::display::CGDisplay;
use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl, Encode, Encoding};

/// Open a macOS application by its path using the `open` command
pub fn open_application(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    Command::new("open").arg("-a").arg(path).spawn()?;
    Ok(())
}

/// Get the bounds of the display that has keyboard focus (NSScreen.mainScreen).
/// NSScreen.mainScreen returns the screen containing the window currently receiving keyboard events.
pub fn focused_display_bounds() -> (f64, f64, f64, f64) {
    unsafe {
        let main_screen: *mut Object = msg_send![class!(NSScreen), mainScreen];
        if !main_screen.is_null() {
            if let Some(display_id) = screen_display_id(main_screen) {
                let display = CGDisplay::new(display_id);
                let bounds = display.bounds();
                tracing::debug!(
                    "focused_display_bounds: NSScreen.mainScreen → CGDisplayID={}, bounds=({}, {}, {}, {})",
                    display_id, bounds.origin.x, bounds.origin.y, bounds.size.width, bounds.size.height
                );
                return (
                    bounds.origin.x,
                    bounds.origin.y,
                    bounds.size.width,
                    bounds.size.height,
                );
            }
        }
    }

    tracing::warn!("focused_display_bounds: NSScreen.mainScreen unavailable, falling back");
    fallback_main_display()
}

/// Extract CGDirectDisplayID from an NSScreen via deviceDescription["NSScreenNumber"].
unsafe fn screen_display_id(screen: *mut Object) -> Option<u32> {
    let desc: *mut Object = msg_send![screen, deviceDescription];
    let key: *mut Object =
        msg_send![class!(NSString), stringWithUTF8String: b"NSScreenNumber\0".as_ptr()];
    let screen_number: *mut Object = msg_send![desc, objectForKey: key];
    if screen_number.is_null() {
        None
    } else {
        let id: u32 = msg_send![screen_number, unsignedIntValue];
        Some(id)
    }
}

/// Get the bounds of a display by name (substring match).
/// Returns CG coordinates (origin at top-left of main display).
pub fn display_bounds_by_name(name: &str) -> (f64, f64, f64, f64) {
    let screens = list_screens();
    let name_lower = name.to_lowercase();

    for (screen_name, cg_display_id) in &screens {
        if screen_name.to_lowercase().contains(&name_lower) {
            let display = CGDisplay::new(*cg_display_id);
            let bounds = display.bounds();
            tracing::info!(
                "Matched display \"{}\" for query \"{}\", bounds: ({}, {}, {}, {})",
                screen_name,
                name,
                bounds.origin.x,
                bounds.origin.y,
                bounds.size.width,
                bounds.size.height
            );
            return (
                bounds.origin.x,
                bounds.origin.y,
                bounds.size.width,
                bounds.size.height,
            );
        }
    }

    let available: Vec<&str> = screens.iter().map(|(n, _)| n.as_str()).collect();
    tracing::warn!(
        "Display \"{}\" not found. Available: {:?}",
        name,
        available
    );
    fallback_main_display()
}

/// List all screens: (name, CGDirectDisplayID).
/// Uses NSScreen.localizedName for names, NSScreen.deviceDescription for CGDirectDisplayID.
fn list_screens() -> Vec<(String, u32)> {
    let mut result = Vec::new();

    unsafe {
        let screens: *mut Object = msg_send![class!(NSScreen), screens];
        let count: usize = msg_send![screens, count];

        for i in 0..count {
            let screen: *mut Object = msg_send![screens, objectAtIndex: i];

            // Get display name via NSScreen.localizedName (macOS 10.15+)
            let ns_name: *mut Object = msg_send![screen, localizedName];
            let c_str: *const i8 = msg_send![ns_name, UTF8String];
            let name = if c_str.is_null() {
                format!("Display {}", i + 1)
            } else {
                std::ffi::CStr::from_ptr(c_str)
                    .to_str()
                    .unwrap_or("Unknown")
                    .to_string()
            };

            let display_id: u32 = screen_display_id(screen).unwrap_or(0);

            tracing::debug!("Screen {}: \"{}\" (CGDisplayID={})", i, name, display_id);
            result.push((name, display_id));
        }
    }

    result
}

// ---- Native NSWindow show/hide (Raycast-style) ----

/// NSPoint / NSSize / NSRect for passing to msg_send!
#[repr(C)]
#[derive(Copy, Clone)]
struct NSPoint {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct NSSize {
    width: f64,
    height: f64,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct NSRect {
    origin: NSPoint,
    size: NSSize,
}

unsafe impl Encode for NSPoint {
    fn encode() -> Encoding {
        let code = format!("{{CGPoint={}{}}}", f64::encode().as_str(), f64::encode().as_str());
        unsafe { Encoding::from_str(&code) }
    }
}

unsafe impl Encode for NSSize {
    fn encode() -> Encoding {
        let code = format!("{{CGSize={}{}}}", f64::encode().as_str(), f64::encode().as_str());
        unsafe { Encoding::from_str(&code) }
    }
}

unsafe impl Encode for NSRect {
    fn encode() -> Encoding {
        let code = format!(
            "{{CGRect={}{}}}",
            NSPoint::encode().as_str(),
            NSSize::encode().as_str()
        );
        unsafe { Encoding::from_str(&code) }
    }
}

/// Find the NSWindow with title "Heats".
unsafe fn find_heats_window() -> Option<*mut Object> {
    let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
    let windows: *mut Object = msg_send![app, windows];
    let count: usize = msg_send![windows, count];

    for i in 0..count {
        let window: *mut Object = msg_send![windows, objectAtIndex: i];
        let title: *mut Object = msg_send![window, title];
        if title.is_null() {
            continue;
        }
        let c_str: *const i8 = msg_send![title, UTF8String];
        if c_str.is_null() {
            continue;
        }
        let title_str = std::ffi::CStr::from_ptr(c_str).to_str().unwrap_or("");
        if title_str == "Heats" {
            return Some(window);
        }
    }
    None
}

/// Show the Heats window at the center of the given display (CG coordinates).
/// Uses NSWindow.setFrame + makeKeyAndOrderFront (like Raycast).
pub fn native_show_window(display: &(f64, f64, f64, f64), win_w: f64, win_h: f64) {
    let (disp_x, disp_y, disp_w, disp_h) = *display;

    // Center position in CG coordinates (origin = top-left of main display, y down)
    let cg_x = disp_x + (disp_w - win_w) / 2.0;
    let cg_y = disp_y + (disp_h - win_h) / 3.0;

    // Convert CG → AppKit coordinates (origin = bottom-left of main display, y up)
    let main_height = CGDisplay::main().bounds().size.height;
    let appkit_x = cg_x;
    let appkit_y = main_height - cg_y - win_h;

    tracing::debug!(
        "native_show_window: CG({}, {}), AppKit({}, {}), main_h={}",
        cg_x, cg_y, appkit_x, appkit_y, main_height
    );

    unsafe {
        if let Some(window) = find_heats_window() {
            let frame = NSRect {
                origin: NSPoint { x: appkit_x, y: appkit_y },
                size: NSSize { width: win_w, height: win_h },
            };
            let display_flag: i8 = 1; // YES
            let animate_flag: i8 = 0; // NO
            let _: () = msg_send![window, setFrame:frame display:display_flag animate:animate_flag];
            let _: () = msg_send![window, makeKeyAndOrderFront: std::ptr::null::<Object>()];
        } else {
            tracing::warn!("native_show_window: Heats window not found");
        }
    }
}

/// Hide the Heats window using NSWindow.orderOut (Raycast-style).
pub fn native_hide_window() {
    unsafe {
        if let Some(window) = find_heats_window() {
            let _: () = msg_send![window, orderOut: std::ptr::null::<Object>()];
        }
    }
}

fn fallback_main_display() -> (f64, f64, f64, f64) {
    let main = CGDisplay::main();
    let b = main.bounds();
    (b.origin.x, b.origin.y, b.size.width, b.size.height)
}

fn mouse_position() -> (f64, f64) {
    let source = match core_graphics::event_source::CGEventSource::new(
        core_graphics::event_source::CGEventSourceStateID::CombinedSessionState,
    ) {
        Ok(s) => s,
        Err(_) => return (0.0, 0.0),
    };
    let event = match core_graphics::event::CGEvent::new(source) {
        Ok(e) => e,
        Err(_) => return (0.0, 0.0),
    };
    let point = event.location();
    (point.x, point.y)
}
