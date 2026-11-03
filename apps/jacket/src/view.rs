use crate::app::{App, Message, Panel};
use crate::editor::Kind;
use crate::settings::{ArtworkChoice, BrowserChoice, FormatChoice, ModeChoice, OutputChoice};
use crate::style;
use audio_engine::{CollectionMode, OutputKind};
use iced::widget::{
    button, checkbox, column, container, image, opaque, pick_list, progress_bar, radio, responsive,
    row, rule, scrollable, space, stack, svg, text, text_input,
};
use iced::{Alignment, Element, Length};
use jacket_core::SourceIntent;

pub fn view(app: &App) -> Element<'_, Message> {
    responsive(move |size| {
        let compact = size.width < 860.0;
        let base = workspace(app, compact);
        if app.panel == Panel::Workspace {
            return base;
        }
        let body = panel(app);
        let mut modal_body = column![
            row![
                text(match app.panel {
                    Panel::Settings => "Settings",
                    Panel::Preview => "Preview files",
                    Panel::Crop => "Crop artwork",
                    _ => "Jacket",
                })
                .size(13)
                .color(style::muted(app.appearance.dark)),
                space::horizontal(),
                button("Close")
                    .on_press(Message::ClosePanel)
                    .style(button::text)
            ]
            .align_y(Alignment::Center),
            rule::horizontal(1),
            scrollable(container(body).width(Length::Fill).padding(iced::Padding {
                right: 14.0,
                ..iced::Padding::ZERO
            }))
            .width(Length::Fill)
            .height(Length::Fill),
        ]
        .spacing(12);
        if let Some(actions) = panel_actions(app) {
            modal_body = modal_body.push(rule::horizontal(1)).push(actions);
        }
        let modal = container(modal_body)
            .padding(if compact { 18 } else { 26 })
            .width(Length::Fill)
            .max_width(720)
            .height(Length::Fill)
            .max_height(size.height - 40.0)
            .style(style::card);
        stack![
            container(space()).width(Length::Fill).height(Length::Fill),
            opaque(
                container(modal)
                    .center(Length::Fill)
                    .padding(20)
                    .style(|_| container::Style {
                        background: Some(iced::Color::from_rgba(0.08, 0.07, 0.06, 0.56).into()),
                        ..container::Style::default()
                    })
            )
        ]
        .into()
    })
    .into()
}

fn panel_actions(app: &App) -> Option<Element<'_, Message>> {
    match app.panel {
        Panel::Preview => Some(
            row![
                button("Back to review")
                    .on_press(Message::ClosePanel)
                    .style(button::secondary),
                space::horizontal(),
                button("Choose destination & save")
                    .padding(12)
                    .on_press_maybe(app.can_export().then_some(Message::Save))
                    .style(style::primary)
            ]
            .spacing(10)
            .into(),
        ),
        Panel::Editor => Some(
            row![
                button("Cancel")
                    .on_press(Message::ClosePanel)
                    .style(button::secondary),
                space::horizontal(),
                button("Apply changes")
                    .on_press_maybe(
                        (!app.busy()).then_some(Message::Editor(crate::editor::Action::Apply))
                    )
                    .style(style::primary)
            ]
            .spacing(10)
            .into(),
        ),
        Panel::Crop => Some(
            row![
                button("Cancel")
                    .on_press(Message::ClosePanel)
                    .style(button::secondary),
                space::horizontal(),
                button("Apply crop")
                    .on_press_maybe((!app.busy() && !app.example).then_some(Message::ApplyCrop))
                    .style(style::primary)
            ]
            .spacing(10)
            .into(),
        ),
        _ => None,
    }
}

