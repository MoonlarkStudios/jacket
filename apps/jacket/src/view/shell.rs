use super::{components, destination, downloads, options, preferences, preview, workspace};
use crate::app::{App, DownloadAction, DownloadSurface, Message, Panel, SourceKind};
use crate::icons::{self, Icon};
use crate::style;
use iced::widget::{
    button, column, container, mouse_area, opaque, progress_bar, responsive, row, rule, scrollable,
    space, stack, svg, text, text_input, tooltip,
};
use iced::{Alignment, Element, Length};
use jacket_core::{SourceIntent, SourceLink};

pub fn view(app: &App) -> Element<'_, Message> {
    responsive(move |size| view_at(app, size)).into()
}

fn view_at(app: &App, size: iced::Size) -> Element<'_, Message> {
    let compact = size.width < 900.0;
    let p = app.presentation();
    let content: Element<'_, Message> =
        if app.downloads.surface == DownloadSurface::Page && app.panel == Panel::Workspace {
            column![
                header(app, compact),
                rule::horizontal(1).style(p.line()),
                downloads::page(app, compact)
            ]
            .height(Length::Fill)
            .into()
        } else if app.panel == Panel::Workspace || app.panel.review_tool() {
            let show_catalog = app.catalog.visible
                && (app.panel == Panel::Workspace || app.reviewing_catalog_album());
            let mut body = column![header(app, compact), rule::horizontal(1).style(p.line())]
                .spacing(0)
                .height(Length::Fill);
            if !show_catalog {
                body = body
                    .push(source(app, compact))
                    .push(rule::horizontal(1).style(p.line()));
            }
            if let Some(error) = &app.error {
                body = body.push(notice(app, error, true));
            } else if let Some(note) = &app.notice {
                body = body.push(notice(app, note, false));
            }
            if let Some(recovery) = app.recovery.as_ref().filter(|_| !app.panel.review_tool()) {
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
                            super::focus_button::wrap(
                                "resume-recovery",
                                button("Resume").padding([7, 12]).style(p.primary()),
                                (!app.busy()).then_some(Message::Resume),
                                p.colors.accent,
                            ),
                            super::focus_button::wrap(
                                "dismiss-recovery",
                                button("Dismiss").padding([7, 10]).style(p.quiet()),
                                (!app.busy()).then_some(Message::DismissRecovery),
                                p.colors.accent,
                            ),
                        ]
                        .spacing(10)
                        .align_y(Alignment::Center),
                    )
                    .padding([12, 18])
                    .width(Length::Fill)
                    .style(move |_| p.surface(p.colors.panel)),
                );
            }
            let workspace: Element<'_, Message> = if show_catalog {
                super::catalog::view(app, compact)
            } else {
                column![workspace::view(app, compact), status(app)]
                    .height(Length::Fill)
                    .into()
            };
            let mut content = stack![workspace];
            if app.downloads.surface == DownloadSurface::Dropdown && app.panel == Panel::Workspace {
                content = content.push(stack![
                    mouse_area(space().width(Length::Fill).height(Length::Fill))
                        .on_press(Message::Downloads(DownloadAction::Close)),
                    container(opaque(downloads::dropdown(app, size.width - 32.0)))
                        .align_right(Length::Fill)
                        .padding(iced::Padding {
                            top: 6.0,
                            right: 16.0,
                            ..iced::Padding::ZERO
                        }),
                ]);
            } else if let Some(label) = app
                .downloads
                .notice()
                .filter(|_| app.panel == Panel::Workspace && app.review.is_none())
            {
                content = content.push(
                    container(opaque(downloads::notice(app, label)))
                        .align_right(Length::Fill)
                        .align_bottom(Length::Fill)
                        .padding(12),
                );
            }
            body.push(content).into()
        } else {
            let body = panel_body(app, compact);

            let title = match app.panel {
                Panel::Appearance => "Appearance",
                Panel::Settings => "Settings",
                Panel::MusicFolder => "Music folder",
                Panel::Options => "Export options",
                Panel::Preview => "Preview export",
                Panel::Cover => "Edit cover",
                Panel::Editor => {
                    if app.editor.as_ref().is_some_and(|editor| {
                        matches!(editor.kind, crate::editor::Kind::OutputName)
                    }) {
                        "Output name"
                    } else {
                        "Edit tags"
                    }
                }
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
            } else if let Some(note) = &app.notice {
                modal = modal.push(text(note).size(12).color(p.colors.text_muted));
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
                Panel::Cover | Panel::Options | Panel::MusicFolder => 480.0,
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
        Panel::MusicFolder => destination::editor(app),
        Panel::Options => options::view(app),
        Panel::Preview => preview::view(app, compact),
        Panel::Cover => super::cover_controls::view(app),
        Panel::Editor => {
            if let (Some(editor), Some(review)) = (&app.editor, &app.review) {
                editor.view(review, p)
            } else {
                space().into()
            }
        },
        Panel::Crop => crop(app, compact),
        Panel::SourceNotes => source_notes(app),
        Panel::Replace => confirmation(
            app,
            "Replace this review?",
            "Your current review stays available until the new link has loaded.",
            "Open link",
            Message::ConfirmFind,
        ),
        Panel::Discard => confirmation(
            app,
            "Discard this review?",
            "This removes the saved edits for this review. Your exported files stay in their destination.",
            "Discard review",
            Message::ConfirmReset,
        ),
        Panel::Close if app.catalog.pending.is_some() => confirmation(
            app,
            "Stop this lookup and close?",
            "Any saved review will be available when you reopen Jacket.",
            "Stop and close",
            Message::ConfirmClose,
        ),
        Panel::Close if app.intake.is_some() => confirmation(
            app,
            "Stop looking up this link and close?",
            if app.review.is_some() {
                "Your saved review will be available when you reopen Jacket."
            } else {
                "You can paste the link again when you reopen Jacket."
            },
            "Stop and close",
            Message::ConfirmClose,
        ),
        Panel::Close => confirmation(
            app,
            "Cancel the download and close?",
            "Your review and completed source downloads will be kept so you can resume later.",
            "Cancel and close",
            Message::ConfirmClose,
        ),
        Panel::Workspace | Panel::SongEditor | Panel::CollectionEditor | Panel::TrackOrder => space().into(),
    }
}

