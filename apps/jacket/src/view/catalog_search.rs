use super::catalog::{action, card};
use super::components;
use crate::app::{result_id, song_album_id, App, CatalogAction as Action, Message};
use crate::icons::{self, Icon};
use crate::style;
use iced::widget::{button, column, container, grid, image, row, space, text};
use iced::{Alignment, ContentFit, Element, Length};
use jacket_desktop::catalog::SearchKind;
use jacket_desktop::youtube_music::{Album, SearchHit, SearchItem as YouTubeItem};

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

pub(super) fn view(app: &App, compact: bool) -> Element<'_, Message> {
    let p = app.presentation();
    let catalog = &app.catalog;
    let artist_page = catalog.artist();
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

    let mut body = column![].spacing(22);
    if let Some(page) = artist_page {
        body = body
            .push(action(
                app,
                "artist-back",
                catalog.back_label(),
                Action::Back,
                !app.busy() && !catalog.confirm_replace,
                false,
            ))
            .push(
                row![
                    thumbnail(
                        app,
                        page.artist
                            .artwork_url
                            .as_deref()
                            .and_then(|url| catalog.media.get(url)),
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
    if hits.is_empty() {
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

    if let (None, Some(kind)) = (artist_page, catalog.filter) {
        return result_group(app, kind, hits.iter(), compact);
    }

    let featured = catalog.featured();
    if let Some(hit) = featured {
        body = body.push(
            column![
                text("Top result").size(17).font(style::semibold()),
                result_row(app, &hit.item, true),
            ]
            .spacing(8),
        );
    }
    let order = if artist_page.is_some() {
        [SearchKind::Albums, SearchKind::Songs, SearchKind::Artists]
    } else {
        catalog.result_order()
    };
    let album_columns = if compact { 3 } else { 4 };
    for kind in order {
        let group = hits
            .iter()
            .filter(|hit| {
                hit.item.kind() == kind
                    && featured.is_none_or(|featured| featured.item.id() != hit.item.id())
            })
            .collect::<Vec<_>>();
        if group.is_empty() {
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
        if artist_page.is_some() && kind == SearchKind::Albums && group.len() > album_columns {
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
        let limit = match (artist_page.is_some(), kind) {
            (true, SearchKind::Albums) if !catalog.show_all_artist_releases() => album_columns,
            (true, _) => group.len(),
            (false, SearchKind::Albums) => album_columns,
            (false, _) if compact => 2,
            (false, _) => 3,
        };
        body = body.push(
            column![
                heading,
                result_group(app, kind, group.into_iter().take(limit), compact)
            ]
            .spacing(9),
        );
    }
    body.into()
}

fn result_group<'a>(
    app: &'a App,
    kind: SearchKind,
    hits: impl Iterator<Item = &'a SearchHit>,
    compact: bool,
) -> Element<'a, Message> {
    if kind == SearchKind::Albums {
        grid(hits.filter_map(|hit| match &hit.item {
            YouTubeItem::Album(album) => Some(album_card(app, album, compact)),
            _ => None,
        }))
        .columns(if compact { 3 } else { 4 })
        .height(Length::Shrink)
        .spacing(10)
        .into()
    } else {
        column(hits.map(|hit| result_row(app, &hit.item, false)))
            .spacing(8)
            .into()
    }
}

fn album_card<'a>(app: &'a App, album: &'a Album, compact: bool) -> Element<'a, Message> {
    let p = app.presentation();
    let cover = album
        .artwork_url
        .as_deref()
        .and_then(|url| app.catalog.media.get(url));
    let detail = album.year.map_or_else(
        || album.kind.clone(),
        |year| format!("{year} · {}", album.kind),
    );
    let content = column![
        container(thumbnail(
            app,
            cover,
            None,
            if compact { 124.0 } else { 164.0 }
        ))
        .center_x(Length::Fill),
        column![
            text(&album.title).size(14).font(style::semibold()),
            text(&album.artist).size(12).color(p.colors.text_muted),
            text(detail).size(12).color(p.colors.text_muted),
        ]
        .spacing(4)
        .width(Length::Fill),
    ]
    .spacing(8)
    .width(Length::Fill);
    super::focus_button::wrap(
        result_id(SearchKind::Albums, &album.id),
        button(content)
            .width(Length::Fill)
            .padding(9)
            .style(p.secondary()),
        (!app.busy() && !app.catalog.confirm_replace)
            .then(|| Message::Catalog(Action::OpenResult(SearchKind::Albums, album.id.clone()))),
        p.colors.accent,
    )
}

fn result_row<'a>(app: &'a App, item: &YouTubeItem, featured: bool) -> Element<'a, Message> {
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
        YouTubeItem::Song(song) => (
            &song.title,
            [
                song.artist.clone(),
                song.album.clone(),
                song.duration
                    .map_or_else(String::new, |seconds| components::duration(seconds as f64)),
            ],
            None,
        ),
    };
    let details = details
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
    let subtitle = if featured && matches!(item, YouTubeItem::Song(_)) {
        format!("Song · {details}")
    } else {
        details
    };
    let result = card(
        app,
        result_id(item.kind(), item.id()),
        title.clone(),
        subtitle,
        Some(thumbnail(
            app,
            item.artwork_url()
                .and_then(|url| app.catalog.media.get(url)),
            artist,
            if featured { 56.0 } else { 40.0 },
        )),
        Action::OpenResult(item.kind(), item.id().into()),
    );
    if let YouTubeItem::Song(song) = item {
        row![
            result,
            action(
                app,
                song_album_id(&song.id),
                "View album",
                Action::OpenSongAlbum(song.id.clone()),
                !app.busy() && !app.catalog.confirm_replace,
                false,
            ),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into()
    } else {
        result
    }
}

pub(super) fn thumbnail(
    app: &App,
    picture: Option<&image::Handle>,
    artist: Option<&str>,
    edge: f32,
) -> Element<'static, Message> {
    let p = app.presentation();
    let radius = if artist.is_some() { edge / 2.0 } else { 5.0 };
    if let Some(handle) = picture {
        return image(handle.clone())
            .width(edge)
            .height(edge)
            .content_fit(ContentFit::Cover)
            .border_radius(radius)
            .into();
    }
    let initial = artist.and_then(|name| name.chars().find(|c| c.is_alphanumeric()));
    let placeholder = match initial {
        Some(letter) => text(letter.to_uppercase().collect::<String>())
            .size(edge * 0.44)
            .font(style::semibold())
            .color(p.colors.accent)
            .into(),
        None => icons::view(Icon::Music, p.colors.accent, edge * 0.46),
    };
    container(placeholder)
        .center(edge)
        .style(move |_| {
            let mut surface = p.panel();
            surface.border.radius = radius.into();
            surface
        })
        .into()
}
