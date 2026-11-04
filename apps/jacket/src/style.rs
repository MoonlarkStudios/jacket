//! Widget styles derived from the active semantic palette.
use crate::theme::{self, Colors, Derived};
use iced::widget::{button, container, rule, scrollable, text_input};
use iced::{Background, Border, Color, Font, Theme};

#[derive(Clone, Copy)]
pub struct Presentation {
    pub colors: Colors,
    pub derived: Derived,
}

pub fn semibold() -> Font {
    Font {
        weight: iced::font::Weight::Semibold,
        ..Font::with_name("Geist")
    }
}

pub fn scroll_direction() -> scrollable::Direction {
    // Keep a generous grab area while the visible scroller stays quiet.
    scrollable::Direction::Vertical(
        scrollable::Scrollbar::new()
            .width(10)
            .scroller_width(4)
            .margin(2),
    )
}

impl Presentation {
    pub fn surface(self, background: Color) -> container::Style {
        container::Style {
            background: Some(background.into()),
            text_color: Some(self.colors.text),
            ..container::Style::default()
        }
    }

    pub fn panel(self) -> container::Style {
        container::Style {
            border: Border {
                color: self.colors.border,
                width: 1.0,
                radius: 7.0.into(),
            },
            ..self.surface(self.colors.panel)
        }
    }

    pub fn badge(self) -> container::Style {
        container::Style {
            background: Some(theme::blend(self.colors.accent, self.colors.panel, 0.10).into()),
            text_color: Some(self.colors.accent),
            border: Border {
                radius: 4.0.into(),
                ..Border::default()
            },
            ..container::Style::default()
        }
    }

    pub fn line(self) -> impl Fn(&Theme) -> rule::Style {
        move |theme| {
            let mut style = rule::default(theme);
            style.color = self.colors.border;
            style
        }
    }

    pub fn primary(self) -> impl Fn(&Theme, button::Status) -> button::Style {
        move |_, status| {
            let background = match status {
                button::Status::Hovered => theme::blend(self.colors.accent, self.colors.text, 0.90),
                button::Status::Pressed => {
                    theme::blend(self.colors.accent, self.colors.background, 0.85)
                }
                button::Status::Disabled => self.colors.raised,
                button::Status::Active => self.colors.accent,
            };
            button::Style {
                background: Some(background.into()),
                text_color: if status == button::Status::Disabled {
                    self.derived.disabled
                } else {
                    self.colors.accent_text
                },
                border: Border {
                    radius: 5.0.into(),
                    ..Border::default()
                },
                ..button::Style::default()
            }
        }
    }

    pub fn secondary(self) -> impl Fn(&Theme, button::Status) -> button::Style {
        move |_, status| button::Style {
            background: Some(Background::Color(match status {
                button::Status::Hovered => self.derived.hover,
                button::Status::Pressed => self.derived.pressed,
                _ => self.colors.panel,
            })),
            text_color: if status == button::Status::Disabled {
                self.derived.disabled
            } else {
                self.colors.text
            },
            border: Border {
                color: self.colors.border,
                width: 1.0,
                radius: 5.0.into(),
            },
            ..button::Style::default()
        }
    }

    pub fn quiet(self) -> impl Fn(&Theme, button::Status) -> button::Style {
        move |_, status| button::Style {
            background: Some(
                match status {
                    button::Status::Hovered => self.derived.hover,
                    button::Status::Pressed => self.derived.pressed,
                    _ => Color::TRANSPARENT,
                }
                .into(),
            ),
            text_color: match status {
                button::Status::Disabled => self.derived.disabled,
                button::Status::Hovered | button::Status::Pressed => self.colors.text,
                _ => self.colors.text_muted,
            },
            border: Border {
                radius: 5.0.into(),
                ..Border::default()
            },
            ..button::Style::default()
        }
    }

    pub fn choice(self, active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
        move |_, status| button::Style {
            background: Some(
                if active {
                    self.derived.selection_fill
                } else if status == button::Status::Hovered {
                    self.derived.hover
                } else {
                    self.colors.panel
                }
                .into(),
            ),
            text_color: if status == button::Status::Disabled {
                self.derived.disabled
            } else if active {
                self.colors.accent
            } else {
                self.colors.text_muted
            },
            border: Border {
                color: if active {
                    self.colors.accent
                } else {
                    self.colors.border
                },
                width: 1.0,
                radius: 5.0.into(),
            },
            ..button::Style::default()
        }
    }

    pub fn scrollbar(self) -> impl Fn(&Theme, scrollable::Status) -> scrollable::Style {
        move |theme, status| {
            let mut appearance = scrollable::default(theme, status);
            let color = if matches!(status, scrollable::Status::Dragged { .. }) {
                self.colors.text_muted
            } else {
                theme::blend(self.colors.text_muted, self.colors.panel, 0.60)
            };
            for rail in [
                &mut appearance.vertical_rail,
                &mut appearance.horizontal_rail,
            ] {
                rail.background = None;
                rail.border = Border::default();
                rail.scroller.background = color.into();
                rail.scroller.border = Border {
                    radius: 3.0.into(),
                    ..Border::default()
                };
            }
            appearance
        }
    }

    pub fn input(self) -> impl Fn(&Theme, text_input::Status) -> text_input::Style {
        move |_, status| text_input::Style {
            background: self.colors.background.into(),
            border: Border {
                color: if matches!(status, text_input::Status::Focused { .. }) {
                    self.colors.accent
                } else {
                    theme::blend(self.colors.text_muted, self.colors.panel, 0.75)
                },
                width: 1.0,
                radius: 5.0.into(),
            },
            icon: self.colors.text_muted,
            placeholder: self.colors.text_muted,
            value: if status == text_input::Status::Disabled {
                self.derived.disabled
            } else {
                self.colors.text
            },
            selection: self.derived.text_selection,
        }
    }
}