fn header(app: &App, compact: bool) -> Element<'_, Message> {
    let p = app.presentation();
    let bar = container(
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
            .on_press_maybe(
                (!app.panel.review_tool()).then_some(Message::OpenPanel(Panel::Appearance))
            )
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
            .on_press_maybe(
                (!app.panel.review_tool()).then_some(Message::OpenPanel(Panel::Settings))
            )
            .style(p.quiet()),
            downloads::trigger(app),
        ]
        .spacing(9)
        .align_y(Alignment::Center),
    )
    .padding([9, 16])
    .width(Length::Fill)
    .style(move |_| p.surface(p.colors.chrome));
    let area = mouse_area(bar);
    if app.downloads.surface == DownloadSurface::Dropdown {
        area.on_press(Message::Downloads(DownloadAction::Close))
            .into()
    } else {
        area.into()
    }
}

fn source(app: &App, _compact: bool) -> Element<'_, Message> {
    let p = app.presentation();
    let enabled = !app.busy() && app.intake.is_none() && !app.panel.review_tool();
    let link = SourceLink::parse(&app.url).ok();
    let label = if app.loading {
        "Starting…".into()
    } else if let Some(link) = &link {
        format!("Open {}", SourceKind::from_link(link, app.intent).label())
    } else {
        "Open link".into()
    };
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
        super::focus_button::wrap(
            "open-source",
            button(
                row![
                    text(label).size(13),
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
            .style(p.primary()),
            (enabled && !app.url.trim().is_empty()).then_some(Message::Find),
            p.colors.accent
        ),
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .push(super::focus_button::wrap(
        "browse-albums",
        button("Find music").padding([8, 10]).style(p.secondary()),
        (enabled && !app.example).then_some(Message::Catalog(crate::app::CatalogAction::Browse)),
        p.colors.accent,
    ))]
    .spacing(10);
    if link.is_some_and(|link| link.has_playlist_context()) {
        let choice = |id, label, intent| {
            super::focus_button::wrap(
                id,
                button(text(label).size(12))
                    .padding([6, 9])
                    .style(p.choice(intent == app.intent)),
                enabled.then_some(Message::Intent(intent)),
                p.colors.accent,
            )
        };
        body = body.push(
            row![
                text("Link includes a song and a playlist")
                    .size(12)
                    .color(p.colors.text_muted),
                choice("choose-song", "This song", SourceIntent::Song),
                choice("choose-playlist", "Entire playlist", SourceIntent::Playlist),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        );
    }
    if let Some(error) = &app.source_error {
        body = body.push(
            container(
                scrollable(
                    text(error)
                        .size(12)
                        .color(p.colors.danger)
                        .wrapping(iced::widget::text::Wrapping::WordOrGlyph)
                        .width(Length::Fill),
                )
                .style(p.scrollbar())
                .direction(style::scroll_direction())
                .width(Length::Fill),
            )
            .max_height(68_u32),
        );
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
    let right = if let Some(review) = app
        .review
        .as_ref()
        .filter(|_| app.result.is_none() && app.intake.is_none())
    {
        let seconds: f64 = review
            .selected
            .iter()
            .filter_map(|index| review.inspection.tracks.get(*index)?.duration)
            .sum();
        if review.is_song() {
            components::duration(seconds)
        } else {
            format!(
                "{} selected  ·  {}",
                review.selected.len(),
                components::duration(seconds)
            )
        }
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
    if app.panel == Panel::MusicFolder {
        return Some(destination::editor_actions(app));
    }
    if app.panel == Panel::Preview {
        return Some(destination::save_bar(app, true));
    }
    let p = app.presentation();
    let (label, message, enabled) = match app.panel {
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
        Panel::Cover => ("Done", Message::ClosePanel, !app.busy()),
        Panel::Settings | Panel::Appearance | Panel::SourceNotes => {
            ("Done", Message::ClosePanel, true)
        }
        _ => return None,
    };
    let back = if app.panel == Panel::Options {
        "Back to review"
    } else {
        "Cancel"
    };
    let back: Element<'_, Message> = if matches!(
        app.panel,
        Panel::Cover | Panel::Settings | Panel::Appearance | Panel::SourceNotes
    ) {
        space().into()
    } else {
        super::focus_button::wrap(
            "modal-back",
            button(back).padding([9, 12]).style(p.secondary()),
            (!app.dialog_open).then_some(Message::ClosePanel),
            p.colors.accent,
        )
    };
    Some(
        row![
            back,
            space::horizontal(),
            super::focus_button::wrap(
                "modal-apply",
                button(text(label).size(13))
                    .padding([10, 16])
                    .style(p.primary()),
                enabled.then_some(message),
                p.colors.accent
            ),
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

fn crop(app: &App, compact: bool) -> Element<'_, Message> {
    let p = app.presentation();
    let mut body = column![
        text("Drag the square or enter exact values.")
            .size(13)
            .color(p.colors.text_muted),
        container(crate::crop_view::view(app, compact)).center_x(Length::Fill),
    ]
    .spacing(10)
    .width(Length::Fill);
    if let Some((asset, _)) = app.crop_asset() {
        body = body.push(row![
            text("Crop size").size(12).color(p.colors.text_muted),
            space::horizontal(),
            text(format!("{} px", app.crop_preview_size))
                .size(12)
                .color(p.colors.text_muted)
        ]);
        body = body.push(iced::widget::slider(
            1..=asset.width.min(asset.height),
            app.crop_preview_size,
            |size| Message::CropSize(size.to_string()),
        ));
    }
    body.push(
        row![
            column![
                text("Left").size(12).color(p.colors.text_muted),
                text_input("Pixels", &app.crop_x)
                    .id("crop-left")
                    .on_input_maybe((!app.busy()).then_some(Message::CropX))
                    .padding(8)
                    .size(13)
                    .style(p.input())
            ]
            .spacing(6),
            column![
                text("Top").size(12).color(p.colors.text_muted),
                text_input("Pixels", &app.crop_y)
                    .id("crop-top")
                    .on_input_maybe((!app.busy()).then_some(Message::CropY))
                    .padding(8)
                    .size(13)
                    .style(p.input())
            ]
            .spacing(6),
            column![
                text("Size").size(12).color(p.colors.text_muted),
                text_input("Pixels", &app.crop_size)
                    .id("crop-size")
                    .on_input_maybe((!app.busy()).then_some(Message::CropSize))
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
