use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use iced::futures::SinkExt;
use iced::stream::channel;
use iced::Subscription;

use heats_core::config::ModeConfig;

/// Message emitted by the hotkey subscription
#[derive(Debug, Clone)]
pub struct HotkeyMessage {
    pub mode_name: String,
}

/// Initialize the global hotkey manager on the main thread.
/// Returns the manager (must be kept alive!) and a mapping of hotkey_id → mode_name.
pub fn init_manager(modes: &[ModeConfig]) -> (GlobalHotKeyManager, Vec<(u32, String)>) {
    let manager = GlobalHotKeyManager::new().expect("Failed to create GlobalHotKeyManager");
    let mut mappings = Vec::new();

    for mode in modes {
        let (mods, code) = parse_hotkey_str(&mode.hotkey);
        let hotkey = HotKey::new(Some(mods), code);
        manager
            .register(hotkey)
            .unwrap_or_else(|e| panic!("Failed to register hotkey for mode '{}': {e}", mode.name));
        tracing::info!(
            "Registered hotkey '{}' for mode '{}'",
            mode.hotkey,
            mode.name
        );
        mappings.push((hotkey.id(), mode.name.clone()));
    }

    (manager, mappings)
}

/// Create an iced Subscription that listens for global hotkey events
pub fn subscription(mappings: Vec<(u32, String)>) -> Subscription<HotkeyMessage> {
    Subscription::run_with(mappings, hotkey_stream)
}

#[allow(clippy::ptr_arg)]
fn hotkey_stream(mappings: &Vec<(u32, String)>) -> impl iced::futures::Stream<Item = HotkeyMessage> {
    let mappings = mappings.clone();
    channel(
        32,
        move |mut sender: iced::futures::channel::mpsc::Sender<HotkeyMessage>| async move {
            let receiver = GlobalHotKeyEvent::receiver();
            loop {
                if let Ok(event) = receiver.try_recv() {
                    if event.state == HotKeyState::Pressed {
                        for (id, mode_name) in &mappings {
                            if event.id == *id {
                                let _ = sender
                                    .send(HotkeyMessage {
                                        mode_name: mode_name.clone(),
                                    })
                                    .await;
                                break;
                            }
                        }
                    }
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        },
    )
}

/// Parse a hotkey string like "Cmd+Semicolon" into (Modifiers, Code)
fn parse_hotkey_str(s: &str) -> (Modifiers, Code) {
    let parts: Vec<&str> = s.split('+').collect();
    // Last part is the key, everything before is modifiers
    let key_str = parts.last().expect("Empty hotkey string");
    let mod_parts = &parts[..parts.len() - 1];

    let mut mods = Modifiers::empty();
    for part in mod_parts {
        match part.trim().to_lowercase().as_str() {
            "cmd" | "super" | "command" | "meta" => mods |= Modifiers::SUPER,
            "ctrl" | "control" => mods |= Modifiers::CONTROL,
            "alt" | "option" => mods |= Modifiers::ALT,
            "shift" => mods |= Modifiers::SHIFT,
            _ => tracing::warn!("Unknown modifier: {}", part),
        }
    }

    let code = parse_code(key_str.trim());
    (mods, code)
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
