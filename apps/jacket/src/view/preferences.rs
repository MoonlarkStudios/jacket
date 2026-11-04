use crate::app::{App, Message, Panel};
use crate::settings::{BrowserChoice, FormatChoice};
use crate::style;
use crate::theme::{Colors, Preset, Selection};
use iced::widget::{button, checkbox, column, container, pick_list, row, rule, space, text};
use iced::{Alignment, Border, Element, Length};
use jacket_core::SourceLink;
use jacket_desktop::downloader_updates::Mode;

pub fn appearance(app: &App) -> Element<'_, Message> {
    let presentation = app.presentation();
    let mut body = column![row(Preset::ALL.map(|preset| theme_card(app, preset)))
        .spacing(12)
        .width(Length::Fill)]
    .spacing(20)
    .width(Length::Fill);

    body = body.push(
        text("Keyboard: 1 Dark · 2 Light · 3 Mocha")
            .size(11)
            .color(presentation.colors.text_muted),
    );
    if let Selection::Custom { name, .. } = &app.appearance.theme {
        body = body.push(
            container(
                column![
                    text("CUSTOM THEME")
                        .size(10)
                        .color(presentation.colors.text_muted),
                    text(name).size(14).font(style::semibold()),
                ]
                .spacing(6)
                .width(Length::Fill),
            )
            .padding(14)
            .width(Length::Fill)
            .style(move |_| presentation.panel()),
        );
    }

    body.push(rule::horizontal(1).style(presentation.line()))
        .push(
            button("Load theme file…")
                .padding([10, 14])
                .on_press_maybe((!app.dialog_open).then_some(Message::ImportTheme))
                .style(presentation.secondary()),
        )
        .push(
            text("Theme files use the same TOML format as Resonant v2.")
                .size(12)
                .color(presentation.colors.text_muted),
        )
        .into()
}

