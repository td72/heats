use iced::widget::{container, text, text_input};
use iced::{Border, Color, Shadow, Theme};

/// Semi-transparent dark background color for the launcher window
pub const BACKGROUND: Color = Color {
    r: 0.12,
    g: 0.12,
    b: 0.15,
    a: 0.92,
};

/// Slightly lighter surface color for the search input
const SURFACE: Color = Color {
    r: 0.18,
    g: 0.18,
    b: 0.22,
    a: 1.0,
};

/// Accent color for selected items
const ACCENT: Color = Color {
    r: 0.35,
    g: 0.55,
    b: 0.85,
    a: 1.0,
};

/// Text color
const TEXT_PRIMARY: Color = Color {
    r: 0.9,
    g: 0.9,
    b: 0.92,
    a: 1.0,
};

const TEXT_SECONDARY: Color = Color {
    r: 0.55,
    g: 0.55,
    b: 0.6,
    a: 1.0,
};

/// Style for the main container wrapping the entire launcher
pub fn main_container(theme: &Theme) -> container::Style {
    let _ = theme;
    container::Style {
        background: Some(BACKGROUND.into()),
        border: Border {
            color: Color {
                r: 0.3,
                g: 0.3,
                b: 0.35,
                a: 0.5,
            },
            width: 1.0,
            radius: 12.0.into(),
        },
        shadow: Shadow {
            color: Color::BLACK,
            offset: iced::Vector::new(0.0, 4.0),
            blur_radius: 20.0,
        },
        text_color: Some(TEXT_PRIMARY),
        snap: false,
    }
}

/// Style for the search text input
pub fn search_input(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let _ = theme;
    let focused = matches!(status, text_input::Status::Focused { .. });
    text_input::Style {
        background: SURFACE.into(),
        border: Border {
            color: if focused { ACCENT } else { Color::TRANSPARENT },
            width: if focused { 2.0 } else { 0.0 },
            radius: 8.0.into(),
        },
        icon: TEXT_SECONDARY,
        placeholder: TEXT_SECONDARY,
        value: TEXT_PRIMARY,
        selection: Color {
            r: ACCENT.r,
            g: ACCENT.g,
            b: ACCENT.b,
            a: 0.3,
        },
    }
}

/// Style for a result row (not selected)
pub fn result_row(theme: &Theme) -> container::Style {
    let _ = theme;
    container::Style {
        background: None,
        text_color: Some(TEXT_PRIMARY),
        ..container::Style::default()
    }
}

/// Style for the selected result row
pub fn result_row_selected(theme: &Theme) -> container::Style {
    let _ = theme;
    container::Style {
        background: Some(
            Color {
                r: ACCENT.r,
                g: ACCENT.g,
                b: ACCENT.b,
                a: 0.2,
            }
            .into(),
        ),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 6.0.into(),
        },
        text_color: Some(TEXT_PRIMARY),
        ..container::Style::default()
    }
}

/// Style for result item name text
pub fn result_name(_theme: &Theme) -> text::Style {
    text::Style {
        color: Some(TEXT_PRIMARY),
    }
}

/// Style for result item path/subtitle text
pub fn result_subtitle(_theme: &Theme) -> text::Style {
    text::Style {
        color: Some(TEXT_SECONDARY),
    }
}
