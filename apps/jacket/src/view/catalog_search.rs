use super::catalog::{action, card};
use super::catalog_grid::{self, AlbumGridSize};
use super::{catalog_song_menu, components};
use crate::app::{result_id, App, CatalogAction as Action, Message, Thumbnail};
use crate::icons::{self, Icon};
use crate::style::{self, CatalogSurface};
use iced::widget::{button, column, container, image, row, rule, sensor, space, text};
use iced::{border, Alignment, ContentFit, Element, Length, Padding};
use jacket_desktop::catalog::SearchKind;
use jacket_desktop::youtube_music::{Album, SearchHit, SearchItem as YouTubeItem, Song};

const CARD_BORDER_INSET: f32 = 1.0;
const CARD_TEXT_INSET: f32 = 8.0;
const ARTIST_TILE_WIDTH: f32 = 104.0;
const ARTIST_TILE_SPACING: f32 = 10.0;

pub(super) fn filters(app: &App) -> Element<'_, Message> {
    let p = app.presentation();
    std::iter::once(None)
        .chain(SearchKind::ALL.into_iter().map(Some))
        .fold(row![].spacing(7), |bar, filter| {
            let label = filter.map_or("All", SearchKind::label);
            bar.push(super::focus_button::wrap(
                format!("catalog-filter-{label}"),
                button(label)
                    .padding([6, 12])
                    .style(p.choice(app.catalog.filter == filter)),
                (!app.busy() && !app.catalog.confirm_replace)
                    .then_some(Message::Catalog(Action::Filter(filter))),
                p.colors.accent,
            ))
        })
        .into()
}

pub(super) fn view(app: &App, compact: bool, width: f32) -> Element<'_, Message> {
    let p = app.presentation();
    let catalog = &app.catalog;
    let artist_page = catalog.artist();
    let has_more_releases = artist_page.is_some_and(|page| page.has_more_releases);
    if artist_page.is_none() {
        if catalog.query.trim().is_empty() {
            return text("Search YouTube Music to find a release or recording.")
                .size(15)
                .color(p.colors.text_muted)
                .into();
        }
        if catalog.query.trim() != catalog.searched {
            return space().into();
        }
    }

    let compact_results = compact && artist_page.is_none();
    let section_spacing = if compact_results { 16 } else { 22 };
    let heading_spacing = if compact_results { 8 } else { 9 };
    let mut body = column![].spacing(section_spacing);
    if let Some(page) = artist_page {
        body = body
            .push(super::catalog::navigation(
                app,
                "artist-back",
                !app.busy() && !catalog.confirm_replace,
                false,
            ))
            .push(
                row![
                    thumbnail(
                        app,
                        catalog.media.portrait(page.artist.artwork_url.as_deref()),
                        Some(&page.artist.name),
                        if compact { 66.0 } else { 90.0 },
                    ),
                    text(&page.artist.name)
                        .size(if compact { 27 } else { 36 })
                        .font(style::semibold()),
                ]
                .spacing(16)
                .align_y(Alignment::Center),
            );
    }

    let hits = catalog.hits();
    if hits.is_empty() && !has_more_releases {
        if catalog.pending.is_none() && catalog.error.is_none() {
            let label = if artist_page.is_some() {
                "No releases or songs found"
            } else {
                match catalog.filter {
                    Some(SearchKind::Artists) => "No artists found",
                    Some(SearchKind::Albums) => "No albums found",
                    Some(SearchKind::Songs) => "No songs found",
                    None => "No results found",
                }
            };
            body = body.push(text(label).size(18).font(style::semibold()));
            if artist_page.is_none() {
                body = body.push(
                    text("Try a shorter name or include the artist.")
                        .size(14)
                        .color(p.colors.text_muted),
                );
            }
        }
        return body.into();
    }

    let album_grid = AlbumGridSize::for_width(width);
    if let (None, Some(kind)) = (artist_page, catalog.filter) {
        return result_group(app, kind, hits.iter(), album_grid);
    }

    let featured = catalog.featured();
    if let Some(hit) = featured {
        body = body.push(
            column![
                text("Top result").size(18).font(style::semibold()),
                result_row(app, &hit.item, Some(if compact { 72.0 } else { 88.0 })),
            ]
            .spacing(heading_spacing),
        );
    }
    let order = if artist_page.is_some() {
        [SearchKind::Albums, SearchKind::Songs, SearchKind::Artists]
    } else {
        catalog.result_order()
    };
    for kind in order {
        let group = hits
            .iter()
            .filter(|hit| {
                hit.item.kind() == kind
                    && featured.is_none_or(|featured| featured.item.id() != hit.item.id())
            })
            .collect::<Vec<_>>();
        let artist_releases = artist_page.is_some() && kind == SearchKind::Albums;
        if group.is_empty() && !(artist_releases && has_more_releases) {
            continue;
        }
        let label = match (artist_page.is_some(), kind) {
            (true, SearchKind::Albums) => "Albums & singles",
            (true, SearchKind::Songs) => "Top songs",
            _ => kind.label(),
        };
        let mut heading = row![
            text(label).size(18).font(style::semibold()),
            space::horizontal()
        ]
        .align_y(Alignment::Center);
        if artist_page.is_none() {
            heading = heading.push(action(
                app,
                format!("catalog-more-{}", kind.label()),
                "See all",
                Action::Filter(Some(kind)),
                !app.busy() && !catalog.confirm_replace,
                false,
            ));
        }
        if artist_releases && (has_more_releases || group.len() > album_grid.columns) {
            heading = heading.push(action(
                app,
                "artist-releases",
                if catalog.show_all_artist_releases() {
                    "Show fewer"
                } else {
                    "Show all"
                },
                Action::ToggleArtistReleases,
                !app.busy() && !catalog.confirm_replace,
                false,
            ));
        }
        let results = if artist_page.is_none() && kind == SearchKind::Artists {
            artist_preview(app, group.into_iter(), width)
        } else {
            let limit = match (artist_page.is_some(), kind) {
                (true, SearchKind::Albums) if !catalog.show_all_artist_releases() => {
                    album_grid.columns
                }
                (true, _) => group.len(),
                (false, SearchKind::Albums) => album_grid.columns,
                (false, _) if compact => 2,
                (false, _) => 3,
            };
            result_group(app, kind, group.into_iter().take(limit), album_grid)
        };
        body = body.push(column![heading, results].spacing(heading_spacing));
    }
    body.into()
}

