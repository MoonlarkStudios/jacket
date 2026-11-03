//! Bind iced controls to the authored semantic palette once per appearance change.
use crate::theme::{self, Colors, Derived};
use iced::theme::palette::{self, Pair};
use iced::theme::Palette;
use iced::{Color, Theme};

pub fn build(name: &str, colors: Colors, derived: Derived) -> Theme {
    let base = Palette {
        background: colors.background,
        text: colors.text,
        primary: colors.accent,
        success: colors.success,
        warning: colors.warning,
        danger: colors.danger,
    };
    Theme::custom_with_fn(name.to_owned(), base, move |base| {
        let mut extended = palette::Extended::generate(base);
        // Preserve literal author choices: Pair::new may silently replace text colors.
        let pair = |color, text| Pair { color, text };
        extended.background = palette::Background {
            base: pair(colors.background, colors.text),
            weakest: pair(colors.chrome, colors.text),
            weaker: pair(colors.panel, colors.text),
            weak: pair(colors.panel, colors.text),
            neutral: pair(colors.raised, colors.text),
            strong: pair(derived.outline, colors.text),
            stronger: pair(colors.text_muted, colors.background),
            strongest: pair(colors.text, colors.background),
        };
        extended.primary = palette::Primary {
            base: pair(colors.accent, colors.accent_text),
            weak: pair(derived.selection_fill, colors.text),
            strong: pair(colors.accent_strong, colors.accent_text),
        };
        extended.secondary = palette::Secondary {
            base: pair(colors.text_muted, colors.background),
            weak: pair(colors.raised, colors.text),
            strong: pair(colors.text, colors.background),
        };
        let foreground = |color: Color| {
            if theme::contrast(color, colors.text) > theme::contrast(color, colors.background) {
                colors.text
            } else {
                colors.background
            }
        };
        extended.success = palette::Success {
            base: pair(colors.success, foreground(colors.success)),
            weak: pair(
                theme::blend(colors.success, colors.panel, 0.08),
                colors.success,
            ),
            strong: pair(colors.success, foreground(colors.success)),
        };
        extended.warning = palette::Warning {
            base: pair(colors.warning, foreground(colors.warning)),
            weak: pair(
                theme::blend(colors.warning, colors.panel, 0.08),
                colors.warning,
            ),
            strong: pair(colors.warning, foreground(colors.warning)),
        };
        extended.danger = palette::Danger {
            base: pair(colors.danger, foreground(colors.danger)),
            weak: pair(
                theme::blend(colors.danger, colors.panel, 0.08),
                colors.danger,
            ),
            strong: pair(colors.danger, foreground(colors.danger)),
        };
        extended
    })
}
