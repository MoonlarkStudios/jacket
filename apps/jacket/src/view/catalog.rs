use super::{catalog_search, components};
use crate::app::{artist_credit_id, App, CatalogAction as Action, CatalogPending, Message};
use crate::icons::{self, Icon};
use crate::style;
use audio_engine::Inspection;
use iced::widget::{button, column, container, row, rule, scrollable, space, text, text_input};
use iced::{Alignment, Element, Length};
use jacket_desktop::youtube_music::AlbumPage;

const CONTENT_WIDTH: f32 = 910.0;

pub(super) fn view(app: &App, compact: bool) -> Element<'_, Message> {
    let p = app.presentation();
    let catalog = &app.catalog;
    let edge = if compact { 18 } else { 32 };
    let mut layout = column![].height(Length::Fill);
    if catalog.search_home() {
        layout = layout.push(page_header(app, search_header(app), edge));
    } else if let Some(album) = catalog.album() {
        layout = layout.push(page_header(app, album_header(app, album, compact), edge));
    }

    let body = if let Some(album) = catalog.album() {
        album_tracks(app, &album.inspection)
    } else {
        catalog_search::view(app, compact)
    };
    layout = layout.push(
        scrollable(
            container(
                container(body)
                    .padding([18, edge])
                    .width(Length::Fill)
                    .max_width(CONTENT_WIDTH),
            )
            .center_x(Length::Fill),
        )
        .id("album-browser")
        .on_scroll(|viewport| Message::Catalog(Action::Scrolled(viewport.absolute_offset().y)))
        .height(Length::Fill)
        .width(Length::Fill)
        .style(p.scrollbar())
        .direction(style::scroll_direction()),
    );
    if catalog.error.is_some() || catalog.pending.is_some() || catalog.confirm_replace {
        layout = layout.push(rule::horizontal(1).style(p.line())).push(
            container(
                container(status(app))
                    .width(Length::Fill)
                    .max_width(CONTENT_WIDTH),
            )
            .padding([10, edge])
            .center_x(Length::Fill)
            .style(move |_| p.surface(p.colors.panel)),
        );
    }
    layout.into()
}

fn page_header<'a>(app: &'a App, content: Element<'a, Message>, edge: u16) -> Element<'a, Message> {
    let p = app.presentation();
    column![
        container(
            container(content)
                .padding([20, edge])
                .width(Length::Fill)
                .max_width(CONTENT_WIDTH)
        )
        .center_x(Length::Fill),
        rule::horizontal(1).style(p.line()),
    ]
    .into()
}