fn artist_preview<'a>(
    app: &'a App,
    hits: impl Iterator<Item = &'a SearchHit>,
    width: f32,
) -> Element<'a, Message> {
    let p = app.presentation();
    let capacity =
        ((width + ARTIST_TILE_SPACING) / (ARTIST_TILE_WIDTH + ARTIST_TILE_SPACING)) as usize;
    let artists = hits.filter_map(|hit| match &hit.item {
        YouTubeItem::Artist(artist) => Some(artist),
        _ => None,
    });
    row(artists.take(capacity.max(1)).map(|artist| {
        let portrait = app.catalog.media.portrait(artist.artwork_url.as_deref());
        let tile = column![
            thumbnail(app, portrait, Some(&artist.name), 76.0),
            text(&artist.name)
                .size(13)
                .width(Length::Fill)
                .align_x(Alignment::Center),
        ]
        .spacing(7)
        .align_x(Alignment::Center)
        .width(Length::Fill);
        super::focus_button::wrap_inset(
            result_id(SearchKind::Artists, &artist.id),
            button(tile)
                .width(ARTIST_TILE_WIDTH)
                .padding(6)
                .style(p.catalog_result(CatalogSurface::Flat)),
            (!app.busy() && !app.catalog.confirm_replace).then(|| {
                Message::Catalog(Action::OpenResult(SearchKind::Artists, artist.id.clone()))
            }),
            p.colors.accent,
            p.colors.background,
        )
    }))
    .spacing(ARTIST_TILE_SPACING)
    .into()
}

