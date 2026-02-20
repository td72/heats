use std::process;

use heats::ipc;

fn main() {
    let items = ipc::client::read_stdin_items();

    if items.is_empty() {
        eprintln!("heats: no items received from stdin");
        process::exit(2);
    }

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to create tokio runtime");

    match rt.block_on(ipc::client::send_and_receive(items)) {
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
