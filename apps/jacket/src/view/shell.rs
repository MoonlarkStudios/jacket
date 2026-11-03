use super::{components, options, preferences, preview, workspace};
use crate::app::{App, Message, Panel};
use crate::icons::{self, Icon};
use crate::style;
use iced::widget::{
    button, column, container, progress_bar, radio, responsive, row, rule, scrollable, space, svg,
    text, text_input, tooltip,
};
use iced::{Alignment, Element, Length};
use jacket_core::SourceIntent;

pub fn view(app: &App) -> Element<'_, Message> {
    responsive(move |size| view_at(app, size)).into()
}

fn view_at(app: &App, size: iced::Size) -> Element<'_, Message> {
    let compact = size.width < 900.0;
    let p = app.presentation();
    let content: Element<'_, Message> = if app.panel == Panel::Workspace {
        let mut body = column![
            header(app, compact),
            rule::horizontal(1).style(p.line()),
            source(app, compact),
            rule::horizontal(1).style(p.line())
        ]
        .spacing(0)
        .height(Length::Fill);
        if let Some(error) = &app.error {
            body = body.push(notice(app, error, true));
        } else if let Some(note) = &app.notice {
            body = body.push(notice(app, note, false));
        }
        if let Some(recovery) = &app.recovery {
            body = body.push(
                container(
                    row![
                        column![
                            text("Resume download").font(style::semibold()).size(13),
                            text(format!(
                                "{} · {} of {} tracks prepared",
                                recovery.plan.title,
                                recovery.progress.completed,
                                recovery.progress.total
                            ))
                            .size(12)
                            .color(p.colors.text_muted)
                        ]
                        .spacing(4)
                        .width(Length::Fill),
                        button("Resume")
                            .padding([7, 12])
                            .on_press_maybe((!app.busy()).then_some(Message::Resume))
                            .style(p.primary()),
                        button("Dismiss")
                            .padding([7, 10])
                            .on_press_maybe((!app.busy()).then_some(Message::DismissRecovery))
                            .style(p.quiet()),
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center),
                )
                .padding([12, 18])
                .width(Length::Fill)
                .style(move |_| p.surface(p.colors.panel)),
            );
        }
        body = body.push(workspace::view(app, compact));
        body.push(status(app)).into()
    } else {
        let body = panel_body(app, compact);

        let title = match app.panel {
            Panel::Appearance => "Appearance",
            Panel::Settings => "Settings",
            Panel::Options => "Export options",
            Panel::Preview => "Preview export",
            Panel::Editor => "Edit tags",
            Panel::Crop => "Crop artwork",
            Panel::SourceNotes => "Source notes",
            _ => "Jacket",
        };
        let mut modal = column![
            row![
                text(title).size(14).font(style::semibold()),
                space::horizontal(),
                tooltip(
                    button(icons::view(Icon::Close, p.colors.text_muted, 16.0))
                        .padding(5)
                        .on_press(Message::ClosePanel)
                        .style(p.quiet()),
                    "Close",
                    tooltip::Position::Bottom
                )
            ]
            .align_y(Alignment::Center),
            rule::horizontal(1).style(p.line()),
            scrollable(container(body).width(Length::Fill).padding(iced::Padding {
                right: 12.0,
                ..iced::Padding::ZERO
            }))
            .width(Length::Fill)
            .style(p.scrollbar())
            .direction(crate::style::scroll_direction())
            .height(Length::Fill),
        ]
        .spacing(16);
        if let Some(error) = &app.error {
            modal = modal.push(text(error).size(12).color(p.colors.danger));
        }
        if let Some(actions) = actions(app) {
            modal = modal
                .push(rule::horizontal(1).style(p.line()))
                .push(actions);
        }
        let desired_height: f32 = match app.panel {
            Panel::Appearance => {
                if matches!(app.appearance.theme, crate::theme::Selection::Custom { .. }) {
                    570.0
                } else {
                    480.0
                }
            }
            Panel::Replace | Panel::Discard | Panel::Close => 280.0,
            Panel::Crop => 640.0,
            _ => 780.0,
        };
        let modal = container(modal)
            .padding(if compact { 18 } else { 24 })
            .width(Length::Fill)
            .max_width(if app.panel == Panel::Preview {
                780
            } else {
                680
            })
            .height(desired_height.min((size.height - 56.0).max(320.0)))
            .style(move |_| p.panel());
        container(modal)
            .center(Length::Fill)
            .padding(if compact { 16 } else { 28 })
            .style(move |_| p.surface(p.colors.chrome))
            .into()
    };
    container(content)
        .height(Length::Fill)
        .width(Length::Fill)
        .style(move |_| p.surface(p.colors.background))
        .into()
}

