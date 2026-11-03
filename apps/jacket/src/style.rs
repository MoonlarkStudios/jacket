use iced::widget::{button, container};
use iced::{border, color, Background, Border, Color, Theme};

pub const INK: Color = color!(0x282725);
pub const PAPER: Color = color!(0xF4EFE5);
pub const COPPER: Color = color!(0xD47749);
pub fn muted(dark: bool) -> Color {
    if dark {
        color!(0xBEB5A7)
    } else {
        color!(0x716B62)
    }
}

pub fn theme(dark: bool) -> Theme {
    Theme::custom(
        if dark {
            "Jacket · Ink"
        } else {
            "Jacket · Paper"
        },
        iced::theme::Palette {
            background: if dark { INK } else { PAPER },
            text: if dark { PAPER } else { INK },
            primary: if dark { COPPER } else { color!(0x914323) },
            success: color!(0x458164),
            danger: color!(0xBE4940),
            warning: color!(0xB15D35),
        },
    )
}

pub fn card(theme: &Theme) -> container::Style {
    let dark = theme.extended_palette().is_dark;
    container::Style {
        background: Some(Background::Color(if dark {
            color!(0x32312E)
        } else {
            color!(0xFFFCF7)
        })),
        border: Border {
            color: if dark {
                color!(0x494640)
            } else {
                color!(0xE4DBCE)
            },
            width: 1.0,
            radius: border::Radius::from(12.0),
        },
        ..container::Style::default()
    }
}

pub fn toolbar(theme: &Theme) -> container::Style {
    container::Style {
        border: Border {
            radius: border::Radius::from(8.0),
            ..Border::default()
        },
        ..card(theme)
    }
}

pub fn primary(theme: &Theme, status: button::Status) -> button::Style {
    let mut result = button::primary(theme, status);
    if !matches!(status, button::Status::Disabled) {
        result.background = Some(Background::Color(
            if matches!(status, button::Status::Hovered) {
                color!(0xDF8A5D)
            } else {
                COPPER
            },
        ));
        result.text_color = INK;
    }
    result.border.radius = border::Radius::from(8.0);
    result
}
