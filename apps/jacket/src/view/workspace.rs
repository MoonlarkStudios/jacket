use super::components::{self, CoverContext};
use super::options;
use crate::app::{App, Message, Panel};
use crate::editor::Kind;
use crate::icons::{self, Icon};
use crate::style;
use audio_engine::{CollectionMode, OutputKind};
use iced::widget::{button, column, container, row, rule, space, text};
use iced::{Alignment, Element, Length};

pub fn view(app: &App, compact: bool) -> Element<'_, Message> {
    let p = app.presentation();
    let content: Element<'_, Message> = if let Some(review) = &app.review {
        let title = if review.inspection.tracks.len() == 1 {
            review.inspection.tracks[0].metadata.title.as_str()
        } else if review.preferences.mode == CollectionMode::Album
            && !review.preferences.album_title.trim().is_empty()
        {
            review.preferences.album_title.as_str()
        } else {
            review.inspection.title.as_str()
        };
        let artist = if review.inspection.tracks.len() == 1 {
            review.inspection.tracks[0].metadata.artist.as_str()
        } else if !review.preferences.album_artist.trim().is_empty() {
            review.preferences.album_artist.as_str()
        } else {
            ""
        };
        let duration: f64 = review
            .inspection
            .tracks
            .iter()
            .filter_map(|track| track.duration)
            .sum();
        let count = review.inspection.tracks.len();
        let metadata = [
            (!artist.is_empty()).then(|| artist.to_owned()),
            review.preferences.year.map(|year| year.to_string()),
            Some(format!(
                "{count} {}",
                if count == 1 { "track" } else { "tracks" }
            )),
            (duration > 0.0).then(|| components::duration(duration)),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join("  ·  ");
        let mut details = column![
            iced::widget::tooltip(
                container(
                    text(title)
                        .font(style::semibold())
                        .size(if compact { 20 } else { 24 })
                        .wrapping(iced::widget::text::Wrapping::None)
                )
                .width(Length::Fill)
                .height(if compact { 28 } else { 32 })
                .clip(true),
                container(text(title).size(13).width(360))
                    .padding(10)
                    .style(move |_| p.panel()),
                iced::widget::tooltip::Position::Bottom,
            ),
            container(
                text(metadata)
                    .size(12)
                    .color(p.colors.text_muted)
                    .wrapping(iced::widget::text::Wrapping::None)
            )
            .width(Length::Fill)
            .height(18)
            .clip(true),
        ]
        .spacing(8)
        .width(Length::Fill);
        let kind = if count == 1 {
            Kind::Track(0)
        } else {
            Kind::Album
        };
        let mut actions = row![button(
            row![
                icons::view(Icon::Edit, p.colors.text_muted, 13.0),
                text(if count == 1 {
                    "Edit tags"
                } else {
                    "Edit album"
                })
                .size(12)
            ]
            .spacing(6)
            .align_y(Alignment::Center)
        )
        .padding([5, 0])
        .on_press_maybe((!app.busy()).then_some(Message::OpenEditor(kind)))
        .style(p.quiet()),]
        .spacing(12)
        .align_y(Alignment::Center);
        if !app.example && !review.inspection.warnings.is_empty() {
            actions = actions.push(
                button(
                    text(format!(
                        "{} source {}",
                        review.inspection.warnings.len(),
                        if review.inspection.warnings.len() == 1 {
                            "note"
                        } else {
                            "notes"
                        }
                    ))
                    .size(12)
                    .color(p.colors.warning),
                )
                .padding([5, 0])
                .on_press(Message::OpenPanel(Panel::SourceNotes))
                .style(p.quiet()),
            );
        }
        if app.example {
            actions = actions
                .push(
                    container(text("Example").size(10))
                        .padding([3, 6])
                        .style(move |_| p.badge()),
                )
                .push(
                    button("Close example")
                        .padding([5, 0])
                        .on_press_maybe((!app.busy()).then_some(Message::ConfirmReset))
                        .style(p.quiet()),
                );
        }
        details = details.push(actions);
        let hero = row![
            components::cover(
                app,
                if compact { 86.0 } else { 104.0 },
                CoverContext::Source
            ),
            details
        ]
        .spacing(if compact { 16 } else { 20 })
        .align_y(Alignment::Center);
        let left = column![
            container(hero)
                .padding(if compact { 16 } else { 22 })
                .width(Length::Fill),
            rule::horizontal(1).style(p.line()),
            container(crate::track_list::view(app, compact))
                .padding(if compact { 12 } else { 16 })
                .height(Length::Fill),
        ]
        .height(Length::Fill)
        .width(Length::Fill);
        if compact {
            left.into()
        } else {
            row![
                left,
                rule::vertical(1).style(p.line()),
                container(options::view(app, true))
                    .width(284)
                    .padding(20)
                    .height(Length::Fill)
                    .style(move |_| p.surface(p.colors.panel))
            ]
            .height(Length::Fill)
            .into()
        }
    } else if let Some(result) = &app.result {
        let mut content = column![
            container(icons::view(Icon::Check, p.colors.success, 28.0))
                .padding(14)
                .style(move |_| p.panel()),
            text("Download complete").size(22).font(style::semibold()),
            text(format!(
                "{} {} saved",
                result.track_count,
                if result.track_count == 1 {
                    "track"
                } else {
                    "tracks"
                }
            ))
            .size(14)
            .color(p.colors.text_muted),
            text(&result.output_path)
                .size(12)
                .width(Length::Fill)
                .color(p.colors.text_muted),
            row![
                button(
                    row![
                        icons::view(Icon::Folder, p.colors.accent_text, 15.0),
                        text(if result.output_kind == OutputKind::File {
                            "Show file"
                        } else {
                            "Open folder"
                        })
                        .size(13)
                    ]
                    .spacing(7)
                    .align_y(Alignment::Center)
                )
                .padding([10, 14])
                .on_press(Message::OpenOutput)
                .style(p.primary()),
                button("New download")
                    .padding([10, 14])
                    .on_press(Message::ConfirmReset)
                    .style(p.secondary()),
            ]
            .spacing(10),
        ]
        .spacing(16)
        .max_width(540);
        if result.playlist_path.is_some() {
            content = content.push(
                button("Open playlist")
                    .on_press(Message::OpenPlaylist)
                    .style(p.quiet()),
            );
        }
        for warning in &result.warnings {
            content = content.push(text(warning).size(12).color(p.colors.warning));
        }
        container(content).center(Length::Fill).padding(28).into()
    } else {
        empty_workspace(app)
    };
    if compact {
        let footer: Element<'_, Message> = if app.review.is_some() {
            container(
                row![
                    button(
                        row![
                            icons::view(Icon::Settings, p.colors.text_muted, 15.0),
                            text("Export options").size(12)
                        ]
                        .spacing(7)
                        .align_y(Alignment::Center)
                    )
                    .padding([10, 12])
                    .on_press(Message::OpenPanel(Panel::Options))
                    .style(p.secondary()),
                    space::horizontal(),
                    button(
                        row![
                            text("Preview files").size(13),
                            icons::view(
                                Icon::ArrowRight,
                                if app.can_preview() {
                                    p.colors.accent_text
                                } else {
                                    p.derived.disabled
                                },
                                15.0
                            )
                        ]
                        .spacing(9)
                        .align_y(Alignment::Center)
                    )
                    .padding([11, 14])
                    .on_press_maybe(app.can_preview().then_some(Message::Preview))
                    .style(p.primary()),
                ]
                .align_y(Alignment::Center),
            )
            .padding([12, 16])
            .width(Length::Fill)
            .style(move |_| p.surface(p.colors.panel))
            .into()
        } else {
            space().into()
        };
        column![content, footer].height(Length::Fill).into()
    } else {
        content
    }
}

fn empty_workspace(app: &App) -> Element<'_, Message> {
    let p = app.presentation();
    let emblem = container(icons::view(Icon::Link, p.colors.accent, 28.0))
        .padding(14)
        .style(move |_| p.panel());
    let title = text("Prepare audio from a link")
        .size(23)
        .font(style::semibold());
    let description = text(
        "Add a song, album or playlist. Review its tags and artwork, then save the audio to your library.",
    )
    .size(14)
    .color(p.colors.text_muted)
    .align_x(Alignment::Center)
    .width(Length::Fill);
    let paste = button(
        row![
            icons::view(Icon::Link, p.colors.accent_text, 15.0),
            text("Paste a link").size(13),
        ]
        .spacing(7)
        .align_y(Alignment::Center),
    )
    .padding([11, 18])
    .on_press_maybe((!app.busy()).then_some(Message::PasteLink))
    .style(p.primary());
    let example = button("View example album")
        .on_press_maybe((!app.busy()).then_some(Message::Example))
        .style(p.quiet());
    let content = column![emblem, title, description, paste, example]
        .spacing(18)
        .align_x(Alignment::Center)
        .max_width(390);
    container(content).center(Length::Fill).padding(24).into()
}