fn workspace(app: &App, compact: bool) -> Element<'_, Message> {
    let brand = svg(svg::Handle::from_memory(include_bytes!(
        "../../../branding/jacket/svg/jacket-mark.svg"
    )))
    .width(38)
    .height(38);
    let header = row![
        brand,
        column![
            text("Jacket").size(26),
            text("From link to neatly packed.")
                .size(12)
                .color(style::muted(app.appearance.dark))
        ]
        .spacing(1),
        space::horizontal(),
        button("Settings")
            .on_press(Message::OpenPanel(Panel::Settings))
            .style(button::secondary)
    ]
    .spacing(12)
    .align_y(Alignment::Center);
    let source = text_input("Paste a YouTube or YouTube Music link…", &app.url)
        .id("source-url")
        .on_input(Message::Url)
        .on_paste(Message::Pasted)
        .on_submit(Message::Find)
        .padding(12)
        .size(15);
    let source_row = row![
        source,
        button(if app.loading {
            "Starting…"
        } else {
            "Find tracks"
        })
        .padding([12, 16])
        .on_press_maybe((!app.busy() && !app.url.trim().is_empty()).then_some(Message::Find))
        .style(style::primary)
    ]
    .spacing(10);
    let mut source_block = column![source_row].spacing(8);
    if let Ok(link) = jacket_core::SourceLink::parse(&app.url) {
        if link.song_url.is_some() && link.playlist_url.is_some() {
            source_block = source_block.push(
                row![
                    radio(
                        "This song",
                        SourceIntent::Song,
                        Some(app.intent),
                        Message::Intent
                    ),
                    radio(
                        "Entire playlist",
                        SourceIntent::Playlist,
                        Some(app.intent),
                        Message::Intent
                    )
                ]
                .spacing(20),
            );
        }
    }
    let mut top = column![header, source_block].spacing(22);
    if app.desktop.is_none() && !app.example && !app.loading {
        top = top.push(
            button("Retry startup")
                .on_press_maybe((!app.busy()).then_some(Message::RetryStartup))
                .style(button::secondary),
        );
    }
    if let Some(error) = &app.error {
        top = top.push(banner(app, error, true));
    } else if let Some(notice) = &app.notice {
        top = top.push(banner(app, notice, false));
    }
    if let Some(recovery) = &app.recovery {
        top = top.push(
            container(
                column![
                    text("An unfinished download is ready to resume").size(15),
                    text(format!(
                        "{} · {} of {} prepared",
                        recovery.plan.title, recovery.progress.completed, recovery.progress.total
                    ))
                    .size(13),
                    row![
                        button("Resume download")
                            .on_press_maybe((!app.busy()).then_some(Message::Resume))
                            .style(style::primary),
                        button("Dismiss")
                            .on_press_maybe((!app.busy()).then_some(Message::DismissRecovery))
                            .style(button::text)
                    ]
                    .spacing(10)
                ]
                .spacing(8),
            )
            .padding(14)
            .width(Length::Fill)
            .style(style::toolbar),
        );
    }
    let content: Element<'_, Message> = if let Some(review) = &app.review {
        let tracks = crate::track_list::view(app, compact);
        let heading = column![
            text(if review.preferences.mode == CollectionMode::Album {
                "ALBUM REVIEW"
            } else if review.inspection.tracks.len() == 1 {
                "SONG REVIEW"
            } else {
                "PLAYLIST REVIEW"
            })
            .size(11)
            .color(style::muted(app.appearance.dark)),
            text(&review.preferences.title).size(if compact { 24 } else { 32 }),
            text(format!(
                "{} tracks · {} selected{}",
                review.inspection.tracks.len(),
                review.selected.len(),
                if app.example {
                    " · Fictional example"
                } else {
                    ""
                }
            ))
            .size(13)
            .color(style::muted(app.appearance.dark))
        ]
        .spacing(6);
        let art = artwork(app, if compact { 76 } else { 110 });
        let summary = row![
            art,
            heading,
            space::horizontal(),
            button("Details")
                .on_press_maybe((!app.busy()).then_some(Message::OpenEditor(
                    if review.inspection.tracks.len() == 1 {
                        Kind::Track(0)
                    } else {
                        Kind::Album
                    }
                )))
                .style(button::secondary)
        ]
        .spacing(18)
        .align_y(Alignment::Center);
        let mut review_body = column![summary].spacing(18);
        if app.example {
            review_body = review_body.push(
                row![
                    text("Explore selection, editing and file previews.").size(13),
                    space::horizontal(),
                    button("Close example")
                        .on_press(Message::ConfirmReset)
                        .style(button::text)
                ]
                .align_y(Alignment::Center),
            );
        }
        if !review.inspection.warnings.is_empty() && !app.example {
            review_body = review_body.push(
                text(review.inspection.warnings.join("\n"))
                    .size(13)
                    .color(iced::color!(0xB15D35)),
            );
        }
        review_body = review_body.push(tracks);
        if !compact {
            row![
                container(review_body.height(Length::Fill)).width(Length::Fill),
                container(options(app, compact))
                    .width(270)
                    .padding(18)
                    .style(style::card)
            ]
            .spacing(22)
            .height(Length::Fill)
            .into()
        } else {
            column![review_body, options(app, compact)]
                .spacing(20)
                .into()
        }
    } else if let Some(result) = &app.result {
        let mut body = column![
            text("Neatly packed.").size(36),
            text(format!(
                "{} track{} saved",
                result.track_count,
                if result.track_count == 1 { "" } else { "s" }
            ))
            .size(17),
            text(&result.output_path).size(13),
            row![
                button(if result.output_kind == OutputKind::File {
                    "Show audio file"
                } else {
                    "Open folder"
                })
                .on_press(Message::OpenOutput)
                .style(style::primary),
                button("Prepare another")
                    .on_press(Message::ConfirmReset)
                    .style(button::secondary)
            ]
            .spacing(10)
        ]
        .spacing(20);
        if result.playlist_path.is_some() {
            body = body.push(
                button("Open playlist")
                    .on_press(Message::OpenPlaylist)
                    .style(button::text),
            );
        }
        for warning in &result.warnings {
            body = body.push(text(warning).size(13));
        }
        container(body).center(Length::Fill).padding(30).into()
    } else {
        container(column![svg(svg::Handle::from_memory(include_bytes!("../../../branding/jacket/svg/jacket-mark.svg"))).width(140).height(140),text("A place for your next listen.").size(if compact{25}else{34}),text("Start with a song, album or playlist.\nReview the details. Take your music with you.").size(16).align_x(Alignment::Center).color(style::muted(app.appearance.dark)),button("Explore an example album").on_press_maybe((!app.busy()).then_some(Message::Example)).style(button::secondary),text("Local downloads · Tagged audio · Ready for Resonant").size(12).color(style::muted(app.appearance.dark))].spacing(20).align_x(Alignment::Center)).center(Length::Fill).into()
    };
    let mut body = column![top].spacing(24);
    body = body.push(if compact {
        scrollable(
            container(content)
                .width(Length::Fill)
                .padding(iced::Padding {
                    right: 14.0,
                    ..iced::Padding::ZERO
                }),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    } else {
        content
    });
    if let Some(progress) = &app.progress {
        body = body.push(
            container(
                column![
                    row![
                        text(&progress.message).size(13),
                        space::horizontal(),
                        button("Cancel")
                            .on_press(Message::Cancel)
                            .style(button::secondary)
                    ]
                    .spacing(12)
                    .align_y(Alignment::Center),
                    progress_bar(
                        0.0..=progress.total.max(1) as f32,
                        progress.completed as f32
                    )
                    .girth(5)
                ]
                .spacing(8),
            )
            .padding(12)
            .style(style::toolbar),
        );
    }
    let footer = if app.busy() {
        "Working locally…"
    } else if app.example {
        "Example · downloads disabled"
    } else if app.save_failed {
        "Review could not be saved"
    } else if app.save_pending > 0 {
        "Saving review…"
    } else if app.review.is_some() {
        "Review saved on this computer"
    } else {
        "Ready when you are"
    };
    body = body.push(row![
        text(footer)
            .size(11)
            .color(style::muted(app.appearance.dark)),
        space::horizontal(),
        text(format!(
            "Jacket {} · Moonlark Studios",
            env!("CARGO_PKG_VERSION")
        ))
        .size(11)
        .color(style::muted(app.appearance.dark))
    ]);
    container(body)
        .padding(if compact { 18 } else { 28 })
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn options(app: &App, compact: bool) -> Element<'_, Message> {
    let Some(review) = &app.review else {
        return space().into();
    };
    let mut body = column![
        text("DOWNLOAD OPTIONS")
            .size(11)
            .color(style::muted(app.appearance.dark)),
        text("Prepare your files").size(20),
        text("Audio format").size(12),
        pick_list(
            FormatChoice::ALL,
            Some(FormatChoice::from(review.preferences.format)),
            Message::Format
        )
        .width(Length::Fill)
        .padding(9),
        text("Save as").size(12)
    ]
    .spacing(10);
    let outputs: Vec<_> = OutputChoice::ALL
        .iter()
        .copied()
        .filter(|output| *output != OutputChoice::File || review.selected.len() == 1)
        .collect();
    body = body
        .push(
            pick_list(
                outputs,
                Some(OutputChoice::from(review.preferences.output_kind)),
                Message::Output,
            )
            .width(Length::Fill)
            .padding(9),
        )
        .push(text("Artwork").size(12))
        .push(
            pick_list(
                ArtworkChoice::ALL,
                Some(ArtworkChoice::from(review.preferences.artwork_mode)),
                Message::Artwork,
            )
            .width(Length::Fill)
            .padding(9),
        );
    body = body.push(
        row![
            button("Choose cover")
                .on_press_maybe((!app.busy() && !app.example).then_some(Message::ImportCover))
                .style(button::text),
            button("Crop")
                .on_press_maybe((!app.busy() && app.cover.is_some()).then_some(Message::CropCover))
                .style(button::text)
        ]
        .spacing(5),
    );
    if review.inspection.tracks.len() > 1 {
        body = body.push(text("Metadata").size(12)).push(
            pick_list(
                ModeChoice::ALL,
                Some(ModeChoice::from(review.preferences.mode)),
                Message::Mode,
            )
            .width(Length::Fill)
            .padding(9),
        );
        body = body.push(
            button("Number tracks in source order")
                .on_press_maybe((!app.busy()).then_some(Message::NumberTracks))
                .style(button::text),
        );
        if review.can_undo_numbering() {
            body = body.push(
                button("Undo numbering")
                    .on_press_maybe((!app.busy()).then_some(Message::UndoNumbering))
                    .style(button::text),
            );
        }
    }
    let form: Element<'_, Message> = if compact {
        body.into()
    } else {
        scrollable(container(body).padding(iced::Padding {
            right: 14.0,
            ..iced::Padding::ZERO
        }))
        .height(Length::Fill)
        .into()
    };
    column![
        form,
        rule::horizontal(1),
        text(if review.preferences.output_kind == OutputKind::File {
            "Save one tagged audio file for your player or Resonant."
        } else {
            "Includes tagged audio, artwork, a portable playlist and a manifest."
        })
        .size(12)
        .color(style::muted(app.appearance.dark)),
        button("Preview files")
            .width(Length::Fill)
            .padding(12)
            .on_press_maybe(
                (!app.busy() && !review.selected.is_empty()).then_some(Message::Preview)
            )
            .style(style::primary),
        button("Discard review")
            .on_press_maybe((!app.busy()).then_some(Message::Reset))
            .style(button::text)
    ]
    .spacing(12)
    .height(if compact {
        Length::Shrink
    } else {
        Length::Fill
    })
    .into()
}

