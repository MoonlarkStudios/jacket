use super::{catalog_search, components, focus_button};
use crate::app::{
    artist_credit_id, edition_id, App, CatalogAction as Action, CatalogPending, Message,
    CATALOG_SCROLL_ID,
};
use crate::icons::{self, Icon};
use crate::style::{self, CatalogSurface};
use audio_engine::Inspection;
use iced::widget::{
    button, column, container, responsive, row, rule, scrollable, space, stack, text, text_input,
    tooltip, Text,
};
use iced::{Alignment, Element, Length, Padding};
use jacket_core::SourceLink;
use jacket_desktop::youtube_music::AlbumPage;
use std::time::Duration;

const BROWSE_WIDTH: f32 = 1280.0;
const ALBUM_PAGE_WIDTH: f32 = 910.0;

pub(super) fn view(app: &App, compact: bool) -> Element<'_, Message> {
    let p = app.presentation();
    let catalog = &app.catalog;
    let album = catalog.album();
    let content_width = if album.is_some() {
        ALBUM_PAGE_WIDTH
    } else {
        BROWSE_WIDTH
    };
    let edge = if compact { 18 } else { 32 };
    let compact_search = compact && catalog.search_home();
    let header_padding = [if compact_search { 12 } else { 20 }, edge];
    let body_padding = [if compact_search { 14 } else { 18 }, edge];
    let mut layout = column![].height(Length::Fill);
    if catalog.search_home() {
        layout = layout.push(page_header(
            app,
            search_header(app, compact_search),
            header_padding,
            content_width,
        ));
    } else if let Some(album) = album {
        layout = layout.push(page_header(
            app,
            album_header(app, album, compact),
            header_padding,
            content_width,
        ));
    }

    let body = if let Some(album) = album {
        let tracks = album_tracks(app, &album.inspection);
        if catalog.edition_shelf().is_some_and(|shelf| shelf.open) {
            column![other_editions(app), tracks].spacing(24).into()
        } else {
            tracks
        }
    } else {
        responsive(move |size| catalog_search::view(app, compact, size.width))
            .height(Length::Shrink)
            .into()
    };
    layout = layout.push(
        scrollable(
            container(
                container(body)
                    .padding(body_padding)
                    .width(Length::Fill)
                    .max_width(content_width),
            )
            .center_x(Length::Fill),
        )
        .id(CATALOG_SCROLL_ID)
        .on_scroll(|viewport| Message::Catalog(Action::Scrolled(viewport.absolute_offset().y)))
        .height(Length::Fill)
        .width(Length::Fill)
        .style(p.scrollbar())
        .direction(style::scroll_direction()),
    );
    if (catalog.error.is_some() && !(catalog.search_home() && catalog.link_input()))
        || catalog.pending.is_some()
        || catalog.confirm_replace
    {
        layout = layout.push(rule::horizontal(1).style(p.line())).push(
            container(
                container(status(app))
                    .width(Length::Fill)
                    .max_width(content_width),
            )
            .padding([10, edge])
            .center_x(Length::Fill)
            .style(move |_| p.surface(p.colors.panel)),
        );
    }
    layout.into()
}

fn page_header<'a>(
    app: &'a App,
    content: Element<'a, Message>,
    padding: [u16; 2],
    content_width: f32,
) -> Element<'a, Message> {
    let p = app.presentation();
    column![
        container(
            container(content)
                .padding(padding)
                .width(Length::Fill)
                .max_width(content_width)
        )
        .center_x(Length::Fill),
        rule::horizontal(1).style(p.line()),
    ]
    .into()
}

fn search_header(app: &App, compact: bool) -> Element<'_, Message> {
    let catalog = &app.catalog;
    let mut heading = row![text("Find music")
        .size(27)
        .font(style::semibold())
        .width(Length::Fill)]
    .spacing(10)
    .align_y(Alignment::Center);
    if app.review.is_some() || app.result.is_some() {
        heading = heading.push(action(
            app,
            "return-to-review",
            "Back to review",
            Action::ReturnToReview,
            !app.busy(),
            false,
        ));
    }

    let mut header = column![heading, search_field(app)].spacing(if compact { 10 } else { 14 });
    if let Some(error) = catalog.error.as_deref().filter(|_| catalog.link_input()) {
        header = header.push(text(error).size(13).color(app.presentation().colors.danger));
    }
    header.push(catalog_search::filters(app)).into()
}