fn search_header(app: &App) -> Element<'_, Message> {
    let p = app.presentation();
    let catalog = &app.catalog;
    let editing = !catalog.confirm_replace
        && !matches!(
            catalog.pending,
            Some(CatalogPending::Artist | CatalogPending::Album | CatalogPending::Song)
        );
    let can_replace = matches!(catalog.pending, Some(CatalogPending::Search))
        && catalog.query.trim() != catalog.searched;
    let can_search = (!app.busy() || can_replace)
        && editing
        && app.desktop.is_some()
        && !catalog.query.trim().is_empty();
    let search = text_input("Artist, album, or song", &catalog.query)
        .id("album-search")
        .on_input_maybe(editing.then_some(|query| Message::Catalog(Action::Query(query))))
        .on_submit_maybe(can_search.then_some(Message::Catalog(Action::Search)))
        .size(16)
        .padding([11, 13])
        .style(p.input());
    let (id, label, on_press) = if app.review.is_some() || app.result.is_some() {
        ("return-to-review", "Back to review", Action::ReturnToReview)
    } else {
        ("open-link", "Paste a link", Action::PasteLink)
    };

    column![
        row![
            text("Find music")
                .size(27)
                .font(style::semibold())
                .width(Length::Fill),
            action(app, id, label, on_press, !app.busy(), false),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
        row![
            search,
            if catalog.query.is_empty() {
                space().into()
            } else {
                action(
                    app,
                    "clear-music-search",
                    "Clear",
                    Action::ClearSearch,
                    editing,
                    false,
                )
            },
            action(
                app,
                "search-albums",
                "Search",
                Action::Search,
                can_search,
                true
            ),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        catalog_search::filters(app),
    ]
    .spacing(14)
    .into()
}

fn album_header<'a>(app: &'a App, page: &'a AlbumPage, compact: bool) -> Element<'a, Message> {
    let p = app.presentation();
    let album = &page.inspection;
    let count = album.tracks.len();
    let mut details = Vec::new();
    if let Some(year) = album.album_release_year {
        details.push(year.to_string());
    }
    details.push(if count == 1 {
        "1 track".into()
    } else {
        format!("{count} tracks")
    });
    if let Some(seconds) = album
        .tracks
        .iter()
        .map(|track| track.duration)
        .sum::<Option<f64>>()
    {
        details.push(components::duration(seconds));
    }
    let cover = album
        .artwork_url
        .as_deref()
        .and_then(|url| app.catalog.media.get(url));

    column![
        action(
            app,
            "album-back",
            app.catalog.back_label(),
            Action::Back,
            !app.busy() && !app.catalog.confirm_replace,
            false,
        ),
        row![
            catalog_search::thumbnail(app, cover, None, if compact { 112.0 } else { 164.0 }),
            column![
                text(&album.title)
                    .size(if compact { 25 } else { 36 })
                    .font(style::semibold()),
                album_artist_credits(app, page),
                text(details.join("  ·  "))
                    .size(13)
                    .color(p.colors.text_muted),
                action(
                    app,
                    "review-album",
                    "Review download",
                    Action::Review,
                    !app.busy() && !app.catalog.confirm_replace,
                    true,
                ),
            ]
            .spacing(8)
            .width(Length::Fill),
        ]
        .spacing(if compact { 15 } else { 23 })
        .align_y(Alignment::Center),
    ]
    .spacing(14)
    .into()
}

fn album_artist_credits<'a>(app: &'a App, page: &'a AlbumPage) -> Element<'a, Message> {
    if page.artists.is_empty() {
        let artist = page
            .inspection
            .tracks
            .first()
            .map_or("", |track| track.metadata.album_artist.as_str());
        return text(artist).size(16).into();
    }

    let p = app.presentation();
    let enabled = !app.busy() && !app.catalog.confirm_replace;
    let mut credits = row![].spacing(4).align_y(Alignment::Center);
    for (index, artist) in page.artists.iter().enumerate() {
        if index > 0 {
            credits = credits.push(text("&").size(14).color(p.colors.text_muted));
        }
        if artist.id.is_some() {
            let label = text(&artist.name).size(16).color(if enabled {
                p.colors.accent
            } else {
                p.colors.text_muted
            });
            credits = credits.push(super::focus_button::wrap(
                artist_credit_id(index),
                button(label).padding([3, 4]).style(p.quiet()),
                enabled.then_some(Message::Catalog(Action::OpenAlbumArtist(index))),
                p.colors.accent,
            ));
        } else {
            credits = credits.push(text(&artist.name).size(16));
        }
    }
    credits.wrap().vertical_spacing(2).into()
}

fn album_tracks<'a>(app: &'a App, album: &'a Inspection) -> Element<'a, Message> {
    let p = app.presentation();
    let mut tracks = column![text("Track list").size(18).font(style::semibold())].spacing(8);
    for warning in &album.warnings {
        tracks = tracks.push(text(warning).color(p.colors.text_muted).size(13));
    }
    tracks
        .extend(album.tracks.iter().enumerate().map(|(index, track)| {
            let number = track
                .metadata
                .track_number
                .map_or_else(|| index + 1, |tag| tag as usize);
            let mut labels = column![text(&track.metadata.title).size(15)]
                .spacing(3)
                .width(Length::Fill);
            if track.metadata.artist != track.metadata.album_artist {
                labels = labels.push(
                    text(&track.metadata.artist)
                        .size(12)
                        .color(p.colors.text_muted),
                );
            }
            column![
                rule::horizontal(1).style(p.line()),
                row![
                    text(number.to_string())
                        .size(12)
                        .color(p.colors.text_muted)
                        .width(32),
                    labels,
                    text(track.duration.map(components::duration).unwrap_or_default())
                        .size(12)
                        .color(p.colors.text_muted),
                ]
                .padding([8, 5])
                .spacing(12)
                .align_y(Alignment::Center),
            ]
            .into()
        }))
        .into()
}

fn status(app: &App) -> Element<'_, Message> {
    let p = app.presentation();
    let catalog = &app.catalog;
    let mut footer = column![].spacing(9);
    if let Some(error) = &catalog.error {
        let mut failure = row![text(error)
            .size(13)
            .color(p.colors.danger)
            .width(Length::Fill)]
        .spacing(10)
        .align_y(Alignment::Center);
        if catalog.search_home() && !catalog.query.trim().is_empty() && app.desktop.is_some() {
            failure = failure.push(action(
                app,
                "search-in-browser",
                "Search in browser",
                Action::OpenInBrowser,
                true,
                false,
            ));
        }
        footer = footer.push(failure);
    }
    if let Some(pending) = catalog.pending {
        let label = if catalog.cancelling {
            "Cancelling…"
        } else {
            match pending {
                CatalogPending::Search => "Searching YouTube Music…",
                CatalogPending::Artist => "Loading artist…",
                CatalogPending::Album => "Loading album…",
                CatalogPending::Song => "Loading recording details…",
            }
        };
        footer = footer.push(
            row![
                text(label)
                    .size(13)
                    .color(p.colors.text_muted)
                    .width(Length::Fill),
                action(
                    app,
                    "cancel-catalog",
                    "Cancel",
                    Action::Cancel,
                    !catalog.cancelling,
                    false
                ),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        );
    }
    if catalog.confirm_replace {
        footer = footer
            .push(
                text("Replace the current review?")
                    .size(14)
                    .font(style::semibold()),
            )
            .push(
                row![
                    action(
                        app,
                        "keep-current-review",
                        "Keep current review",
                        Action::ReturnToReview,
                        !app.busy(),
                        false
                    ),
                    action(
                        app,
                        "confirm-album-review",
                        "Replace and review",
                        Action::ConfirmReview,
                        !app.busy(),
                        true
                    ),
                ]
                .spacing(8),
            );
    }
    footer.into()
}

pub(super) fn card<'a>(
    app: &'a App,
    id: String,
    title: String,
    subtitle: String,
    thumbnail: Option<Element<'a, Message>>,
    select: Action,
) -> Element<'a, Message> {
    let p = app.presentation();
    let labels = column![
        text(title).size(15).font(style::semibold()),
        text(subtitle).size(12).color(p.colors.text_muted),
    ]
    .spacing(4)
    .width(Length::Fill);
    let mut item = row![].spacing(10).align_y(Alignment::Center);
    if let Some(thumbnail) = thumbnail {
        item = item.push(thumbnail);
    }
    super::focus_button::wrap(
        id,
        button(
            item.push(labels)
                .push(icons::view(Icon::ArrowRight, p.colors.text_muted, 16.0)),
        )
        .width(Length::Fill)
        .padding([9, 12])
        .style(p.secondary()),
        (!app.busy() && !app.catalog.confirm_replace).then_some(Message::Catalog(select)),
        p.colors.accent,
    )
}

pub(super) fn action<'a>(
    app: &App,
    id: impl Into<iced::advanced::widget::Id>,
    label: impl Into<String>,
    event: Action,
    enabled: bool,
    primary: bool,
) -> Element<'a, Message> {
    let p = app.presentation();
    let control = button(text(label.into()).size(13)).padding([8, 12]);
    let control = if primary {
        control.style(p.primary())
    } else {
        control.style(p.secondary())
    };
    super::focus_button::wrap(
        id,
        control,
        enabled.then_some(Message::Catalog(event)),
        p.colors.accent,
    )
}
