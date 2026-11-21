use super::components::{self, CoverContext};
use super::{collection_tools, cover_controls, focus_button, options};
use crate::app::{App, Message, Panel};
use crate::editor::Kind;
use crate::icons::{self, Icon};
use crate::settings::ModeChoice;
use crate::style;
use audio_engine::{CollectionMode, OutputKind};
use iced::widget::{button, column, container, row, rule, space, text};
use iced::{Alignment, Element, Length};

pub fn view(app: &App, compact: bool) -> Element<'_, Message> {
    if app.panel == Panel::SongEditor {
        let p = app.presentation();
        return column![
            super::song::view(app, compact),
            rule::horizontal(1).style(p.line()),
            container(
                container(super::song::edit_actions(app))
                    .max_width(760)
                    .width(Length::Fill)
            )
            .padding(if compact { [10, 18] } else { [12, 28] })
            .width(Length::Fill)
            .style(move |_| p.surface(p.colors.panel)),
        ]
        .height(Length::Fill)
        .into();
    }
    if let Some(opening) = super::opening::view(app, compact) {
        return opening;
    }
    if let Some(receipt) = &app.result {
        return super::receipt::view(app, receipt, compact);
    }
    let p = app.presentation();
    let content: Element<'_, Message> = if let Some(review) = &app.review {
        let left: Element<'_, Message> = if review.is_song() {
            super::song::view(app, compact)
        } else {
            collection(app, compact)
        };
        let mut split = row![left].height(Length::Fill);
        if !compact {
            let inspector = match app.panel {
                Panel::CollectionEditor => collection_tools::editor(app, false),
                Panel::TrackOrder => collection_tools::order(app, false),
                _ => options::view(app, true),
            };
            split = split.push(rule::vertical(1).style(p.line())).push(
                container(inspector)
                    .width(if app.panel == Panel::CollectionEditor {
                        384
                    } else {
                        284
                    })
                    .padding(if app.panel == Panel::CollectionEditor {
                        0
                    } else {
                        20
                    })
                    .height(Length::Fill)
                    .style(move |_| p.surface(p.colors.panel)),
            );
        }
        split.into()
    } else {
        empty_workspace(app)
    };
    if compact {
        let footer: Element<'_, Message> = if app.panel == Panel::CollectionEditor {
            column![
                rule::horizontal(1).style(p.line()),
                collection_tools::editor(app, true)
            ]
            .into()
        } else if app.panel == Panel::TrackOrder {
            container(collection_tools::order(app, true))
                .padding([10, 16])
                .width(Length::Fill)
                .height(76)
                .align_y(Alignment::Center)
                .style(move |_| p.surface(p.colors.panel))
                .into()
        } else if app
            .review
            .as_ref()
            .is_some_and(|review| review.preferences.output_kind == OutputKind::File)
        {
            container(super::destination::save_controls(
                app,
                super::destination::Placement::Compact,
            ))
            .padding([10, 16])
            .width(Length::Fill)
            .style(move |_| p.surface(p.colors.panel))
            .into()
        } else if app.review.is_some() {
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
        column![content].height(Length::Fill).into()
    }
}

fn collection(app: &App, compact: bool) -> Element<'_, Message> {
    let p = app.presentation();
    let Some(review) = &app.review else {
        return space().into();
    };
    let shared = review.preferences.mode == CollectionMode::Album;
    let album = review.inspection.suggested_mode == CollectionMode::Album;
    let title = if album && shared && !review.preferences.album_title.trim().is_empty() {
        review.preferences.album_title.as_str()
    } else if album {
        review.inspection.title.as_str()
    } else {
        review.preferences.title.as_str()
    };
    let artist = if shared && !review.preferences.album_artist.trim().is_empty() {
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
        review
            .preferences
            .year
            .filter(|_| shared)
            .map(|year| year.to_string()),
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
        text(if album { "Album" } else { "Playlist" })
            .size(11)
            .color(p.colors.accent),
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
    let idle = !app.busy() && app.panel == Panel::Workspace;
    let action = |id, label, message| {
        focus_button::wrap(
            id,
            button(text(label).size(12))
                .padding([5, 7])
                .style(p.quiet()),
            idle.then_some(message),
            p.colors.accent,
        )
    };
    let mut actions = row![].spacing(6).align_y(Alignment::Center);
    if shared {
        actions = actions.push(action(
            "collection-edit-album",
            "Edit shared tags",
            Message::OpenEditor(Kind::Album),
        ));
    }
    if !album {
        actions = actions.push(action(
            "collection-rename",
            "Rename playlist",
            Message::OpenEditor(Kind::PlaylistName),
        ));
    }
    actions = actions.push(action(
        "collection-tag-mode",
        if shared {
            "Use individual tags"
        } else {
            "Use shared album tags"
        },
        Message::Mode(if shared {
            ModeChoice::Collection
        } else {
            ModeChoice::Album
        }),
    ));
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
            .on_press_maybe(idle.then_some(Message::OpenPanel(Panel::SourceNotes)))
            .style(p.quiet()),
        );
    }
    if idle && app.example {
        actions = actions.push(
            button("Close example")
                .padding([5, 0])
                .on_press_maybe((!app.busy()).then_some(Message::ConfirmReset))
                .style(p.quiet()),
        );
    }
    if !app.panel.review_tool() {
        details = details.push(actions.wrap().vertical_spacing(4));
        details = details.push(cover_controls::view(app, review));
    }
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
    let heading = if compact && app.panel == Panel::CollectionEditor {
        container(
            text(title)
                .size(13)
                .font(style::semibold())
                .wrapping(iced::widget::text::Wrapping::None)
                .width(Length::Fill)
                .height(18),
        )
        .padding([8, 16])
        .width(Length::Fill)
        .clip(true)
    } else {
        container(hero)
            .padding(if compact { 16 } else { 22 })
            .width(Length::Fill)
    };
    let collection = column![
        heading,
        rule::horizontal(1).style(p.line()),
        container(crate::track_list::view(app, compact))
            .padding(if compact { 12 } else { 16 })
            .height(Length::Fill),
    ]
    .height(Length::Fill)
    .width(Length::Fill);
    collection.into()
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
    let mut content = column![emblem, title, description, paste]
        .spacing(18)
        .align_x(Alignment::Center)
        .max_width(390);
    // Fictional data is an explicit developer entry point, never an ordinary CTA.
    if app.example {
        content = content.push(
            button("Open example album")
                .on_press_maybe((!app.busy()).then_some(Message::Example))
                .style(p.quiet()),
        );
    }
    container(content).center(Length::Fill).padding(24).into()
}
