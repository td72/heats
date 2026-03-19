use iced::keyboard;

use crate::key_parse::{self, ModifierKind};

/// A parsed keybinding (modifier keys + key)
#[derive(Debug, Clone)]
pub struct KeyBinding {
    pub key: keyboard::Key,
    pub modifiers: keyboard::Modifiers,
}

/// Parse a keybinding string like "Alt+Enter" or "Ctrl+O" into a KeyBinding.
/// Supported modifiers: Alt, Ctrl, Shift, Cmd/Super
/// Supported keys: Enter/Return, Escape, Tab, Space, single characters (A-Z, 0-9, etc.)
pub fn parse(s: &str) -> Option<KeyBinding> {
    let parts: Vec<&str> = s.split('+').map(|p| p.trim()).collect();
    if parts.is_empty() {
        return None;
    }

    let mut modifiers = keyboard::Modifiers::empty();
    let key_str = parts.last()?;

    for &part in &parts[..parts.len() - 1] {
        match key_parse::parse_modifier(part) {
            Some(ModifierKind::Super) => modifiers |= keyboard::Modifiers::LOGO,
            Some(ModifierKind::Ctrl) => modifiers |= keyboard::Modifiers::CTRL,
            Some(ModifierKind::Alt) => modifiers |= keyboard::Modifiers::ALT,
            Some(ModifierKind::Shift) => modifiers |= keyboard::Modifiers::SHIFT,
            None => return None,
        }
    }

    let key = parse_key(key_str)?;
    Some(KeyBinding { key, modifiers })
}

fn parse_key(s: &str) -> Option<keyboard::Key> {
    use keyboard::key::Named;

    match s.to_lowercase().as_str() {
        "enter" | "return" => Some(keyboard::Key::Named(Named::Enter)),
        "escape" | "esc" => Some(keyboard::Key::Named(Named::Escape)),
        "tab" => Some(keyboard::Key::Named(Named::Tab)),
        "space" => Some(keyboard::Key::Named(Named::Space)),
        "backspace" => Some(keyboard::Key::Named(Named::Backspace)),
        "delete" => Some(keyboard::Key::Named(Named::Delete)),
        "arrowup" | "up" => Some(keyboard::Key::Named(Named::ArrowUp)),
        "arrowdown" | "down" => Some(keyboard::Key::Named(Named::ArrowDown)),
        "arrowleft" | "left" => Some(keyboard::Key::Named(Named::ArrowLeft)),
        "arrowright" | "right" => Some(keyboard::Key::Named(Named::ArrowRight)),
        _ => {
            // Single character key
            if s.len() == 1 {
                Some(keyboard::Key::Character(s.to_lowercase().into()))
            } else {
                None
            }
        }
    }
}

/// Check if a keyboard key + modifiers matches a keybinding
pub fn matches(key: &keyboard::Key, modifiers: &keyboard::Modifiers, binding: &KeyBinding) -> bool {
    *modifiers == binding.modifiers && *key == binding.key
}