fn artwork(app: &App, size: u32) -> Element<'_, Message> {
    let art: Element<'_, Message> = if let Some((_, path)) = &app.cover {
        image(path)
            .width(size as f32)
            .height(size as f32)
            .content_fit(iced::ContentFit::Cover)
            .into()
    } else if let Some(handle) = &app.source_cover {
        image(handle.clone())
            .width(size as f32)
            .height(size as f32)
            .content_fit(iced::ContentFit::Cover)
            .into()
    } else {
        svg(svg::Handle::from_memory(include_bytes!(
            "../../../branding/jacket/svg/jacket-mark.svg"
        )))
        .width(size as f32)
        .height(size as f32)
        .into()
    };
    container(art).padding(8).style(style::toolbar).into()
}

fn banner<'a>(app: &App, message: &'a str, error: bool) -> Element<'a, Message> {
    container(
        row![
            text(message).size(13).width(Length::Fill).color(if error {
                iced::color!(0xBE4940)
            } else {
                style::muted(app.appearance.dark)
            }),
            button("Dismiss")
                .on_press(Message::DismissError)
                .style(button::text)
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .padding(12)
    .width(Length::Fill)
    .style(style::toolbar)
    .into()
}

fn panel(app: &App) -> Element<'_, Message> {
    match app.panel {
        Panel::Settings=>settings(app),
        Panel::Preview=>preview(app),
        Panel::Editor=>if let (Some(editor),Some(review))=(&app.editor,&app.review){editor.view(review)}else{space().into()},
        Panel::Crop=>{
            let mut body=column![text("Make it your own.").size(26),text("Drag the square to position your crop. Adjust its size below; Jacket saves a new local copy.").size(14),crate::crop_view::view(app)].spacing(16);
            if let Some((asset,_))=&app.cover{body=body.push(text(format!("Image dimensions: {} × {}",asset.width,asset.height)).size(13)).push(iced::widget::slider(1..=asset.width.min(asset.height),app.crop_size.parse::<u32>().unwrap_or(1),|size|Message::CropSize(size.to_string())));}
            body=body.push(row![column![text("Left").size(12),text_input("0",&app.crop_x).on_input(Message::CropX).padding(9)],column![text("Top").size(12),text_input("0",&app.crop_y).on_input(Message::CropY).padding(9)],column![text("Square size").size(12),text_input("Pixels",&app.crop_size).on_input(Message::CropSize).padding(9)]].spacing(12));
            if let Some(error)=&app.error{body=body.push(text(error).size(13).color(iced::color!(0xBE4940)));}
            body.into()
        }
        Panel::Replace=>confirm("Start a new review?","The current review will be replaced after the new link is successfully inspected.","Find tracks",Message::ConfirmFind),
        Panel::Discard=>confirm("Discard this review?","This clears the saved edits for this review. Exported files and cached source downloads are kept.","Discard review",Message::ConfirmReset),
        Panel::Close=>confirm("Close while working?","Jacket will cancel the current operation and keep your review and completed source downloads for recovery.","Cancel operation & close",Message::ConfirmClose),
        Panel::Workspace=>space().into(),
    }
}