fn panel_body(app: &App, compact: bool) -> Element<'_, Message> {
    let p = app.presentation();
    match app.panel {
        Panel::Appearance => preferences::appearance(app),
        Panel::Settings => preferences::settings(app),
        Panel::Options => options::view(app, false),
        Panel::Preview => preview::view(app, compact),
        Panel::Editor => {
            if let (Some(editor), Some(review)) = (&app.editor, &app.review) {
                editor.view(review, p)
            } else {
                space().into()
            }
        },
        Panel::Crop => crop(app),
        Panel::SourceNotes => source_notes(app),
        Panel::Replace => confirmation(
            app,
            "Replace this review?",
            "Your current review stays available until the new link has loaded.",
            "Find tracks",
            Message::ConfirmFind,
        ),
        Panel::Discard => confirmation(
            app,
            "Discard this review?",
            "This removes the saved edits for this review. Your exported files stay in their destination.",
            "Discard review",
            Message::ConfirmReset,
        ),
        Panel::Close => confirmation(
            app,
            "Cancel the download and close?",
            "Your review and completed source downloads will be kept so you can resume later.",
            "Cancel and close",
            Message::ConfirmClose,
        ),
        Panel::Workspace => space().into(),
    }
}

fn header(app: &App, compact: bool) -> Element<'_, Message> {
    let p = app.presentation();
    container(
        row![
            svg(svg::Handle::from_memory(include_bytes!(
                "../../../../branding/jacket/svg/jacket-mark.svg"
            )))
            .width(23)
            .height(23),
            text("Jacket").font(style::semibold()).size(15),
            space::horizontal(),
            button(
                row![
                    icons::view(Icon::Palette, p.colors.text_muted, 15.0),
                    text(if compact {
                        "Theme".to_owned()
                    } else {
                        {
                            let label = app.appearance.theme.label();
                            if label.chars().count() > 32 {
                                format!("{}…", label.chars().take(31).collect::<String>())
                            } else {
                                label
                            }
                        }
                    })
                    .size(12)
                ]
                .spacing(7)
                .align_y(Alignment::Center)
            )
            .padding([6, 9])
            .on_press(Message::OpenPanel(Panel::Appearance))
            .style(p.quiet()),
            button(
                row![
                    icons::view(Icon::Settings, p.colors.text_muted, 15.0),
                    text("Settings").size(12)
                ]
                .spacing(7)
                .align_y(Alignment::Center)
            )
            .padding([6, 9])
            .on_press(Message::OpenPanel(Panel::Settings))
            .style(p.quiet()),
        ]
        .spacing(9)
        .align_y(Alignment::Center),
    )
    .padding([9, 16])
    .width(Length::Fill)
    .style(move |_| p.surface(p.colors.chrome))
    .into()
}