fn theme_card(app: &App, preset: Preset) -> Element<'_, Message> {
    let presentation = app.presentation();
    let selected =
        matches!(&app.appearance.theme, Selection::Preset { preset: active } if *active == preset);
    let sample = Colors::preset(preset);
    let preview = column![
        container(space())
            .width(Length::Fill)
            .height(13)
            .style(move |_| presentation.surface(sample.chrome)),
        container(
            row![
                container(space())
                    .width(25)
                    .height(30)
                    .style(move |_| presentation.surface(sample.accent)),
                column![
                    container(space())
                        .width(Length::Fill)
                        .height(5)
                        .style(move |_| presentation.surface(sample.text_muted)),
                    container(space())
                        .width(Length::Fill)
                        .height(5)
                        .style(move |_| presentation.surface(sample.raised)),
                    container(space())
                        .width(Length::Fill)
                        .height(5)
                        .style(move |_| presentation.surface(sample.raised)),
                ]
                .spacing(6)
                .width(Length::Fill),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .padding(10)
        .width(Length::Fill)
        .height(57)
        .style(move |_| presentation.surface(sample.panel)),
    ]
    .width(Length::Fill);

    button(
        column![
            container(preview)
                .width(Length::Fill)
                .clip(true)
                .style(move |_| container::Style {
                    border: Border {
                        color: sample.border,
                        width: 1.0,
                        radius: 4.0.into()
                    },
                    ..container::Style::default()
                }),
            text(preset.to_string()).size(12).font(style::semibold()),
            text(if selected { "Selected" } else { "Apply" })
                .size(11)
                .color(if selected {
                    presentation.colors.accent
                } else {
                    presentation.colors.text_muted
                }),
        ]
        .spacing(7)
        .width(Length::Fill),
    )
    .on_press_maybe((!selected && !app.dialog_open).then_some(Message::ThemePreset(preset)))
    .padding(10)
    .width(Length::Fill)
    .height(142)
    .style(move |_, status| button::Style {
        background: Some(
            if matches!(status, button::Status::Hovered | button::Status::Pressed) {
                presentation.derived.hover
            } else {
                presentation.colors.panel
            }
            .into(),
        ),
        text_color: presentation.colors.text,
        border: Border {
            color: if selected {
                presentation.colors.accent
            } else {
                presentation.colors.border
            },
            width: if selected { 2.0 } else { 1.0 },
            radius: 7.0.into(),
        },
        ..button::Style::default()
    })
    .into()
}

pub fn settings(app: &App) -> Element<'_, Message> {
    let presentation = app.presentation();
    let enabled = !app.busy() && !app.example;
    let mode = app.downloader_preferences.mode;
    let browsers = BrowserChoice::ALL
        .iter()
        .copied()
        .filter(|browser| cfg!(target_os = "macos") || *browser != BrowserChoice::Safari)
        .collect::<Vec<_>>();
    let mut body = column![
        section_title("Download defaults"),
        text("Audio format")
            .size(12)
            .color(presentation.colors.text_muted),
        pick_list(
            FormatChoice::ALL,
            Some(FormatChoice::from(audio_engine::AudioFormat::from(
                app.appearance.default_format
            ))),
            Message::DefaultFormat,
        )
        .text_size(13)
        .padding(10)
        .width(Length::Fill),
        text("Used for new reviews.")
            .size(12)
            .color(presentation.colors.text_muted),
        row![
            text(app.appearance.theme.label())
                .size(13)
                .width(Length::Fill),
            button("Change theme")
                .on_press(Message::OpenPanel(Panel::Appearance))
                .style(presentation.quiet()),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
        rule::horizontal(1).style(presentation.line()),
        section_title("Browser access"),
        text(if app.auth.browser.is_some() {
            "Browser session connected"
        } else {
            "Public access"
        })
        .size(13),
        text("Connect a signed-in profile for links that require your account.")
            .size(12)
            .color(presentation.colors.text_muted),
        pick_list(browsers, Some(app.browser), Message::Browser)
            .text_size(13)
            .padding(10)
            .width(Length::Fill),
    ]
    .spacing(12)
    .width(Length::Fill);

    if app.browser != BrowserChoice::Public {
        body = body.push(
            pick_list(
                app.profiles.as_slice(),
                app.profile.as_ref(),
                Message::Profile,
            )
            .placeholder("Choose a profile")
            .text_size(13)
            .padding(10)
            .width(Length::Fill),
        );
        if let Some(notice) = &app.profile_notice {
            body = body.push(text(notice).size(12).color(presentation.colors.text_muted));
        }
        let mut actions = row![
            button("Use profile")
                .padding([9, 13])
                .on_press_maybe((enabled && app.profile.is_some()).then_some(Message::UseSession))
                .style(presentation.primary()),
            button("Refresh profiles")
                .padding([9, 13])
                .on_press_maybe(enabled.then_some(Message::Browser(app.browser)))
                .style(presentation.secondary()),
        ]
        .spacing(8);
        if app.auth.browser.is_some() && SourceLink::parse(&app.url).is_ok() {
            actions = actions.push(
                button("Test link")
                    .padding([9, 13])
                    .on_press_maybe(enabled.then_some(Message::TestAccess))
                    .style(presentation.secondary()),
            );
        }
        body = body.push(actions);
    }
    if app.auth.browser.is_some() {
        body = body.push(
            button("Use public access")
                .on_press_maybe(enabled.then_some(Message::PublicAccess))
                .style(presentation.quiet()),
        );
    }

    body = body
        .push(rule::horizontal(1).style(presentation.line()))
        .push(section_title("Downloader"))
        .push(
            row![
                text("yt-dlp").size(13).font(style::semibold()),
                space::horizontal(),
                text(if mode == Mode::Managed {
                    "Default downloader"
                } else {
                    "Local executable"
                })
                .size(12)
                .color(presentation.colors.text_muted),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        );
    if let Some(status) = &app.downloader {
        if !status.installed_version.trim().is_empty() {
            body = body.push(
                text(format!("Version {}", status.installed_version))
                    .size(12)
                    .color(presentation.colors.text_muted),
            );
        }
        if status.pending_activation {
            body = body.push(
                text("Update ready. It will be used after the current operation finishes.")
                    .size(12)
                    .color(presentation.colors.warning),
            );
        }
        if status.downgrade && status.update_available {
            body = body.push(
                text("The selected release is older than the installed version.")
                    .size(12)
                    .color(presentation.colors.warning),
            );
        }
        if let Some(error) = &status.error {
            body = body.push(text(error).size(12).color(presentation.colors.danger));
        }
    }
    if mode == Mode::Managed {
        body = body.push(
            checkbox(app.downloader_preferences.auto_update)
                .label("Install downloader updates automatically")
                .text_size(13)
                .on_toggle_maybe(enabled.then_some(Message::AutoUpdate)),
        );
        let mut actions = row![button("Check for updates")
            .padding([9, 13])
            .on_press_maybe(enabled.then_some(Message::CheckDownloader))
            .style(presentation.secondary()),]
        .spacing(8);
        if let Some(status) = app
            .downloader
            .as_ref()
            .filter(|status| status.update_available)
        {
            let label = status.latest_version.as_ref().map_or_else(
                || "Install update".into(),
                |version| format!("Install {version}"),
            );
            actions = actions.push(
                button(text(label).size(13))
                    .padding([9, 13])
                    .on_press_maybe(enabled.then_some(Message::InstallDownloader))
                    .style(presentation.primary()),
            );
        }
        body = body.push(actions);
    } else {
        if let Some(path) = &app.downloader_preferences.local_path {
            body = body.push(
                text(path)
                    .size(12)
                    .width(Length::Fill)
                    .color(presentation.colors.text_muted),
            );
        }
        body = body.push(
            text("Jacket does not update a local executable.")
                .size(12)
                .color(presentation.colors.text_muted),
        );
    }

    let mut advanced = row![button("Choose executable…")
        .on_press_maybe(enabled.then_some(Message::PickDownloader))
        .style(presentation.quiet()),]
    .spacing(8);
    if mode == Mode::Local
        || app
            .downloader
            .as_ref()
            .is_some_and(|status| status.using_updated_copy)
    {
        advanced = advanced.push(
            button("Reset downloader")
                .on_press_maybe(enabled.then_some(Message::RestoreDownloader))
                .style(presentation.quiet()),
        );
    }

    body.push(advanced)
        .push(rule::horizontal(1).style(presentation.line()))
        .push(
            text(format!(
                "Jacket {} · GPL-3.0-or-later",
                env!("CARGO_PKG_VERSION")
            ))
            .size(11)
            .color(presentation.colors.text_muted),
        )
        .into()
}

fn section_title(title: &str) -> Element<'_, Message> {
    text(title).size(16).font(style::semibold()).into()
}
