use std::process;

use heats_core::ipc;
use heats_core::ipc::client::IpcFormat;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Parse --format flag
    let format = if args.iter().any(|a| a == "--format") {
        let idx = args.iter().position(|a| a == "--format").unwrap();
        match args.get(idx + 1).map(|s| s.as_str()) {
            Some("jsonl") => IpcFormat::Jsonl,
            Some("text") => IpcFormat::Text,
            Some(other) => {
                eprintln!("heats: unknown format '{other}', expected 'text' or 'jsonl'");
                process::exit(2);
            }
            None => {
                eprintln!("heats: --format requires a value");
                process::exit(2);
            }
        }
    } else {
        IpcFormat::Text
    };

    let items = ipc::client::read_stdin_items();

    if items.is_empty() {
        eprintln!("heats: no items received from stdin");
        process::exit(2);
    }

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to create tokio runtime");

    match rt.block_on(ipc::client::send_and_receive(items, format)) {
        Ok(Some(selected)) => {
            println!("{selected}");
            process::exit(0);
        }
        Ok(None) => {
            // Cancelled (Escape)
            process::exit(1);
        }
        Err(e) => {
            eprintln!("heats: {e}");
            process::exit(2);
        }
    }
}
