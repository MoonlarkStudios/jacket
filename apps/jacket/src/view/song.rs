//! A song is reviewed as one recording, without playlist selection controls.
use super::components::{self, CoverContext};
use crate::app::{App, Message, Panel};
use crate::editor::Kind;
use crate::icons::{self, Icon};
use crate::style;
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
    let mut heading = column![
        text("Song").size(12).color(p.colors.accent),
        text(metadata.title.clone())
            .size(if compact { 22 } else { 28 })
            .font(style::semibold())
            .width(Length::Fill),
        text(metadata.artist.clone())
            .size(14)
            .color(p.colors.text_muted),
    ]
    .spacing(9)
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
    if app.example {
        heading = heading.push(
            container(text("Example · downloads disabled").size(10))
                .padding([3, 6])
                .style(move |_| p.badge()),
        );
    }
    let hero = row![
        components::cover(
            app,
            if compact { 128.0 } else { 184.0 },
            CoverContext::Source
        ),
        heading,
    ]
    .spacing(if compact { 18 } else { 24 })
    .align_y(Alignment::Center);
    let edit = button(
        row![
            icons::view(Icon::Edit, p.colors.text_muted, 14.0),
            text("Edit tags").size(12)
        ]
        .spacing(7)
        .align_y(Alignment::Center),
    )
    .padding([7, 10])
    .on_press_maybe((!app.busy()).then_some(Message::OpenEditor(Kind::Track(0))))
    .style(p.secondary());
    let mut tags = Column::new().spacing(14).width(Length::Fill);
    for (label, value) in [
        ("Album", Some(metadata.album)),
        ("Album artist", Some(metadata.album_artist)),
        ("Year", metadata.release_year.map(|year| year.to_string())),
        (
            "Track",
            metadata.track_number.map(|number| number.to_string()),
        ),
        (
            "Disc",
            metadata.disc_number.map(|number| number.to_string()),
        ),
    ] {
        if matches!(label, "Track" | "Disc") && value.is_none() {
            continue;
        }
        let value = value
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "Not set".into());
        tags = tags.push(
            row![
                text(label).size(12).color(p.colors.text_muted).width(100),
                text(value).size(13).width(Length::Fill),
            ]
            .spacing(16),
        );
    }
    let mut content = column![
        hero,
        rule::horizontal(1).style(p.line()),
        row![
            text("Metadata").size(14).font(style::semibold()),
            space::horizontal(),
            edit
        ]
        .align_y(Alignment::Center),
        tags,
    ]
    .spacing(20)
    .width(Length::Fill);
    if review.preferences.mode == audio_engine::CollectionMode::Album {
        content = content.push(
            button("Edit shared album tags")
                .padding([6, 0])
                .on_press_maybe((!app.busy()).then_some(Message::OpenEditor(Kind::Album)))
                .style(p.quiet()),
        );
    }
    if app.source_cover_error.is_some() {
        content = content.push(
            button("Retry cover")
                .padding([6, 0])
                .on_press_maybe((!app.busy()).then_some(Message::RetryArtwork))
                .style(p.quiet()),
        );
    }
    if !review.selected.contains(&0) {
        content = content.push(
            button("Include this song")
                .padding([8, 12])
                .on_press_maybe((!app.busy()).then_some(Message::Toggle(0, true)))
                .style(p.secondary()),
        );
    }
    if !app.example && !review.inspection.warnings.is_empty() {
        content = content.push(
            button(
                row![
                    icons::view(Icon::Info, p.colors.warning, 14.0),
                    text(format!(
                        "{} source {}",
                        review.inspection.warnings.len(),
                        if review.inspection.warnings.len() == 1 {
                            "note"
                        } else {
                            "notes"
                        }
                    ))
                    .size(12),
                ]
                .spacing(7)
                .align_y(Alignment::Center),
            )
            .padding([6, 0])
            .on_press(Message::OpenPanel(Panel::SourceNotes))
            .style(p.quiet()),
        );
    }
    scrollable(
        container(content)
            .padding(if compact { 18 } else { 28 })
            .width(Length::Fill),
    )
    .height(Length::Fill)
    .width(Length::Fill)
    .style(p.scrollbar())
    .direction(crate::style::scroll_direction())
    .into()
}
