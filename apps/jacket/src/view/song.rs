//! Review one recording and edit its tags in place.
use super::components::{self, CoverContext};
use super::focus_button;
use crate::app::{App, Message, Panel};
use crate::editor::{self, Kind};
use crate::icons::{self, Icon};
use crate::style;
use audio_engine::{CollectionMode, Metadata};
use iced::widget::text::Wrapping;
use iced::widget::{button, column, container, row, rule, scrollable, space, text, Column};
use iced::{Alignment, Element, Length};

pub fn view(app: &App, compact: bool) -> Element<'_, Message> {
    let p = app.presentation();
    let Some(review) = &app.review else {
        return space().into();
    };
    let Some(metadata) = review.effective_metadata(0) else {
        return space().into();
    };
    let editing = app.panel == Panel::SongEditor;
    let mut heading = column![
        text("Song").size(12).color(p.colors.accent),
        text(metadata.title.clone())
            .size(if compact { 22 } else { 28 })
            .font(style::semibold())
            .wrapping(Wrapping::WordOrGlyph)
            .width(Length::Fill),
        text(if metadata.artist.trim().is_empty() {
            "Unknown artist".to_owned()
        } else {
            metadata.artist.clone()
        })
        .size(14)
        .wrapping(Wrapping::WordOrGlyph)
        .color(p.colors.text_muted),
    ]
    .spacing(8)
    .width(Length::Fill);
    if let Some(duration) = review.inspection.tracks[0]
        .duration
        .filter(|value| *value > 0.0)
    {
        heading = heading.push(
            text(components::duration(duration))
                .size(12)
                .color(p.colors.text_muted),
        );
    }
    if !editing {
        heading = heading.push(super::cover_controls::view(app, review));
    }
    let edge = match (compact, editing) {
        (true, true) => 96.0,
        (true, false) => 128.0,
        (false, true) => 128.0,
        (false, false) => 184.0,
    };
    let hero = row![components::cover(app, edge, CoverContext::Source), heading]
        .spacing(if compact { 18 } else { 24 })
        .align_y(Alignment::Center);
    let mut content = column![hero, rule::horizontal(1).style(p.line())]
        .spacing(if compact && !editing { 12 } else { 17 })
        .width(Length::Fill)
        .max_width(760);
    if let Some(editor) = app.editor.as_ref().filter(|_| editing) {
        content = content
            .push(text("Edit tags").size(15).font(style::semibold()))
            .push(editor.song_fields(review, p));
    } else {
        let edit = focus_button::wrap(
            "song-edit-tags",
            button(
                row![
                    icons::view(Icon::Edit, p.colors.text_muted, 14.0),
                    text("Edit tags").size(12),
                ]
                .spacing(7)
                .align_y(Alignment::Center),
            )
            .padding([7, 10])
            .style(p.secondary()),
            (!app.busy()).then_some(Message::OpenEditor(Kind::Track(0))),
            p.colors.accent,
        );
        content = content
            .push(
                row![
                    text("Tags").size(15).font(style::semibold()),
                    space::horizontal(),
                    edit,
                ]
                .align_y(Alignment::Center),
            )
            .push(tag_summary(metadata, p));
        if review.preferences.mode == CollectionMode::Album {
            content = content.push(focus_button::wrap(
                "song-edit-album",
                button("Edit shared album tags")
                    .padding([6, 8])
                    .style(p.quiet()),
                (!app.busy()).then_some(Message::OpenEditor(Kind::Album)),
                p.colors.accent,
            ));
        }
        if !review.selected.contains(&0) {
            content = content.push(focus_button::wrap(
                "include-song",
                button("Include this song")
                    .padding([8, 12])
                    .style(p.secondary()),
                (!app.busy()).then_some(Message::Toggle(0, true)),
                p.colors.accent,
            ));
        }
        if !app.example && !review.inspection.warnings.is_empty() {
            let count = review.inspection.warnings.len();
            let noun = if count == 1 { "note" } else { "notes" };
            content = content.push(focus_button::wrap(
                "song-source-notes",
                button(
                    row![
                        icons::view(Icon::Info, p.colors.warning, 14.0),
                        text(format!("{count} source {noun}")).size(12),
                    ]
                    .spacing(7)
                    .align_y(Alignment::Center),
                )
                .padding([6, 8])
                .style(p.quiet()),
                Some(Message::OpenPanel(Panel::SourceNotes)),
                p.colors.accent,
            ));
        }
    }
    scrollable(
        container(content)
            .padding(if compact { 18 } else { 28 })
            .width(Length::Fill),
    )
    .id("song-review")
    .height(Length::Fill)
    .width(Length::Fill)
    .style(p.scrollbar())
    .direction(style::scroll_direction())
    .into()
}

fn tag_summary(metadata: Metadata, p: style::Presentation) -> Element<'static, Message> {
    let label = |value| text(value).size(12).color(p.colors.text_muted);
    let value = |value: Option<String>| {
        let value = value.filter(|value| !value.trim().is_empty());
        let color = if value.is_some() {
            p.colors.text
        } else {
            p.colors.text_muted
        };
        text(value.unwrap_or_else(|| "Not set".into()))
            .size(13)
            .color(color)
            .wrapping(Wrapping::WordOrGlyph)
    };
    let mut tags = Column::new().spacing(10).width(Length::Fill);
    for (name, tag) in [
        ("Album", metadata.album),
        ("Album artist", metadata.album_artist),
    ] {
        tags = tags
            .push(row![label(name).width(100), value(Some(tag)).width(Length::Fill)].spacing(16));
    }
    let mut numbers = row![value(metadata.release_year.map(|year| year.to_string()))].spacing(20);
    for (name, tag) in [
        ("Track", metadata.track_number),
        ("Disc", metadata.disc_number),
    ] {
        if let Some(number) = tag {
            numbers = numbers.push(
                row![label(name), text(number).size(13)]
                    .spacing(7)
                    .align_y(Alignment::Center),
            );
        }
    }
    tags.push(
        row![label("Year").width(100), numbers.align_y(Alignment::Center)]
            .spacing(16)
            .align_y(Alignment::Center),
    )
    .into()
}

pub fn edit_actions(app: &App) -> Element<'_, Message> {
    let p = app.presentation();
    let action = |id, label, message, primary| {
        focus_button::wrap(
            id,
            button(text(label).size(13))
                .padding([9, 12])
                .style(move |theme, status| {
                    if primary {
                        p.primary()(theme, status)
                    } else {
                        p.secondary()(theme, status)
                    }
                }),
            (!app.busy()).then_some(message),
            p.colors.accent,
        )
    };
    let mut bar = column![].spacing(8).width(Length::Fill);
    if let Some(error) = app
        .editor
        .as_ref()
        .and_then(|editor| editor.error.as_deref())
    {
        bar = bar.push(
            text(error)
                .size(12)
                .color(p.colors.danger)
                .wrapping(Wrapping::WordOrGlyph),
        );
    }
    bar.push(
        row![
            action("cancel-song-tags", "Cancel", Message::ClosePanel, false),
            focus_button::wrap(
                "restore-source-tags",
                button(text("Restore source tags").size(12))
                    .padding([7, 8])
                    .style(p.quiet()),
                (!app.busy()).then_some(Message::RestoreSourceTags),
                p.colors.accent,
            ),
            space::horizontal(),
            action(
                "apply-song-tags",
                "Apply changes",
                Message::Editor(editor::Action::Apply),
                true
            ),
        ]
        .spacing(9)
        .align_y(Alignment::Center),
    )
    .into()
}
