use super::focus_button;
use crate::app::{App, Message, Panel};
use crate::style;
use audio_engine::{AlbumTrackState, AudioFormat, OutputKind};
use iced::widget::{button, column, container, row, scrollable, space, text, text_input};
use iced::{Alignment, Element, Length};
use jacket_core::PathRule;
use std::path::Path;

pub fn save_bar(app: &App, compact: bool) -> Element<'_, Message> {
    let Some(review) = &app.review else {
        return space().into();
    };
    let p = app.presentation();
    let album = app.is_album_folder();
    let organized = app.uses_music_folder();
    let configured = organized && app.music_folder.is_some();
    let preview = app.panel == Panel::Preview;
    let kind = review.preferences.output_kind;
    let format = review.preferences.format;
    let path = if album {
        app.music_album_path()
            .map(|path| path.display().to_string())
    } else {
        app.music_file_path().map(|stem| full_path(&stem, format))
    };
    let scan = app.album_folder_preview();
    let counts = scan.and_then(|result| result.as_ref().ok()).map(|scan| {
        let kept = scan.kept();
        let renamed = scan
            .tracks
            .iter()
            .filter(|track| track.state == AlbumTrackState::AddCopy)
            .count();
        (kept, scan.tracks.len() - kept, renamed)
    });
    let choice = |id, label: String, message, enabled: bool, primary| {
        focus_button::wrap(
            id,
            button(text(label).size(13))
                .padding([8, 11])
                .style(move |theme, status| {
                    if primary {
                        p.primary()(theme, status)
                    } else {
                        p.secondary()(theme, status)
                    }
                }),
            enabled.then_some(message),
            p.colors.accent,
        )
    };
    let dest_label = if let Some(path) = path {
        path
    } else if organized {
        if configured {
            "Music folder"
        } else {
            "Choose where to save"
        }
        .to_owned()
    } else {
        format!(
            "{} · {}",
            track_count(review.selected.len()),
            match kind {
                OutputKind::Zip => "ZIP archive",
                _ => "Folder",
            }
        )
    };
    let mut destination = column![readable_path(dest_label, 34, app)]
        .spacing(4)
        .width(Length::Fill);
    if album && configured {
        if let Some((kept, add, renamed)) = counts {
            let mut summary = format!("{kept} already there · {add} to add");
            if renamed > 0 {
                summary.push_str(&format!(" · {renamed} with new filenames"));
            }
            destination = destination.push(text(summary).size(11).color(p.colors.text_muted));
        } else if scan.is_none() && !app.example && !review.selected.is_empty() {
            destination = destination.push(
                text("Checking album folder…")
                    .size(11)
                    .color(p.colors.text_muted),
            );
        }
    } else if configured && kind == OutputKind::File && !matches!(format, AudioFormat::Mp3) {
        let codec = match format {
            AudioFormat::Compatible => "M4A or MP3 · Extension confirmed after download",
            _ => "Extension follows the source codec",
        };
        destination = destination.push(text(codec).size(11).color(p.colors.text_muted));
    }
    let mut location = row![destination].spacing(8).align_y(Alignment::Center);
    if configured {
        location = location.push(focus_button::wrap(
            "edit-music-folder",
            button(text("Change…").size(12))
                .padding([6, 8])
                .style(p.quiet()),
            (!app.busy()).then_some(Message::PickMusicFolder),
            p.colors.accent,
        ));
    }
    let label = if organized && !configured {
        "Choose folder…".to_owned()
    } else if album {
        match counts {
            Some((_, 1, _)) => "Add 1 track".into(),
            Some((_, count, _)) if count > 1 => format!("Add {count} tracks"),
            _ => "Save album".into(),
        }
    } else {
        match kind {
            OutputKind::File => "Save",
            OutputKind::Folder => "Save folder…",
            OutputKind::Zip => "Save ZIP…",
        }
        .into()
    };
    let mut actions = row![if preview {
        choice(
            "back-to-review",
            "Back to review".into(),
            Message::ClosePanel,
            !app.dialog_open,
            false,
        )
    } else {
        choice(
            "export-options",
            "Options…".into(),
            Message::OpenPanel(Panel::Options),
            !app.busy(),
            false,
        )
    }]
    .spacing(7)
    .align_y(Alignment::Center);
    if album && !preview && configured {
        actions = actions.push(choice(
            "preview-files",
            "Files…".into(),
            Message::Preview,
            app.can_preview(),
            false,
        ));
    }
    if compact || preview {
        actions = actions.push(space::horizontal());
    }
    if organized {
        actions = actions.push(choice(
            "save-as",
            "Save As…".into(),
            Message::SaveAs,
            app.can_export(),
            false,
        ));
    }
    actions = actions.push(choice(
        "save-output",
        label,
        Message::Save,
        app.can_export() && (!album || !configured || counts.is_some()),
        true,
    ));
    let mut bar = column![].spacing(7).width(Length::Fill);
    if let Some(Err(error)) = scan.filter(|_| !preview) {
        bar = bar.push(
            row![
                text(error)
                    .size(12)
                    .color(p.colors.warning)
                    .width(Length::Fill),
                choice(
                    "check-album-folder",
                    "Check again".into(),
                    Message::ScanAlbumFolder,
                    !app.busy(),
                    false
                ),
            ]
            .spacing(7)
            .align_y(Alignment::Center),
        );
    }
    if compact || preview {
        bar.push(location).push(actions)
    } else {
        bar.push(
            row![location.width(Length::Fill), actions]
                .spacing(16)
                .align_y(Alignment::Center),
        )
    }
    .into()
}