fn search_field(app: &App) -> Element<'_, Message> {
    const INSET: f32 = 6.0;
    const GAP: f32 = 4.0;
    const CLEAR_WIDTH: f32 = 32.0;
    const OPEN_WIDTH: f32 = 104.0;

    let p = app.presentation();
    let catalog = &app.catalog;
    let link_input = catalog.link_input();
    let show_open = link_input && SourceLink::parse(&catalog.query).is_ok();
    let show_clear = !catalog.query.is_empty();
    let editing = !catalog.confirm_replace
        && !matches!(
            catalog.pending,
            Some(CatalogPending::Artist | CatalogPending::Album | CatalogPending::Song)
        );
    let searching = matches!(catalog.pending, Some(CatalogPending::Search));
    let search_terms = catalog.search_terms();
    let can_submit = editing
        && if link_input {
            !app.busy() || searching
        } else {
            app.desktop.is_some()
                && search_terms
                    .is_some_and(|query| !app.busy() || (searching && query != catalog.searched))
        };
    let submit = can_submit.then(|| Message::Catalog(Action::Submit(catalog.query.clone())));

    let mut controls = row![].spacing(GAP).align_y(Alignment::Center);
    let mut right = 13.0;
    if show_open {
        controls = controls.push(focus_button::wrap_inset(
            "open-search-link",
            button(text("Open link ↵").size(13).width(Length::Fill).center())
                .width(OPEN_WIDTH)
                .padding([8, 6])
                .style(p.primary()),
            submit.clone(),
            p.colors.accent,
            p.colors.background,
        ));
        right += OPEN_WIDTH + GAP;
    }
    if show_clear {
        let clear = focus_button::wrap_inset(
            "clear-music-search",
            button(icons::view(Icon::Close, p.colors.text_muted, 16.0))
                .width(CLEAR_WIDTH)
                .padding(8)
                .style(p.quiet()),
            editing.then_some(Message::Catalog(Action::ClearSearch)),
            p.colors.accent,
            p.colors.background,
        );
        controls = controls.push(
            tooltip(
                clear,
                text("Clear search").size(12),
                tooltip::Position::Bottom,
            )
            .delay(Duration::from_millis(500)),
        );
        right += CLEAR_WIDTH;
    }
    let input = text_input("Search music or paste a YouTube link", &catalog.query)
        .id("album-search")
        .on_input_maybe(editing.then_some(|query| Message::Catalog(Action::Query(query))))
        .on_submit_maybe(submit)
        .size(16)
        .padding(Padding {
            top: 11.0,
            bottom: 11.0,
            left: 38.0,
            right,
        })
        .style(p.input());
    let adornments = container(
        row![
            icons::view(
                if link_input { Icon::Link } else { Icon::Search },
                p.colors.text_muted,
                17.0,
            ),
            space::horizontal(),
            controls,
        ]
        .align_y(Alignment::Center),
    )
    .padding(Padding {
        left: 13.0,
        right: INSET,
        ..Padding::ZERO
    })
    .center_y(Length::Fill);

    stack![input, adornments].into()
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
    let cover = app.catalog.media.cover(album.artwork_url.as_deref());

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
                row![
                    action(
                        app,
                        "review-album",
                        "Review download",
                        Action::Review,
                        !app.busy() && !app.catalog.confirm_replace,
                        true,
                    ),
                    action(
                        app,
                        "toggle-editions",
                        if app.catalog.edition_shelf().is_some_and(|shelf| shelf.open) {
                            "Hide editions"
                        } else {
                            "Other editions"
                        },
                        Action::ToggleEditions,
                        !app.busy() && !app.catalog.confirm_replace,
                        false,
                    ),
                ]
                .spacing(8)
                .wrap()
                .vertical_spacing(6),
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

fn other_editions(app: &App) -> Element<'_, Message> {
    let p = app.presentation();
    let mut section = column![text("Other editions").size(18).font(style::semibold())].spacing(8);
    if let Some(rows) = app
        .catalog
        .edition_shelf()
        .and_then(|shelf| shelf.rows.as_ref())
    {
        if rows.is_empty() {
            section = section.push(
                text("No other editions found on YouTube Music.")
                    .size(14)
                    .color(p.colors.text_muted),
            );
        }
        section = section.extend(rows.iter().map(|edition| {
            let mut details = Vec::new();
            if let Some(year) = edition.year {
                details.push(year.to_string());
            }
            if let Some(count) = edition.track_count {
                details.push(if count == 1 {
                    "1 track".into()
                } else {
                    format!("{count} tracks")
                });
            }
            let cover = app.catalog.media.cover(edition.artwork_url.as_deref());
            card(
                app,
                edition_id(&edition.id),
                text(&edition.title).size(15),
                details.join(" · "),
                Some(catalog_search::thumbnail(app, cover, None, 48.0)),
                Action::OpenEdition(edition.id.clone()),
            )
        }));
    }
    section.into()
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
    if let Some(error) = catalog
        .error
        .as_ref()
        .filter(|_| !(catalog.search_home() && catalog.link_input()))
    {
        let mut failure = row![text(error)
            .size(13)
            .color(p.colors.danger)
            .width(Length::Fill)]
        .spacing(10)
        .align_y(Alignment::Center);
        if catalog.search_home() && catalog.search_terms().is_some() && app.desktop.is_some() {
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
                CatalogPending::ArtistReleases => "Loading all releases…",
                CatalogPending::Album => "Loading album…",
                CatalogPending::Editions => "Looking for other editions…",
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
    title: Text<'a>,
    subtitle: String,
    thumbnail: Option<Element<'a, Message>>,
    select: Action,
) -> Element<'a, Message> {
    let p = app.presentation();
    let mut labels = column![title.font(style::semibold())]
        .spacing(4)
        .width(Length::Fill);
    if !subtitle.is_empty() {
        labels = labels.push(text(subtitle).size(12).color(p.colors.text_muted));
    }
    let mut item = row![].spacing(10).align_y(Alignment::Center);
    if let Some(thumbnail) = thumbnail {
        item = item.push(thumbnail);
    }
    super::focus_button::wrap_inset(
        id,
        button(
            item.push(labels)
                .push(icons::view(Icon::ArrowRight, p.colors.text_muted, 16.0)),
        )
        .width(Length::Fill)
        .padding([9, 12])
        .style(p.catalog_result(CatalogSurface::Card)),
        (!app.busy() && !app.catalog.confirm_replace).then_some(Message::Catalog(select)),
        p.colors.accent,
        p.colors.background,
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
