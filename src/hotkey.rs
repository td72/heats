use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use iced::futures::SinkExt;
use iced::stream::channel;
use iced::Subscription;

use crate::config::HotkeyConfig;

/// Message emitted by the hotkey subscription
#[derive(Debug, Clone)]
pub enum HotkeyMessage {
    TogglePressed,
    WindowsPressed,
}

/// Initialize the global hotkey manager on the main thread.
/// Returns the manager (must be kept alive!) and both registered hotkeys.
pub fn init_manager(config: &HotkeyConfig) -> (GlobalHotKeyManager, HotKey, HotKey) {
    let manager = GlobalHotKeyManager::new().expect("Failed to create GlobalHotKeyManager");

    // Primary hotkey (toggle: apps + windows)
    let modifiers = parse_modifiers(&config.modifiers);
    let code = parse_code(&config.key);
    let primary = HotKey::new(Some(modifiers), code);
    manager
        .register(primary)
        .expect("Failed to register primary hotkey");
    tracing::info!(
        "Registered primary hotkey: {}+{}",
        config.modifiers,
        config.key
    );

    // Secondary hotkey (windows only)
    let win_modifiers = parse_modifiers(&config.windows_modifiers);
    let win_code = parse_code(&config.windows_key);
    let secondary = HotKey::new(Some(win_modifiers), win_code);
    manager
        .register(secondary)
        .expect("Failed to register windows hotkey");
    tracing::info!(
        "Registered windows hotkey: {}+{}",
        config.windows_modifiers,
        config.windows_key
    );

    (manager, primary, secondary)
}

/// Create an iced Subscription that listens for global hotkey events
pub fn subscription(primary_id: u32, secondary_id: u32) -> Subscription<HotkeyMessage> {
    Subscription::run_with((primary_id, secondary_id), hotkey_stream)
}

fn hotkey_stream(ids: &(u32, u32)) -> impl iced::futures::Stream<Item = HotkeyMessage> {
    let (primary_id, secondary_id) = *ids;
    channel(32, move |mut sender: iced::futures::channel::mpsc::Sender<HotkeyMessage>| async move {
        let receiver = GlobalHotKeyEvent::receiver();
        loop {
            if let Ok(event) = receiver.try_recv() {
                if event.state == HotKeyState::Pressed {
                    if event.id == primary_id {
                        let _ = sender.send(HotkeyMessage::TogglePressed).await;
                    } else if event.id == secondary_id {
                        let _ = sender.send(HotkeyMessage::WindowsPressed).await;
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
}

fn parse_modifiers(s: &str) -> Modifiers {
    let mut mods = Modifiers::empty();
    for part in s.split('+') {
        match part.trim().to_lowercase().as_str() {
            "cmd" | "super" | "command" | "meta" => mods |= Modifiers::SUPER,
            "ctrl" | "control" => mods |= Modifiers::CONTROL,
            "alt" | "option" => mods |= Modifiers::ALT,
            "shift" => mods |= Modifiers::SHIFT,
            _ => tracing::warn!("Unknown modifier: {}", part),
        }
    }
    mods
}

fn parse_code(s: &str) -> Code {
    match s.to_lowercase().as_str() {
        "semicolon" | ";" => Code::Semicolon,
        "quote" | "'" => Code::Quote,
        "space" | " " => Code::Space,
        "enter" | "return" => Code::Enter,
        "tab" => Code::Tab,
        "a" => Code::KeyA,
        "b" => Code::KeyB,
        "c" => Code::KeyC,
        "d" => Code::KeyD,
        "e" => Code::KeyE,
        "f" => Code::KeyF,
        "g" => Code::KeyG,
        "h" => Code::KeyH,
        "i" => Code::KeyI,
        "j" => Code::KeyJ,
        "k" => Code::KeyK,
        "l" => Code::KeyL,
        "m" => Code::KeyM,
        "n" => Code::KeyN,
        "o" => Code::KeyO,
        "p" => Code::KeyP,
        "q" => Code::KeyQ,
        "r" => Code::KeyR,
        "s" => Code::KeyS,
        "t" => Code::KeyT,
        "u" => Code::KeyU,
        "v" => Code::KeyV,
        "w" => Code::KeyW,
        "x" => Code::KeyX,
        "y" => Code::KeyY,
        "z" => Code::KeyZ,
        "0" => Code::Digit0,
        "1" => Code::Digit1,
        "2" => Code::Digit2,
        "3" => Code::Digit3,
        "4" => Code::Digit4,
        "5" => Code::Digit5,
        "6" => Code::Digit6,
        "7" => Code::Digit7,
        "8" => Code::Digit8,
        "9" => Code::Digit9,
        _ => {
            tracing::warn!("Unknown key code: {}, defaulting to Semicolon", s);
            Code::Semicolon
        }
    }
}
