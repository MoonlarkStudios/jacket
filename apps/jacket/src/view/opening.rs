use super::{components, focus_button};
use crate::app::{App, Message, SourceKind};
use crate::icons::{self, Icon};
use crate::style;
use iced::widget::text::Wrapping;
use iced::widget::{button, column, container, row, rule, scrollable, text};
use iced::{Alignment, Element, Length};

pub fn view(app: &App, compact: bool) -> Option<Element<'_, Message>> {
    let intake = app.intake.as_ref()?;
    let p = app.presentation();
    let edge = if compact { 96.0 } else { 128.0 };
    let cover = intake.cover.as_ref().map_or_else(
        || {
            container(icons::view(Icon::Music, p.colors.text_muted, 26.0))
                .center(edge)
                .style(move |_| p.panel())
                .into()
        },
        |cover| components::raster_cover(cover, edge, p),
    );
    let activity = if intake.cancelling {
        "Cancelling lookup…".to_owned()
    } else if intake.count == 0 {
        format!("Looking up {}…", intake.kind.label())
    } else if intake.kind == SourceKind::Song {
        "Preparing song review…".to_owned()
    } else if let Some(total) = intake.total.filter(|total| *total > intake.count) {
        format!("{} of {total} tracks found", intake.count)
    } else {
        let noun = if intake.count == 1 { "track" } else { "tracks" };
        format!("{} {noun} found", intake.count)
    };
    let activity = text(activity)
        .size(13)
        .color(p.colors.accent)
        .wrapping(Wrapping::WordOrGlyph);
    let mut heading = column![].spacing(8).width(Length::Fill);
    if intake.title.is_empty() {
        heading = heading.push(
            activity
                .size(if compact { 20 } else { 24 })
                .font(style::semibold()),
        );
    } else {
        heading = heading.push(activity).push(
            text(&intake.title)
                .size(if compact { 23 } else { 29 })
                .font(style::semibold())
                .wrapping(Wrapping::WordOrGlyph),
        );
        if intake.kind == SourceKind::Song {
            if let Some(track) = intake
                .tracks
                .first()
                .filter(|track| !track.artist.is_empty())
            {
                heading = heading.push(
                    text(&track.artist)
                        .size(14)
                        .color(p.colors.text_muted)
                        .wrapping(Wrapping::WordOrGlyph),
                );
            }
        }
    }
    let mut content = column![row![cover, heading].spacing(18).align_y(Alignment::Center)]
        .spacing(16)
        .max_width(600);
    let cancel = focus_button::wrap(
        "cancel-lookup",
        button(text("Cancel lookup").size(13))
            .padding([9, 13])
            .style(p.secondary()),
        (!intake.cancelling && app.job.is_some()).then_some(Message::Cancel),
        p.colors.accent,
    );
    let mut actions = row![cancel].spacing(12).align_y(Alignment::Center);
    if app.review.is_some() {
        actions = actions.push(
            text("Cancel to return to your current review.")
                .size(12)
                .color(p.colors.text_muted),
        );
    }
    content = content.push(actions);
    if intake.kind != SourceKind::Song && !intake.tracks.is_empty() {
        let mut tracks = column![].spacing(9).width(Length::Fill);
        for track in &intake.tracks {
            let mut line =
                column![text(&track.title).size(13).wrapping(Wrapping::WordOrGlyph)].spacing(2);
            if !track.artist.is_empty() {
                line = line.push(
                    text(&track.artist)
                        .size(12)
                        .color(p.colors.text_muted)
                        .wrapping(Wrapping::WordOrGlyph),
                );
            }
            tracks = tracks.push(line);
        }
        content = content
            .push(rule::horizontal(1).style(p.line()))
            .push(tracks);
    }
    Some(
        scrollable(
            container(content)
                .center_x(Length::Fill)
                .padding(iced::Padding {
                    top: if compact { 38.0 } else { 64.0 },
                    ..iced::Padding::from(if compact { 18 } else { 32 })
                }),
        )
        .style(p.scrollbar())
        .direction(style::scroll_direction())
        .width(Length::Fill)
        .height(Length::Fill)
        .into(),
    )
}