fn track_count(count: usize) -> String {
    format!("{count} {}", if count == 1 { "track" } else { "tracks" })
}

pub fn settings(app: &App) -> Element<'_, Message> {
    let p = app.presentation();
    let status = app.music_folder.as_ref().map_or_else(
        || "Choose a library folder for songs and albums.".into(),
        |folder| folder.root.display().to_string(),
    );
    column![
        row![
            text("Music folder")
                .size(15)
                .font(style::semibold())
                .width(Length::Fill),
            focus_button::wrap(
                "settings-music-folder",
                button(
                    text(if app.music_folder.is_some() {
                        "Edit music folder…"
                    } else {
                        "Set up…"
                    })
                    .size(12)
                )
                .style(p.secondary())
                .padding([7, 10]),
                (!app.busy()).then_some(Message::OpenMusicFolder),
                p.colors.accent,
            )
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        text(status)
            .size(12)
            .color(p.colors.text_muted)
            .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
    ]
    .spacing(6)
    .into()
}

pub fn editor(app: &App) -> Element<'_, Message> {
    let Some(editor) = &app.music_folder_editor else {
        return space().into();
    };
    let p = app.presentation();
    let root = editor.root.as_deref();
    let parsed = editor.rule.parse::<PathRule>();
    let mut content = column![
        row![
            text("Root folder").size(13).font(style::semibold()),
            space::horizontal(),
            focus_button::wrap(
                "music-folder-root",
                button(
                    text(if root.is_some() {
                        "Change folder…"
                    } else {
                        "Choose folder…"
                    })
                    .size(13)
                )
                .padding([8, 12])
                .style(p.secondary()),
                (!app.busy()).then_some(Message::ChooseMusicRoot),
                p.colors.accent,
            ),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
        text(root.map_or_else(
            || "Choose where Jacket should save organized audio files.".into(),
            |path| path.display().to_string()
        ))
        .size(13)
        .color(p.colors.text_muted)
        .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
    ]
    .spacing(14)
    .width(Length::Fill);
    let valid_rule = parsed.is_ok();
    match parsed {
        Ok(rule) => {
            let metadata = app.music_folder_sample();
            let relative = rule.resolve(&metadata);
            let stem = root.map_or_else(|| relative.clone(), |root| root.join(&relative));
            content = content.push(
                container(
                    column![
                        text("Path preview · MP3 example")
                            .size(12)
                            .font(style::semibold()),
                        text(full_path(&stem, AudioFormat::Mp3))
                            .size(13)
                            .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
                    ]
                    .spacing(8)
                    .width(Length::Fill),
                )
                .padding(12)
                .style(move |_| p.panel())
                .width(Length::Fill),
            );
        }
        Err(error) if editor.error.is_none() => {
            content = content.push(text(error).size(12).color(p.colors.danger));
        }
        Err(_) => (),
    }
    let customize = focus_button::wrap(
        "customize-path",
        button(
            text(if editor.customize {
                "Hide path rule"
            } else {
                "Customize path…"
            })
            .size(12),
        )
        .padding([6, 8])
        .style(p.quiet()),
        (!app.busy()).then_some(Message::TogglePathRule),
        p.colors.accent,
    );
    if editor.customize {
        content = content.push(column![
            row![text("Path rule").size(13).font(style::semibold()), space::horizontal(), customize]
                .align_y(Alignment::Center),
            text_input("{{album_artist}}/{{album}}/{{song}}", &editor.rule)
                .id("music-folder-rule")
                .on_input_maybe((!app.busy()).then_some(Message::MusicRule))
                .on_submit_maybe((!app.busy() && root.is_some() && valid_rule).then_some(Message::ApplyMusicFolder))
                .style(p.input())
                .padding([9, 11]),
            text("Fields  {{artist}}  {{album_artist}}  {{album}}  {{song}}")
                .size(12).color(p.colors.text_muted),
            text("A blank album skips its folder. A blank album artist uses the track artist. Jacket adds the file extension.")
                .size(12).color(p.colors.text_muted),
        ].spacing(8));
    } else {
        content = content.push(customize);
    }
    content.into()
}