fn source(app: &App, _compact: bool) -> Element<'_, Message> {
    let p = app.presentation();
    let enabled = !app.busy();
    let input = text_input("YouTube or YouTube Music URL", &app.url)
        .id("source-url")
        .on_input_maybe(enabled.then_some(Message::Url))
        .on_paste_maybe(enabled.then_some(Message::Pasted))
        .on_submit_maybe(enabled.then_some(Message::Find))
        .padding([9, 12])
        .size(13)
        .style(p.input());
    let mut body = column![row![
        icons::view(Icon::Link, p.colors.text_muted, 16.0),
        input,
        button(
            row![
                text(if app.loading {
                    "Starting…"
                } else {
                    "Find tracks"
                })
                .size(13),
                icons::view(
                    Icon::ArrowRight,
                    if enabled && !app.url.trim().is_empty() {
                        p.colors.accent_text
                    } else {
                        p.derived.disabled
                    },
                    14.0
                )
            ]
            .spacing(8)
            .align_y(Alignment::Center)
        )
        .padding([10, 13])
        .on_press_maybe((enabled && !app.url.trim().is_empty()).then_some(Message::Find))
        .style(p.primary()),
    ]
    .spacing(10)
    .align_y(Alignment::Center)]
    .spacing(10);
    if let Ok(link) = jacket_core::SourceLink::parse(&app.url) {
        if link.song_url.is_some() && link.playlist_url.is_some() {
            body = body.push(
                row![
                    radio(
                        "This song",
                        SourceIntent::Song,
                        Some(app.intent),
                        Message::Intent
                    )
                    .size(14)
                    .text_size(12),
                    radio(
                        "Entire playlist",
                        SourceIntent::Playlist,
                        Some(app.intent),
                        Message::Intent
                    )
                    .size(14)
                    .text_size(12),
                ]
                .spacing(20),
            );
        }
    }
    if app.desktop.is_none() && !app.example && !app.loading {
        body = body.push(
            button("Retry startup")
                .padding([5, 8])
                .on_press_maybe(enabled.then_some(Message::RetryStartup))
                .style(p.secondary()),
        );
    }
    container(body)
        .padding([12, 18])
        .width(Length::Fill)
        .style(move |_| p.surface(p.colors.panel))
        .into()
}

fn status(app: &App) -> Element<'_, Message> {
    let p = app.presentation();
    if let Some(progress) = &app.progress {
        return container(
            column![
                row![
                    text(&progress.message).size(12).width(Length::Fill),
                    button("Cancel")
                        .padding([5, 10])
                        .on_press(Message::Cancel)
                        .style(p.secondary())
                ]
                .spacing(12)
                .align_y(Alignment::Center),
                progress_bar(
                    0.0..=progress.total.max(1) as f32,
                    progress.completed as f32
                )
                .girth(3),
            ]
            .spacing(8),
        )
        .padding([10, 16])
        .width(Length::Fill)
        .style(move |_| p.surface(p.colors.chrome))
        .into();
    }
    let left = if app.save_failed {
        "Review not saved".to_owned()
    } else if app.save_pending > 0 {
        "Saving review…".into()
    } else if app.example {
        "Example · downloads disabled".into()
    } else if app.review.is_some() {
        "Review saved".into()
    } else if app.auth.browser.is_some() {
        "Browser session".into()
    } else {
        "Public access".into()
    };
    let right = if let Some(review) = &app.review {
        let seconds: f64 = review
            .selected
            .iter()
            .filter_map(|index| review.inspection.tracks.get(*index)?.duration)
            .sum();
        format!(
            "{} selected  ·  {}",
            review.selected.len(),
            components::duration(seconds)
        )
    } else {
        String::new()
    };
    container(
        row![
            text(left).size(11).color(if app.save_failed {
                p.colors.warning
            } else {
                p.colors.text_muted
            }),
            space::horizontal(),
            text(right).size(11).color(p.colors.text_muted)
        ]
        .align_y(Alignment::Center),
    )
    .padding([7, 16])
    .width(Length::Fill)
    .style(move |_| p.surface(p.colors.chrome))
    .into()
}