fn confirm<'a>(
    title: &'static str,
    body: &'static str,
    label: &'static str,
    action: Message,
) -> Element<'a, Message> {
    column![
        text(title).size(26),
        text(body).size(15),
        row![
            button("Keep working")
                .on_press(Message::ClosePanel)
                .style(button::secondary),
            space::horizontal(),
            button(label).on_press(action).style(style::primary)
        ]
        .spacing(10)
    ]
    .spacing(24)
    .into()
}

fn preview(app: &App) -> Element<'_, Message> {
    let Some(preview) = &app.preview else {
        return text("Select tracks to preview their files.").into();
    };
    let mut body = column![
        text(&preview.root_name).size(27),
        text(format!(
            "{} track{} · {}",
            preview.tracks.len(),
            if preview.tracks.len() == 1 { "" } else { "s" },
            OutputChoice::from(preview.output_kind)
        ))
        .size(14),
        text(
            "Existing files are kept. Jacket creates a numbered copy when a name is already taken."
        )
        .size(13)
        .color(style::muted(app.appearance.dark)),
        rule::horizontal(1)
    ]
    .spacing(14);
    for track in &preview.tracks {
        body = body.push(
            column![
                text(&track.relative_path).size(15),
                text(format!(
                    "{} · {}{}",
                    track.metadata.artist,
                    track.metadata.album,
                    if track.extension_provisional {
                        " · extension confirmed during download"
                    } else {
                        ""
                    }
                ))
                .size(12)
                .color(style::muted(app.appearance.dark))
            ]
            .spacing(4),
        );
    }
    let extras = preview
        .files
        .iter()
        .filter(|file| {
            !preview
                .tracks
                .iter()
                .any(|track| &track.relative_path == *file)
        })
        .cloned()
        .collect::<Vec<_>>();
    if !extras.is_empty() {
        body = body
            .push(rule::horizontal(1))
            .push(text("Also included").size(13))
            .push(text(extras.join("\n")).size(13));
    }
    for warning in &preview.warnings {
        body = body.push(text(warning).size(13).color(iced::color!(0xB15D35)));
    }
    if app.example {
        body = body.push(text("Fictional example · downloads are disabled.").size(13));
    }
    body.width(Length::Fill).into()
}