fn result_group<'a>(
    app: &'a App,
    kind: SearchKind,
    hits: impl Iterator<Item = &'a SearchHit>,
    album_grid: AlbumGridSize,
) -> Element<'a, Message> {
    match kind {
        SearchKind::Albums => {
            let cards = hits.filter_map(|hit| match &hit.item {
                YouTubeItem::Album(album) => Some(album_card(app, album, album_grid.card_width)),
                _ => None,
            });
            catalog_grid::albums(cards, album_grid.columns)
        }
        SearchKind::Songs => song_list(app, hits),
        SearchKind::Artists => column(hits.map(|hit| result_row(app, &hit.item, None)))
            .spacing(8)
            .into(),
    }
}

fn song_list<'a>(app: &'a App, hits: impl Iterator<Item = &'a SearchHit>) -> Element<'a, Message> {
    let p = app.presentation();
    let songs = hits.filter_map(|hit| match &hit.item {
        YouTubeItem::Song(song) => Some(song),
        _ => None,
    });
    let mut list = column![].width(Length::Fill);
    for (index, song) in songs.enumerate() {
        if index > 0 {
            list = list.push(rule::horizontal(1).style(p.line()));
        }
        list = list.push(song_row(app, song, None));
    }
    list.into()
}

fn song_row<'a>(
    app: &'a App,
    song: &'a Song,
    featured_artwork_edge: Option<f32>,
) -> Element<'a, Message> {
    let p = app.presentation();
    let featured = featured_artwork_edge.is_some();
    let cover = app.catalog.media.cover(song.artwork_url.as_deref());
    let subtitle = [
        if featured { "Song" } else { "" },
        song.artist.as_str(),
        song.album.as_str(),
    ]
    .into_iter()
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>()
    .join(" · ");
    let mut labels = column![text(&song.title)
        .size(if featured { 20 } else { 15 })
        .font(style::semibold())]
    .spacing(4)
    .width(Length::Fill);
    if !subtitle.is_empty() {
        labels = labels.push(text(subtitle).size(12).color(p.colors.text_muted));
    }
    let duration = song
        .duration
        .map_or_else(String::new, |seconds| components::duration(seconds as f64));
    let content = row![
        thumbnail(app, cover, None, featured_artwork_edge.unwrap_or(40.0)),
        labels,
        text(duration)
            .size(12)
            .color(p.colors.text_muted)
            .align_x(Alignment::End)
            .wrapping(text::Wrapping::None),
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    let surface = if featured {
        CatalogSurface::Card
    } else {
        CatalogSurface::Flat
    };
    let select_song = super::focus_button::wrap_inset(
        result_id(SearchKind::Songs, &song.id),
        button(content)
            .padding([8, 10])
            .width(Length::Fill)
            .style(p.catalog_result(surface)),
        (!app.busy() && !app.catalog.confirm_replace)
            .then(|| Message::Catalog(Action::OpenResult(SearchKind::Songs, song.id.clone()))),
        p.colors.accent,
        p.colors.background,
    );
    row![select_song, catalog_song_menu::view(app, &song.id)]
        .spacing(8)
        .align_y(Alignment::Center)
        .into()
}

