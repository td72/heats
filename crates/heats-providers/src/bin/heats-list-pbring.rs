use heats_core::source::DmenuItem;
use std::process::Command;

fn main() {
    let output = Command::new("pbring")
        .arg("list")
        .output()
        .expect("failed to run pbring list");

    let stdout = String::from_utf8_lossy(&output.stdout);

    for line in stdout.lines() {
        // TSV: id \t timestamp \t type \t preview
        let cols: Vec<&str> = line.splitn(4, '\t').collect();
        if cols.len() < 4 {
            continue;
        }
        let (id, timestamp, content_type, preview) = (cols[0], cols[1], cols[2], cols[3]);

        let item = DmenuItem {
            title: preview.to_string(),
            subtitle: Some(format!("{} · {}", content_type, timestamp)),
            icon_path: None,
            data: Some(serde_json::json!({ "id": id })),
        };
        println!("{}", serde_json::to_string(&item).unwrap());
    }
}