pub fn editor_actions(app: &App) -> Element<'_, Message> {
    let p = app.presentation();
    let ready = app
        .music_folder_editor
        .as_ref()
        .is_some_and(|editor| editor.root.is_some() && editor.rule.parse::<PathRule>().is_ok());
    let mut buttons = row![focus_button::wrap(
        "cancel-music-folder",
        button(text("Cancel").size(13))
            .padding([8, 10])
            .style(p.secondary()),
        (!app.dialog_open).then_some(Message::ClosePanel),
        p.colors.accent,
    )]
    .spacing(8)
    .align_y(Alignment::Center);
    if app.music_folder.is_some() {
        buttons = buttons.push(focus_button::wrap(
            "clear-music-folder",
            button(text("Forget music folder").size(12))
                .padding([7, 8])
                .style(p.quiet()),
            (!app.busy()).then_some(Message::ClearMusicFolder),
            p.colors.accent,
        ));
    }
    let mut footer = column![].spacing(8);
    if let Some(error) = app
        .music_folder_editor
        .as_ref()
        .and_then(|editor| editor.error.as_ref())
    {
        footer = footer.push(text(error).size(12).color(p.colors.danger));
    }
    footer
        .push(
            buttons.push(space::horizontal()).push(focus_button::wrap(
                "save-music-folder",
                button(text("Save settings").size(13))
                    .padding([9, 13])
                    .style(p.primary()),
                (ready && !app.busy()).then_some(Message::ApplyMusicFolder),
                p.colors.accent,
            )),
        )
        .into()
}

fn readable_path(path: String, max_height: u32, app: &App) -> Element<'_, Message> {
    let p = app.presentation();
    container(
        scrollable(
            text(path)
                .size(12)
                .wrapping(iced::widget::text::Wrapping::WordOrGlyph)
                .width(Length::Fill),
        )
        .style(p.scrollbar())
        .direction(style::scroll_direction())
        .width(Length::Fill),
    )
    .max_height(max_height)
    .width(Length::Fill)
    .into()
}

fn full_path(stem: &Path, format: AudioFormat) -> String {
    let mut path = stem.display().to_string();
    if matches!(format, AudioFormat::Mp3) {
        path.push_str(".mp3");
    }
    path
}
