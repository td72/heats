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
}

/// Initialize the global hotkey manager on the main thread.
/// Returns the manager (must be kept alive!) and the registered hotkey.
pub fn init_manager(config: &HotkeyConfig) -> (GlobalHotKeyManager, HotKey) {
    let manager = GlobalHotKeyManager::new().expect("Failed to create GlobalHotKeyManager");
    let modifiers = parse_modifiers(&config.modifiers);
    let code = parse_code(&config.key);
    let hotkey = HotKey::new(Some(modifiers), code);
    manager
        .register(hotkey)
        .expect("Failed to register global hotkey");
    tracing::info!(
        "Registered global hotkey: {}+{}",
        config.modifiers,
        config.key
    );
    (manager, hotkey)
}

/// Create an iced Subscription that listens for global hotkey events
pub fn subscription(hotkey_id: u32) -> Subscription<HotkeyMessage> {
    Subscription::run_with(hotkey_id, hotkey_stream)
}

fn hotkey_stream(hotkey_id: &u32) -> impl iced::futures::Stream<Item = HotkeyMessage> {
    let hotkey_id = *hotkey_id;
    channel(32, move |mut sender: iced::futures::channel::mpsc::Sender<HotkeyMessage>| async move {
        let receiver = GlobalHotKeyEvent::receiver();
        loop {
            if let Ok(event) = receiver.try_recv() {
                if event.id == hotkey_id && event.state == HotKeyState::Pressed {
                    let _ = sender.send(HotkeyMessage::TogglePressed).await;
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
