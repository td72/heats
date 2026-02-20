use std::sync::Mutex;

use global_hotkey::GlobalHotKeyManager;

use heats::app::{self, State};
use heats::config::{self, Config};
use heats::hotkey;
use heats::ipc;

static BOOT_PARAMS: Mutex<Option<(Config, GlobalHotKeyManager, u32)>> = Mutex::new(None);

fn boot() -> (State, iced::Task<app::Message>) {
    let params = BOOT_PARAMS
        .lock()
        .unwrap()
        .take()
        .expect("boot() called more than once");
    State::new(params.0, params.1, params.2)
}

fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let config = config::load();

    // Clean up stale socket from previous run
    let sock = ipc::socket_path();
    if sock.exists() {
        let _ = std::fs::remove_file(&sock);
        tracing::info!("Removed stale socket: {}", sock.display());
    }

    // Initialize global hotkey manager on the main thread (macOS requirement)
    let (manager, registered_hotkey) = hotkey::init_manager(&config.hotkey);
    let hotkey_id = registered_hotkey.id();

    tracing::info!("Starting Heats launcher");

    *BOOT_PARAMS.lock().unwrap() = Some((config, manager, hotkey_id));

    iced::daemon(boot, State::update, State::view)
        .title(State::title)
        .subscription(State::subscription)
        .theme(State::theme)
        .style(State::style)
        .run()
}