fn settings(app: &App) -> Element<'_, Message> {
    let mut body=column![text("Make yourself at home.").size(27),text("Appearance").size(18),checkbox(app.appearance.dark).label("Use the ink theme").on_toggle(Message::Dark),text("Default audio format for new reviews").size(12),pick_list(FormatChoice::ALL,Some(FormatChoice::from(audio_engine::AudioFormat::from(app.appearance.default_format))),Message::DefaultFormat),rule::horizontal(1),text("Browser session").size(18),text(if app.auth.browser.is_some(){"Using your explicitly selected browser profile for this window."}else{"Public access is active. Most links work without signing in."}).size(13),text("Choose the exact browser profile where you can play the content. Your session stays on this computer and is never exported.").size(13).color(style::muted(app.appearance.dark)),pick_list(BrowserChoice::ALL,Some(app.browser),Message::Browser).width(Length::Fill)].spacing(14);
    if app.browser != BrowserChoice::Public {
        body = body.push(
            pick_list(
                app.profiles.as_slice(),
                app.profile.as_ref(),
                Message::Profile,
            )
            .placeholder("Choose an installed profile")
            .width(Length::Fill),
        );
        if let Some(notice) = &app.profile_notice {
            body = body.push(text(notice).size(13));
        }
        body = body.push(
            row![
                button("Use this session")
                    .on_press_maybe(
                        (!app.busy() && app.profile.is_some()).then_some(Message::UseSession)
                    )
                    .style(style::primary),
                button("Test access")
                    .on_press_maybe(
                        (!app.busy() && app.auth.browser.is_some()).then_some(Message::TestAccess)
                    )
                    .style(button::secondary),
                button("Use public access")
                    .on_press_maybe((!app.busy()).then_some(Message::PublicAccess))
                    .style(button::text)
            ]
            .spacing(10),
        );
    }
    body=body.push(rule::horizontal(1)).push(text("Audio downloader").size(18)).push(text("yt-dlp resolves source links. Updates are verified and activated atomically; FFmpeg continues to handle local audio conversion.").size(13).color(style::muted(app.appearance.dark)));
    if let Some(status) = &app.downloader {
        body = body.push(
            text(format!(
                "Installed: {}{}",
                status.installed_version,
                if status.using_updated_copy {
                    " · updated copy"
                } else {
                    ""
                }
            ))
            .size(14),
        );
        if let Some(latest) = &status.latest_version {
            body = body.push(text(format!("Available: {latest}")).size(13));
        }
        if let Some(error) = &status.error {
            body = body.push(text(error).size(13).color(iced::color!(0xBE4940)));
        }
    }
    body = body
        .push(
            checkbox(app.downloader_preferences.auto_update)
                .label("Automatically update the downloader")
                .on_toggle_maybe((!app.busy()).then_some(Message::AutoUpdate)),
        )
        .push(
            row![
                button("Check now")
                    .on_press_maybe(
                        (!app.busy() && !app.example).then_some(Message::CheckDownloader)
                    )
                    .style(button::secondary),
                button("Install update")
                    .on_press_maybe(
                        (!app.busy()
                            && app.downloader.as_ref().is_some_and(|s| s.update_available))
                        .then_some(Message::InstallDownloader)
                    )
                    .style(style::primary),
                button("Use bundled copy")
                    .on_press_maybe(
                        (!app.busy() && !app.example).then_some(Message::RestoreDownloader)
                    )
                    .style(button::text)
            ]
            .spacing(10),
        )
        .push(
            button("Choose local yt-dlp…")
                .on_press_maybe((!app.busy() && !app.example).then_some(Message::PickDownloader))
                .style(button::text),
        );
    if let Some(error) = &app.error {
        body = body.push(banner(app, error, true));
    } else if let Some(notice) = &app.notice {
        body = body.push(text(notice).size(13));
    }
    body.push(rule::horizontal(1)).push(text(format!("Jacket {} · A Moonlark Studios product",env!("CARGO_PKG_VERSION"))).size(13)).push(text("Designed alongside Resonant. Your music, prepared locally.\nGPL-3.0-or-later · No application server or telemetry.").size(12).color(style::muted(app.appearance.dark))).into()
}
