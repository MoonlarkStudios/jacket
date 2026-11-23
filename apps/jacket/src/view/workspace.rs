use super::components::{self, CoverContext};
use super::{collection_tools, cover_controls, focus_button};
use crate::app::{App, Message, Panel};
use crate::editor::Kind;
use crate::icons::{self, Icon};
use crate::settings::ModeChoice;
use crate::style;
use audio_engine::CollectionMode;
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
        if !compact && app.panel.review_tool() {
            let (inspector, width, padding) = match app.panel {
                Panel::CollectionEditor => (collection_tools::editor(app, false), 384, 0),
                Panel::TrackOrder => (collection_tools::order(app, false), 284, 20),
                _ => unreachable!("song editor has its own workspace"),
            };
            split = split.push(rule::vertical(1).style(p.line())).push(
                container(inspector)
                    .width(width)
                    .padding(padding)
                    .height(Length::Fill)
                    .style(move |_| p.surface(p.colors.panel)),
            );
        }
        split.into()
    } else {
        empty_workspace(app)
    };
    let footer: Element<'_, Message> = match app.panel {
        Panel::CollectionEditor if compact => column![
            rule::horizontal(1).style(p.line()),
            collection_tools::editor(app, true)
        ]
        .into(),
        Panel::TrackOrder if compact => container(collection_tools::order(app, true))
            .padding([10, 16])
            .width(Length::Fill)
            .height(76)
            .align_y(Alignment::Center)
            .style(move |_| p.surface(p.colors.panel))
            .into(),
        Panel::Workspace if app.review.is_some() => column![
            rule::horizontal(1).style(p.line()),
            container(super::destination::save_bar(app, compact))
                .padding(if compact { [8, 16] } else { [10, 20] })
                .width(Length::Fill)
                .style(move |_| p.surface(p.colors.panel))
        ]
        .into(),
        _ => space().into(),
    };
    column![content, footer].height(Length::Fill).into()
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
    .spacing(if compact { 6 } else { 8 })
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
        details = details.push(
            actions
                .push(cover_controls::edit_button(app))
                .wrap()
                .vertical_spacing(4),
        );
    }
    let hero = row![
        components::cover(
            app,
            if compact { 76.0 } else { 104.0 },
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
            .padding(if compact { [12, 16] } else { [22, 22] })
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
