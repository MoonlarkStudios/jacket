use super::focus_button;
use crate::app::{App, Message, Panel};
use crate::style;
use audio_engine::AudioFormat;
use iced::widget::{button, column, container, row, scrollable, space, text, text_input};
use iced::{Alignment, Element, Length};
use jacket_core::PathRule;
use std::path::Path;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    Inspector,
    Compact,
    Preview,
}

pub fn save_controls(app: &App, placement: Placement) -> Element<'_, Message> {
    let p = app.presentation();
    let album = app.is_album_folder();
    let path = if album {
        app.music_album_path()
    } else {
        app.music_file_path()
    };
    let configured = app.music_folder.is_some();
    let compact = placement == Placement::Compact;
    let change = focus_button::wrap(
        "edit-music-folder",
        button(
            text(if configured {
                "Change…"
            } else {
                "Set up a music folder…"
            })
            .size(12),
        )
        .padding([6, 8])
        .style(p.quiet()),
        (!app.busy()).then_some(Message::OpenMusicFolder),
        p.colors.accent,
    );
    let mut content = column![].spacing(8).width(Length::Fill);
    if let Some(stem) = &path {
        let format = app
            .review
            .as_ref()
            .expect("music path belongs to a review")
            .preferences
            .format;
        content = content.push(
            column![
                row![
                    text(if album {
                        "Album folder"
                    } else {
                        "Save to music folder"
                    })
                    .size(12)
                    .font(style::semibold()),
                    space::horizontal(),
                    change
                ]
                .spacing(6)
                .align_y(Alignment::Center),
                readable_path(
                    if album {
                        stem.display().to_string()
                    } else {
                        full_path(stem, format)
                    },
                    if compact { 50 } else { 72 },
                    app,
                ),
            ]
            .spacing(4),
        );
        if !album && !matches!(format, AudioFormat::Mp3) {
            content = content.push(
                text(match format {
                    AudioFormat::Compatible if placement == Placement::Inspector => {
                        "Extension confirmed after download"
                    }
                    AudioFormat::Compatible => "M4A or MP3 · Extension confirmed after download",
                    _ => "Extension follows the source codec",
                })
                .size(11)
                .color(p.colors.text_muted),
            );
        }
    } else {
        content = content.push(change);
        if !album
            && app
                .review
                .as_ref()
                .is_some_and(|review| review.selected.len() != 1)
        {
            content = content.push(
                text("Select one track to save an audio file.")
                    .size(12)
                    .color(p.colors.warning),
            );
        }
    }

    let save_as = focus_button::wrap(
        "save-as",
        button(text("Save As…").size(13))
            .width(if !configured && placement == Placement::Inspector {
                Length::Fill
            } else {
                Length::Shrink
            })
            .padding([9, 12])
            .style(move |theme, status| {
                if configured {
                    p.secondary()(theme, status)
                } else {
                    p.primary()(theme, status)
                }
            }),
        app.can_export().then_some(Message::SaveAs),
        p.colors.accent,
    );
    let navigation = match placement {
        Placement::Compact => (
            "Export options",
            Message::OpenPanel(Panel::Options),
            !app.busy(),
        ),
        Placement::Inspector => ("Preview details", Message::Preview, app.can_preview()),
        Placement::Preview => ("Back to review", Message::ClosePanel, !app.dialog_open),
    };
    let navigate = focus_button::wrap(
        "file-save-navigation",
        button(text(navigation.0).size(12))
            .padding([7, 7])
            .style(p.quiet()),
        navigation.2.then_some(navigation.1),
        p.colors.accent,
    );
    let mut actions = row![].spacing(8).align_y(Alignment::Center);
    if !(album && placement == Placement::Inspector) {
        actions = actions.push(navigate);
    }
    actions = actions.push(space::horizontal());
    if !configured && placement == Placement::Inspector {
        content = content.push(save_as);
    } else {
        actions = actions.push(save_as);
    }
    if configured {
        let ready = app.can_export()
            && (!album
                || placement != Placement::Preview
                || matches!(app.album_preview, Some(Ok(_))));
        let save = focus_button::wrap(
            "save-organized-output",
            button(
                text(if album && placement != Placement::Preview {
                    "Save…"
                } else {
                    "Save"
                })
                .size(13),
            )
            .width(if placement == Placement::Inspector {
                Length::Fill
            } else {
                Length::Shrink
            })
            .padding([9, 12])
            .style(p.primary()),
            ready.then_some(Message::Save),
            p.colors.accent,
        );
        if placement == Placement::Inspector {
            content = content.push(save);
        } else {
            actions = actions.push(save);
        }
    }
    content.push(actions).into()
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
                button(text(if root.is_some() { "Change folder…" } else { "Choose folder…" }).size(13))
                    .padding([8, 12])
                    .style(p.secondary()),
                (!app.busy()).then_some(Message::ChooseMusicRoot),
                p.colors.accent,
            ),
        ].spacing(12).align_y(Alignment::Center),
        text(root.map_or_else(|| "Choose where Jacket should save organized audio files.".into(), |path| path.display().to_string()))
            .size(13).color(p.colors.text_muted).wrapping(iced::widget::text::Wrapping::WordOrGlyph),
        text("Path rule").size(13).font(style::semibold()),
        text_input("{{album_artist}}/{{album}}/{{song}}", &editor.rule)
            .id("music-folder-rule")
            .on_input_maybe((!app.busy()).then_some(Message::MusicRule))
            .on_submit_maybe((!app.busy() && root.is_some() && parsed.is_ok()).then_some(Message::ApplyMusicFolder))
            .style(p.input())
            .padding([9, 11]),
        text("Fields  {{artist}}  {{album_artist}}  {{album}}  {{song}}")
            .size(12).color(p.colors.text_muted),
        text("A blank album skips its folder. A blank album artist uses the track artist. Jacket adds the file extension.")
            .size(12).color(p.colors.text_muted),
    ].spacing(14).width(Length::Fill);
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
