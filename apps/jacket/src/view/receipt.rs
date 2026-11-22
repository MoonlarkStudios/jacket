use super::{components, focus_button};
use crate::app::{App, Message, Receipt};
use crate::icons::{self, Icon};
use crate::style;
use audio_engine::OutputKind;
use iced::widget::text::Wrapping;
use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Alignment, Element, Length};
use std::path::Path;

pub fn view<'a>(app: &'a App, receipt: &'a Receipt, compact: bool) -> Element<'a, Message> {
    let p = app.presentation();
    let output = &receipt.output;
    let path = Path::new(&output.output_path);
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(&output.output_path);
    let (mut status, destination_icon, reveal_label) = match output.output_kind {
        OutputKind::File => ("Audio file saved", Icon::Music, "Show in folder"),
        OutputKind::Folder => ("Music folder saved", Icon::Folder, "Open folder"),
        OutputKind::Zip => ("ZIP archive saved", Icon::Archive, "Show in folder"),
    };
    if let Some(album) = &output.album_save {
        status = if album.added == 0 {
            "Album already in your library"
        } else {
            "Album folder updated"
        };
    }
    let title = receipt
        .recording
        .as_ref()
        .map_or(name, |recording| recording.title.as_str());
    let mut heading = column![
        row![
            icons::view(Icon::Check, p.colors.success, 17.0),
            text(status).size(13).color(p.colors.success)
        ]
        .spacing(7)
        .align_y(Alignment::Center),
        text(title)
            .size(if compact { 25 } else { 31 })
            .font(style::semibold())
            .wrapping(Wrapping::WordOrGlyph),
    ]
    .spacing(7)
    .width(Length::Fill);
    if let Some(artist) = receipt
        .recording
        .as_ref()
        .and_then(|recording| recording.artist.as_deref())
    {
        heading = heading.push(
            text(artist)
                .size(14)
                .color(p.colors.text_muted)
                .wrapping(Wrapping::WordOrGlyph),
        );
    }
    if output.output_kind != OutputKind::File {
        let count = output.track_count;
        let label = output.album_save.as_ref().map_or_else(
            || format!("{count} {}", if count == 1 { "track" } else { "tracks" }),
            |album| format!("{} added · {} already there", album.added, album.kept),
        );
        heading = heading.push(text(label).size(13).color(p.colors.text_muted));
    }
    let mut hero = row![]
        .spacing(if compact { 16 } else { 22 })
        .align_y(Alignment::Center);
    if let Some(cover) = receipt
        .recording
        .as_ref()
        .and_then(|recording| recording.cover.as_ref())
    {
        hero = hero.push(components::raster_cover(
            cover,
            if compact { 96.0 } else { 128.0 },
            p,
        ));
    }
    let hero = hero.push(heading);

    let mut location = column![text(name)
        .size(14)
        .font(style::semibold())
        .wrapping(Wrapping::WordOrGlyph)]
    .spacing(5)
    .width(Length::Fill);
    if let Some(parent) = path.parent().filter(|path| !path.as_os_str().is_empty()) {
        location = location.push(
            text(parent.display().to_string())
                .size(12)
                .color(p.colors.text_muted)
                .wrapping(Wrapping::WordOrGlyph),
        );
    }
    let location = container(
        row![
            icons::view(destination_icon, p.colors.text_muted, 19.0),
            location
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .padding([12, 14])
    .style(move |_| p.panel())
    .width(Length::Fill);

    let has_files = !app.example && app.desktop.is_some();
    let mut actions = row![
        focus_button::wrap(
            "receipt-reveal",
            button(
                row![
                    icons::view(
                        Icon::Folder,
                        if has_files {
                            p.colors.accent_text
                        } else {
                            p.derived.disabled
                        },
                        15.0
                    ),
                    text(reveal_label).size(13),
                ]
                .spacing(8)
                .align_y(Alignment::Center)
            )
            .padding([10, 13])
            .style(p.primary()),
            has_files.then_some(Message::OpenOutput),
            p.colors.accent
        ),
        focus_button::wrap(
            "receipt-dismiss",
            button(
                text(if app.review.is_some() {
                    "Back to review"
                } else {
                    "Save another"
                })
                .size(13)
            )
            .padding([10, 13])
            .style(p.secondary()),
            (!app.busy()).then_some(Message::DismissReceipt),
            p.colors.accent
        ),
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    if output.playlist_path.is_some() {
        actions = actions.push(focus_button::wrap(
            "receipt-playlist",
            button(text("Open playlist").size(12))
                .padding([7, 10])
                .style(p.quiet()),
            has_files.then_some(Message::OpenPlaylist),
            p.colors.accent,
        ));
    }

    let mut content = column![hero, location, actions].spacing(16).max_width(600);
    if !output.warnings.is_empty() {
        let mut notes = column![text("Export notes")
            .font(style::semibold())
            .size(12)
            .color(p.colors.warning)]
        .spacing(7);
        for warning in &output.warnings {
            notes = notes.push(
                text(warning)
                    .size(12)
                    .color(p.colors.text_muted)
                    .wrapping(Wrapping::WordOrGlyph),
            );
        }
        content = content.push(notes);
    }
    container(
        scrollable(
            container(content)
                .center_x(Length::Fill)
                .padding(if compact { 18 } else { 32 }),
        )
        .style(p.scrollbar())
        .direction(style::scroll_direction())
        .width(Length::Fill),
    )
    .center(Length::Fill)
    .into()
}
