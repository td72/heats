use heats_core::source::applications::scan_apps;
use heats_core::source::DmenuItem;

fn main() {
    let apps = scan_apps();
    for app in apps {
        let item = DmenuItem {
            title: app.name,
            subtitle: Some(app.path.clone()),
            icon_path: Some(app.path.clone()),
            data: Some(serde_json::json!({ "path": app.path })),
        };
        println!("{}", serde_json::to_string(&item).unwrap());
    }
}
