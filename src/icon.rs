use std::path::Path;
use std::sync::Arc;

use icns::{IconFamily, IconType, PixelFormat};

use crate::source::IconData;

/// Load an app icon from a .app bundle as 32x32 RGBA pixel data.
///
/// Reads `Contents/Info.plist` → `CFBundleIconFile` → `Contents/Resources/{icon}.icns`,
/// then extracts 32x32 RGBA pixels. Returns `None` on any failure.
pub fn load_app_icon(app_path: &Path) -> Option<IconData> {
    let icon_file = icon_file_from_plist(app_path)?;
    let icns_path = if icon_file.ends_with(".icns") {
        app_path.join("Contents/Resources").join(&icon_file)
    } else {
        app_path
            .join("Contents/Resources")
            .join(format!("{icon_file}.icns"))
    };

    load_icns_rgba(&icns_path)
}

/// Read `CFBundleIconFile` from the app's Info.plist.
fn icon_file_from_plist(app_path: &Path) -> Option<String> {
    let plist_path = app_path.join("Contents/Info.plist");
    let plist = plist::Value::from_file(plist_path).ok()?;
    plist
        .as_dictionary()
        .and_then(|dict| dict.get("CFBundleIconFile"))
        .and_then(|val| val.as_string())
        .map(|s| s.to_string())
}

/// Load an .icns file and extract RGBA pixel data.
/// Tries multiple sizes from small to large; the image widget scales to 24px display.
fn load_icns_rgba(icns_path: &Path) -> Option<IconData> {
    let file = std::io::BufReader::new(std::fs::File::open(icns_path).ok()?);
    let icon_family = IconFamily::read(file).ok()?;

    // Prefer smaller sizes first (less memory), fall back to larger ones.
    // Modern apps often only ship 128x128+ or retina variants.
    let types_to_try = [
        IconType::RGBA32_32x32,
        IconType::RGB24_32x32,
        IconType::RGBA32_32x32_2x,
        IconType::RGB24_48x48,
        IconType::RGBA32_128x128,
        IconType::RGB24_128x128,
        IconType::RGBA32_128x128_2x,
        IconType::RGBA32_256x256,
        IconType::RGBA32_256x256_2x,
        IconType::RGBA32_512x512,
        IconType::RGBA32_512x512_2x,
    ];

    for icon_type in types_to_try {
        if let Ok(image) = icon_family.get_icon_with_type(icon_type) {
            let rgba = image.convert_to(PixelFormat::RGBA);
            let w = rgba.width();
            let h = rgba.height();
            return Some(IconData::Rgba {
                width: w,
                height: h,
                pixels: Arc::new(rgba.into_data().into_vec()),
            });
        }
    }

    None
}