fn notice<'a>(app: &App, message: &'a str, error: bool) -> Element<'a, Message> {
    let p = app.presentation();
    let color = if error {
        p.colors.danger
    } else {
        p.colors.text_muted
    };
    container(
        row![
            text(message).size(12).color(color).width(Length::Fill),
            button(icons::view(Icon::Close, p.colors.text_muted, 14.0))
                .padding(3)
                .on_press(Message::DismissError)
                .style(p.quiet())
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .padding([10, 18])
    .width(Length::Fill)
    .style(move |_| p.surface(p.colors.panel))
    .into()
}

fn actions(app: &App) -> Option<Element<'_, Message>> {
    let p = app.presentation();
    let (label, message, enabled) = match app.panel {
        Panel::Preview => ("Save audio…", Message::Save, app.can_export()),
        Panel::Editor => (
            "Apply changes",
            Message::Editor(crate::editor::Action::Apply),
            !app.busy(),
        ),
        Panel::Crop => (
            "Apply crop",
            Message::ApplyCrop,
            !app.busy() && !app.example,
        ),
        Panel::Options => ("Preview files", Message::Preview, app.can_preview()),
        Panel::Settings | Panel::Appearance | Panel::SourceNotes => {
            ("Done", Message::ClosePanel, true)
        }
        _ => return None,
    };
    let back = if matches!(app.panel, Panel::Preview | Panel::Options) {
        "Back to review"
    } else {
        "Cancel"
    };
    let back: Element<'_, Message> = if matches!(
        app.panel,
        Panel::Settings | Panel::Appearance | Panel::SourceNotes
    ) {
        space().into()
    } else {
        button(back)
            .padding([9, 12])
            .on_press(Message::ClosePanel)
            .style(p.secondary())
            .into()
    };
    Some(
        row![
            back,
            space::horizontal(),
            button(text(label).size(13))
                .padding([10, 16])
                .on_press_maybe(enabled.then_some(message))
                .style(p.primary()),
        ]
        .spacing(12)
        .align_y(Alignment::Center)
        .into(),
    )
}

fn confirmation<'a>(
    app: &App,
    title: &'a str,
    description: &'a str,
    label: &'a str,
    message: Message,
) -> Element<'a, Message> {
    let p = app.presentation();
    column![
        text(title).size(19).font(style::semibold()),
        text(description).size(13).color(p.colors.text_muted),
        row![
            button("Keep working")
                .padding([9, 12])
                .on_press(Message::ClosePanel)
                .style(p.secondary()),
            space::horizontal(),
            button(label)
                .padding([9, 12])
                .on_press(message)
                .style(p.primary())
        ]
        .spacing(12),
    ]
    .spacing(18)
    .into()
}

fn crop(app: &App) -> Element<'_, Message> {
    let p = app.presentation();
    let mut body = column![
        text("Drag to position the square. Adjust the size below.")
            .size(13)
            .color(p.colors.text_muted),
        container(crate::crop_view::view(app)).center_x(Length::Fill),
    ]
    .spacing(16)
    .width(Length::Fill);
    if let Some((asset, _)) = &app.cover {
        body = body.push(row![
            text("Crop size").size(12).color(p.colors.text_muted),
            space::horizontal(),
            text(format!("{} px", app.crop_size))
                .size(12)
                .color(p.colors.text_muted)
        ]);
        body = body.push(iced::widget::slider(
            1..=asset.width.min(asset.height),
            app.crop_size.parse::<u32>().unwrap_or(1),
            |size| Message::CropSize(size.to_string()),
        ));
    }
    body.push(
        row![
            column![
                text("Left").size(12).color(p.colors.text_muted),
                text_input("Pixels", &app.crop_x)
                    .on_input(Message::CropX)
                    .padding(8)
                    .size(13)
                    .style(p.input())
            ]
            .spacing(6),
            column![
                text("Top").size(12).color(p.colors.text_muted),
                text_input("Pixels", &app.crop_y)
                    .on_input(Message::CropY)
                    .padding(8)
                    .size(13)
                    .style(p.input())
            ]
            .spacing(6),
            column![
                text("Size").size(12).color(p.colors.text_muted),
                text_input("Pixels", &app.crop_size)
                    .on_input(Message::CropSize)
                    .padding(8)
                    .size(13)
                    .style(p.input())
            ]
            .spacing(6),
        ]
        .spacing(12),
    )
    .into()
}

fn source_notes(app: &App) -> Element<'_, Message> {
    let p = app.presentation();
    let mut body = iced::widget::Column::new().spacing(14).width(Length::Fill);
    if let Some(review) = &app.review {
        for warning in review.inspection.warnings.iter().take(200) {
            body = body.push(text(warning).size(13).color(p.colors.warning));
        }
        if review.inspection.warnings.len() > 200 {
            body = body.push(
                text(format!(
                    "Showing the first 200 of {} notes.",
                    review.inspection.warnings.len()
                ))
                .size(12)
                .color(p.colors.text_muted),
            );
        }
    }
    body.into()
}