fn album_card<'a>(app: &'a App, album: &'a Album, card_width: f32) -> Element<'a, Message> {
    let p = app.presentation();
    let cover = app.catalog.media.cover(album.artwork_url.as_deref());
    let detail = album.year.map_or_else(
        || album.kind.clone(),
        |year| format!("{year} · {}", album.kind),
    );
    let cover_edge = (card_width - 2.0 * CARD_BORDER_INSET).max(0.0);
    let body = column![
        thumbnail_with_radius(app, cover, None, cover_edge, border::top(4.0)),
        container(
            column![
                text(&album.title).size(14).font(style::semibold()),
                text(&album.artist).size(12).color(p.colors.text_muted),
            ]
            .spacing(4),
        )
        .padding([0.0, CARD_TEXT_INSET])
        .width(Length::Fill),
    ]
    .spacing(8)
    .width(Length::Fill);
    let footer = container(text(detail).size(12).color(p.colors.text_muted)).padding(Padding {
        left: CARD_TEXT_INSET,
        right: CARD_TEXT_INSET,
        bottom: CARD_TEXT_INSET,
        ..Padding::ZERO
    });
    // The album grid stretches this column and positions its footer.
    let content = column![body, footer].spacing(4).width(Length::Fill);
    let card = super::focus_button::wrap_inset(
        result_id(SearchKind::Albums, &album.id),
        button(content)
            .width(Length::Fill)
            .padding(CARD_BORDER_INSET)
            .style(p.catalog_result(CatalogSurface::Card)),
        (!app.busy() && !app.catalog.confirm_replace)
            .then(|| Message::Catalog(Action::OpenResult(SearchKind::Albums, album.id.clone()))),
        p.colors.accent,
        p.colors.background,
    );
    if let Some(url) = album
        .artwork_url
        .as_ref()
        .filter(|_| app.catalog.show_all_artist_releases())
    {
        sensor(card)
            .key_ref(&album.id)
            .anticipate(180)
            .on_show(|_| {
                Message::Catalog(Action::ArtistReleaseArtwork {
                    id: album.id.clone(),
                    url: Some(url.clone()),
                })
            })
            .on_hide(Message::Catalog(Action::ArtistReleaseArtwork {
                id: album.id.clone(),
                url: None,
            }))
            .into()
    } else {
        card
    }
}

fn result_row<'a>(
    app: &'a App,
    item: &'a YouTubeItem,
    featured_artwork_edge: Option<f32>,
) -> Element<'a, Message> {
    let featured = featured_artwork_edge.is_some();
    let (title, details, artist) = match item {
        YouTubeItem::Artist(artist) => (
            &artist.name,
            ["Artist".to_owned(), String::new(), String::new()],
            Some(artist.name.as_str()),
        ),
        YouTubeItem::Album(album) => (
            &album.title,
            [
                album.artist.clone(),
                album.year.map_or_else(String::new, |year| year.to_string()),
                album.kind.clone(),
            ],
            None,
        ),
        YouTubeItem::Song(song) => return song_row(app, song, featured_artwork_edge),
    };
    let details = details
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
    let artwork = if artist.is_some() {
        app.catalog.media.portrait(item.artwork_url())
    } else {
        app.catalog.media.cover(item.artwork_url())
    };
    card(
        app,
        result_id(item.kind(), item.id()),
        text(title).size(if featured { 20 } else { 15 }),
        details,
        Some(thumbnail(
            app,
            artwork,
            artist,
            featured_artwork_edge.unwrap_or(40.0),
        )),
        Action::OpenResult(item.kind(), item.id().into()),
    )
}

pub(super) fn thumbnail(
    app: &App,
    picture: Thumbnail<'_>,
    artist: Option<&str>,
    edge: f32,
) -> Element<'static, Message> {
    let radius = if artist.is_some() { edge / 2.0 } else { 5.0 };
    thumbnail_with_radius(app, picture, artist, edge, border::radius(radius))
}

fn thumbnail_with_radius(
    app: &App,
    picture: Thumbnail<'_>,
    artist: Option<&str>,
    edge: f32,
    radius: border::Radius,
) -> Element<'static, Message> {
    let p = app.presentation();
    let content: Element<'static, Message> = match picture {
        Thumbnail::Ready(handle) => {
            let picture = image(handle.clone())
                .width(Length::Fill)
                .height(Length::Fill)
                .content_fit(ContentFit::Cover);
            if artist.is_some() {
                picture.into()
            } else {
                picture.border_radius(radius).into()
            }
        }
        Thumbnail::Loading => space().into(),
        Thumbnail::Missing => {
            match artist.and_then(|name| name.chars().find(|c| c.is_alphanumeric())) {
                Some(letter) => text(letter.to_uppercase().collect::<String>())
                    .size(edge * 0.44)
                    .font(style::semibold())
                    .color(p.colors.accent)
                    .into(),
                None => icons::view(Icon::Music, p.colors.text_muted, edge * 0.40),
            }
        }
    };
    container(content)
        .center(edge)
        .style(move |_| {
            let mut surface = p.surface(p.colors.raised);
            surface.border.radius = radius;
            surface
        })
        .into()
}
