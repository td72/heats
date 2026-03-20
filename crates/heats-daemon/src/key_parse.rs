/// Common modifier kind shared between global_hotkey and iced keyboard types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModifierKind {
    Super,
    Ctrl,
    Alt,
    Shift,
}

/// Parse a modifier name string into a ModifierKind.
/// Accepts common aliases: "cmd"/"super"/"command"/"meta", "ctrl"/"control",
/// "alt"/"option", "shift".
pub fn parse_modifier(s: &str) -> Option<ModifierKind> {
    match s.to_lowercase().as_str() {
        "cmd" | "super" | "command" | "meta" => Some(ModifierKind::Super),
        "ctrl" | "control" => Some(ModifierKind::Ctrl),
        "alt" | "option" => Some(ModifierKind::Alt),
        "shift" => Some(ModifierKind::Shift),
        _ => None,
    }
}
