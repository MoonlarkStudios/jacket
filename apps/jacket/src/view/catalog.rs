use super::{catalog_search, components, focus_button};
use crate::app::{
    artist_credit_id, edition_id, App, CatalogAction as Action, CatalogPending, Message,
    CATALOG_SCROLL_ID,
};
use crate::icons::{self, Icon};
use crate::style::{self, CatalogSurface};
use iced::widget::{
    button, column, container, responsive, row, rule, scrollable, space, stack, text, text_input,
    tooltip, Text,
};
use iced::{Alignment, Element, Length, Padding};
use jacket_core::SourceLink;
use jacket_desktop::youtube_music::AlbumPage;
use std::time::Duration;

const BROWSE_WIDTH: f32 = 1280.0;
pub(super) const ALBUM_PAGE_WIDTH: f32 = 910.0;

pub(super) fn view(app: &App, compact: bool) -> Element<'_, Message> {
    let p = app.presentation();
    let catalog = &app.catalog;
    if let Some(album) = catalog.album() {
        return album_view(app, album, compact);
    }
    let content_width = BROWSE_WIDTH;
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
    }
    let body: Element<'_, Message> =
        responsive(move |size| catalog_search::view(app, compact, size.width))
            .height(Length::Shrink)
            .into();
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

fn album_view<'a>(app: &'a App, album: &'a AlbumPage, compact: bool) -> Element<'a, Message> {
    let p = app.presentation();
    let edge = if compact { 18 } else { 26 };
    let reviewing = app.reviewing_catalog_album();
    let compact_editor = compact && app.panel == crate::app::Panel::CollectionEditor;
    let heading = if compact_editor {
        text(album_title(app, album))
            .size(14)
            .font(style::semibold())
            .into()
    } else {
        album_header(app, album, compact)
    };
    let header = page_header(
        app,
        heading,
        [
            if compact_editor {
                8
            } else if compact {
                12
            } else {
                16
            },
            edge,
        ],
        ALBUM_PAGE_WIDTH,
    );
    let mut content = column![]
        .spacing(8)
        .height(Length::Fill)
        .width(Length::Fill);
    if !reviewing {
        if app.catalog.edition_shelf().is_some_and(|shelf| shelf.open) {
            content = content.push(
                container(
                    scrollable(other_editions(app))
                        .height(Length::Shrink)
                        .style(p.scrollbar()),
                )
                .max_height(if compact { 130 } else { 180 }),
            );
        }
        if let Some(warning) = album.inspection.warnings.first() {
            content = content.push(text(warning).size(12).color(p.colors.text_muted));
        }
    }
    let tracks = if reviewing {
        crate::track_list::view(app, compact)
    } else {
        crate::track_list::album_preview(app, &album.inspection, compact)
    };
    let body = container(
        container(content.push(tracks))
            .padding([8, edge])
            .max_width(ALBUM_PAGE_WIDTH)
            .width(Length::Fill),
    )
    .center_x(Length::Fill)
    .height(Length::Fill);
    let mut view = column![
        header,
        super::workspace::with_review_tools(app, compact, body.into())
    ]
    .height(Length::Fill);
    if reviewing {
        view = view.push(super::workspace::review_footer(app, compact));
    }
    if app.catalog.error.is_some() || app.catalog.pending.is_some() || app.catalog.confirm_replace {
        view = view.push(rule::horizontal(1).style(p.line())).push(
            container(status(app))
                .padding([10, edge])
                .width(Length::Fill)
                .style(move |_| p.surface(p.colors.panel)),
        );
    }
    view.into()
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
    let review = album_review(app);
    if let Some(year) = review.map_or(album.album_release_year, |review| review.preferences.year) {
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
    let reviewing = app.reviewing_catalog_album();
    let idle =
        app.panel == crate::app::Panel::Workspace && !app.busy() && !app.catalog.confirm_replace;
    let cover_size = if compact { 106.0 } else { 144.0 };
    let cover = if reviewing && (app.cover.is_some() || app.source_cover.is_some()) {
        components::cover(app, cover_size, components::CoverContext::Source)
    } else {
        catalog_search::thumbnail(
            app,
            app.catalog.media.cover(album.artwork_url.as_deref()),
            None,
            cover_size,
        )
    };
    let actions: Element<'_, Message> = if reviewing {
        let mut actions = row![
            focus_button::wrap(
                "collection-edit-album",
                button("Edit shared tags")
                    .padding([7, 10])
                    .style(p.secondary()),
                idle.then_some(Message::OpenEditor(crate::editor::Kind::Album)),
                p.colors.accent,
            ),
            super::cover_controls::edit_button(app),
        ]
        .spacing(7);
        if let Some(review) = &app.review {
            actions = actions.push(focus_button::wrap(
                "collection-tag-mode",
                button(
                    if review.preferences.mode == audio_engine::CollectionMode::Album {
                        "Use individual tags"
                    } else {
                        "Use shared album tags"
                    },
                )
                .padding([7, 10])
                .style(p.quiet()),
                idle.then_some(Message::Mode(
                    if review.preferences.mode == audio_engine::CollectionMode::Album {
                        crate::settings::ModeChoice::Collection
                    } else {
                        crate::settings::ModeChoice::Album
                    },
                )),
                p.colors.accent,
            ));
        }
        if !album.warnings.is_empty() {
            actions = actions.push(
                button("Source notes")
                    .padding([7, 10])
                    .style(p.quiet())
                    .on_press_maybe(
                        idle.then_some(Message::OpenPanel(crate::app::Panel::SourceNotes)),
                    ),
            );
        }
        actions.wrap().vertical_spacing(6).into()
    } else {
        let mut actions = row![
            action(
                app,
                "review-album",
                "Review & queue",
                Action::Review,
                idle,
                true
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
                idle,
                false,
            ),
        ]
        .spacing(8);
        if let Some(status) = app.downloads.status_for(&album.source_url) {
            actions = actions.push(text(status).size(12).color(p.colors.accent));
        }
        actions
            .align_y(Alignment::Center)
            .wrap()
            .vertical_spacing(6)
            .into()
    };
    column![
        navigation(app, "album-back", idle, reviewing),
        row![
            cover,
            column![
                text(album_title(app, page))
                    .size(if compact { 25 } else { 36 })
                    .font(style::semibold()),
                album_artist_credits(app, page),
                text(details.join("  ·  "))
                    .size(13)
                    .color(p.colors.text_muted),
                actions,
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
    if let Some(review) = album_review(app) {
        return text(&review.preferences.album_artist).size(16).into();
    }
    if app.reviewing_catalog_album() {
        return text(
            page.artists
                .iter()
                .map(|artist| artist.name.as_str())
                .collect::<Vec<_>>()
                .join(" & "),
        )
        .size(16)
        .into();
    }
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

fn album_review(app: &App) -> Option<&jacket_core::Review> {
    app.review.as_ref().filter(|review| {
        app.reviewing_catalog_album()
            && review.preferences.mode == audio_engine::CollectionMode::Album
    })
}

fn album_title<'a>(app: &'a App, page: &'a AlbumPage) -> &'a str {
    album_review(app)
        .map(|review| review.preferences.album_title.as_str())
        .filter(|title| !title.trim().is_empty())
        .unwrap_or(&page.inspection.title)
}

pub(super) fn navigation(
    app: &App,
    back_id: &'static str,
    enabled: bool,
    reviewing: bool,
) -> Element<'static, Message> {
    let mut actions = row![action(
        app,
        back_id,
        app.catalog.back_label(),
        Action::Back,
        enabled,
        false
    )];
    if !reviewing && !app.catalog.confirm_replace && (app.review.is_some() || app.result.is_some())
    {
        actions = actions.push(space::horizontal()).push(action(
            app,
            "return-to-review",
            "Back to review",
            Action::ReturnToReview,
            enabled,
            false,
        ));
    }
    actions.into()
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
