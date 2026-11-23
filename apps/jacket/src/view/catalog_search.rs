use super::catalog::{action, card};
use crate::app::{search_result_id, App, CatalogAction as Action, CatalogPending, Message};
use crate::icons::{self, Icon};
use crate::style;
use iced::widget::{button, column, container, row, space, text, text_input};
use iced::{Alignment, Element, Length};
use jacket_desktop::musicbrainz::{Album, Artist, SearchHit, SearchKind, Song};

pub(super) fn filters(app: &App, artist: bool) -> Element<'_, Message> {
    let p = app.presentation();
    let selected = app
        .catalog
        .artist
        .as_ref()
        .map_or(app.catalog.filter, |artist| Some(artist.filter));
    let items: &[Option<SearchKind>] = if artist {
        &[Some(SearchKind::Albums), Some(SearchKind::Songs)]
    } else {
        &[
            None,
            Some(SearchKind::Artists),
            Some(SearchKind::Albums),
            Some(SearchKind::Songs),
        ]
    };
    items
        .iter()
        .fold(row![].spacing(7), |bar, &kind| {
            let label = kind.map_or("All", SearchKind::label);
            bar.push(super::focus_button::wrap(
                format!("catalog-filter-{label}"),
                button(label)
                    .padding([6, 12])
                    .style(p.choice(selected == kind)),
                Some(Message::Catalog(Action::Filter(kind))),
                p.colors.accent,
            ))
        })
        .into()
}

pub(super) fn results(app: &App, compact: bool) -> Element<'_, Message> {
    let p = app.presentation();
    let catalog = &app.catalog;
    if catalog.searched.is_empty() {
        let mut start = column![text(
            "Find an artist’s discography, an album edition, or a specific recording."
        )
        .size(14)
        .color(p.colors.text_muted)]
        .spacing(12);
        if app.desktop.is_none() && !app.loading && !app.example {
            start = start.push(super::focus_button::wrap(
                "retry-catalog-startup",
                button("Retry startup")
                    .padding([8, 12])
                    .style(p.secondary()),
                Some(Message::RetryStartup),
                p.colors.accent,
            ));
        }
        return start.into();
    }
    if let Some(kind) = catalog.filter {
        return section(app, kind, None, None);
    }
    let mut body = column![].spacing(20);
    if catalog.query.trim() != catalog.searched {
        body = body.push(
            text(format!("Results for “{}”", catalog.searched))
                .size(13)
                .color(p.colors.text_muted),
        );
    }
    if let Some(error) = catalog.results.errors.get(&SearchKind::Artists) {
        if catalog.results.pages.is_empty()
            && SearchKind::ALL
                .iter()
                .all(|kind| catalog.results.errors.get(kind) == Some(error))
        {
            return body
                .push(text(error).size(14).color(p.colors.danger))
                .push(action(
                    app,
                    "retry-catalog-search",
                    "Retry search",
                    Action::Search,
                    !app.busy(),
                    true,
                ))
                .into();
        }
    }
    if SearchKind::ALL.iter().all(|kind| {
        catalog
            .results
            .pages
            .get(kind)
            .is_some_and(|page| page.hits.is_empty())
    }) {
        return empty(app, "No results found");
    }
    if catalog.pending.is_none()
        && catalog.results.pages.is_empty()
        && catalog.results.errors.is_empty()
    {
        return empty(app, "Search again to see results");
    }
    let top = catalog.top_result();
    if let Some(hit) = top {
        body = body.push(top_result(app, hit));
    }
    for kind in catalog.result_order() {
        let has_results = catalog.results.pages.get(&kind).is_some_and(|page| {
            page.hits
                .iter()
                .any(|hit| top.is_none_or(|best| best.id() != hit.id()))
                || page.next_offset.is_some()
        });
        if has_results || catalog.results.errors.contains_key(&kind) {
            body = body.push(section(
                app,
                kind,
                Some(if compact { 2 } else { 4 }),
                top.map(SearchHit::id),
            ));
        }
    }
    body.into()
}

pub(super) fn artist<'a>(app: &'a App, artist: &'a Artist) -> Element<'a, Message> {
    let p = app.presentation();
    let kind = app
        .catalog
        .artist
        .as_ref()
        .map_or(SearchKind::Albums, |page| page.filter);
    column![
        back(app, "Back to results"),
        text(&artist.name).size(28).font(style::semibold()),
        text(artist_description(artist))
            .size(14)
            .color(p.colors.text_muted),
        filters(app, true),
        section(app, kind, None, None),
    ]
    .spacing(14)
    .into()
}

pub(super) fn song(app: &App) -> Element<'_, Message> {
    let p = app.presentation();
    let enabled = !app.busy();
    let Some(page) = &app.catalog.song else {
        return space().into();
    };
    let song = page
        .details
        .as_ref()
        .map_or(&page.selected, |details| &details.song);
    let mut body = column![
        back(
            app,
            if app.catalog.artist.is_some() {
                "Back to artist"
            } else {
                "Back to results"
            }
        ),
        text(&song.title).size(28).font(style::semibold()),
        text(song_description(song))
            .size(14)
            .color(p.colors.text_muted),
    ]
    .spacing(12)
    .width(Length::Fill);
    let Some(details) = &page.details else {
        if app.catalog.pending.is_none() {
            body = body.push(action(
                app,
                "retry-song",
                "Retry lookup",
                Action::Song(page.selected.clone()),
                enabled,
                true,
            ));
        }
        return body.into();
    };
    if let Some(link) = details.sources.first().filter(|_| page.matched.is_none()) {
        body = body.push(action(
            app,
            "check-linked-song",
            "Check linked YouTube audio",
            Action::CheckSong(Some(link.clone())),
            enabled,
            true,
        ));
    }
    if page.show_link || details.sources.is_empty() {
        let can_check = enabled && !page.url.trim().is_empty();
        body = body.push(
            row![
                text_input("YouTube song link", &page.url)
                    .id("song-source-link")
                    .on_input_maybe(enabled.then_some(|url| Message::Catalog(Action::SongUrl(url))))
                    .on_submit_maybe(can_check.then_some(Message::Catalog(Action::CheckSong(None))))
                    .padding([9, 10])
                    .style(p.input()),
                action(
                    app,
                    "check-pasted-song",
                    "Check link",
                    Action::CheckSong(None),
                    can_check,
                    true
                ),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        );
    } else {
        body = body.push(action(
            app,
            "use-song-link",
            "Use a different link",
            Action::SongLink,
            enabled,
            false,
        ));
    }
    if !details.albums.is_empty() {
        body = body.push(text("Appears on").size(16).font(style::semibold()));
        for album in &details.albums {
            body = body.push(album_card(app, album));
        }
    }
    body.into()
}

fn section<'a>(
    app: &'a App,
    kind: SearchKind,
    preview: Option<usize>,
    exclude: Option<&str>,
) -> Element<'a, Message> {
    let p = app.presentation();
    let enabled = !app.busy();
    let section = app.catalog.sections();
    let page = section.pages.get(&kind);
    let mut body = column![].spacing(7);
    if let Some(error) = section.errors.get(&kind) {
        body = body.push(
            row![
                text(format!("{} · {error}", kind.label()))
                    .size(13)
                    .color(p.colors.danger)
                    .width(Length::Fill),
                action(
                    app,
                    format!("retry-{}", kind.label()),
                    "Retry",
                    Action::Retry(kind),
                    enabled,
                    false
                ),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        );
    }
    let Some(page) = page else { return body.into() };
    if page.hits.is_empty() {
        if preview.is_none() {
            return empty(
                app,
                match kind {
                    SearchKind::Artists => "No artists found",
                    SearchKind::Albums => "No albums found",
                    SearchKind::Songs => "No songs found",
                },
            );
        }
        return body.into();
    }
    if let Some(limit) = preview {
        let mut heading = row![
            text(kind.label()).size(17).font(style::semibold()),
            space::horizontal()
        ]
        .align_y(Alignment::Center);
        if page
            .hits
            .iter()
            .filter(|hit| exclude != Some(hit.id()))
            .count()
            > limit
            || page.next_offset.is_some()
        {
            heading = heading.push(action(
                app,
                format!("see-all-{}", kind.label()),
                "See all",
                Action::Filter(Some(kind)),
                true,
                false,
            ));
        }
        body = body.push(heading);
    } else if app.catalog.search_home() && app.catalog.query.trim() != app.catalog.searched {
        body = body.push(
            text(format!("Results for “{}”", app.catalog.searched))
                .size(13)
                .color(p.colors.text_muted),
        );
    }
    for hit in page
        .hits
        .iter()
        .filter(|hit| exclude != Some(hit.id()))
        .take(preview.unwrap_or(page.hits.len()))
    {
        let (_, title, description, select) = hit_display(hit);
        body = body.push(card(app, search_result_id(hit), title, description, select));
    }
    if preview.is_none() && page.next_offset.is_some() && !section.errors.contains_key(&kind) {
        body = body.push(action(
            app,
            format!("more-{}", kind.label()),
            "Show more",
            Action::More(kind),
            enabled,
            false,
        ));
    }
    body.into()
}

fn top_result<'a>(app: &'a App, hit: &SearchHit) -> Element<'a, Message> {
    let p = app.presentation();
    let (kind, title, description, action) = hit_display(hit);
    let subtitle = if description.is_empty() {
        kind.to_owned()
    } else {
        format!("{kind} · {description}")
    };
    let tile = row![
        container(icons::view(Icon::Music, p.colors.accent, 25.0))
            .center(48)
            .style(move |_| p.panel()),
        column![
            text(title).size(21).font(style::semibold()),
            text(subtitle).size(13).color(p.colors.text_muted),
        ]
        .spacing(4)
        .width(Length::Fill),
        icons::view(Icon::ArrowRight, p.colors.text_muted, 17.0),
    ]
    .spacing(12)
    .align_y(Alignment::Center);
    let enabled = !app.busy()
        || (matches!(app.catalog.pending, Some(CatalogPending::Search)) && !app.catalog.cancelling);
    column![
        text("Top result").size(17).font(style::semibold()),
        super::focus_button::wrap(
            "catalog-top-result",
            button(tile)
                .width(Length::Fill)
                .padding([11, 13])
                .style(p.secondary()),
            enabled.then_some(Message::Catalog(action)),
            p.colors.accent,
        ),
    ]
    .spacing(8)
    .into()
}

fn hit_display(hit: &SearchHit) -> (&'static str, String, String, Action) {
    match hit {
        SearchHit::Artist(artist) => (
            "Artist",
            artist.name.clone(),
            artist_description(artist),
            Action::Artist(artist.clone()),
        ),
        SearchHit::Album(album) => (
            "Album",
            album.title.clone(),
            album_description(album),
            Action::Album(album.clone()),
        ),
        SearchHit::Song(song) => (
            "Song",
            song.title.clone(),
            song_description(song),
            Action::Song(song.clone()),
        ),
    }
}

fn empty<'a>(app: &App, label: &'static str) -> Element<'a, Message> {
    let p = app.presentation();
    let mut body = column![text(label).size(18).font(style::semibold())].spacing(6);
    if app.catalog.search_home() {
        body = body.push(
            text("Try a shorter name or include the artist.")
                .size(14)
                .color(p.colors.text_muted),
        );
    }
    body.into()
}

fn back<'a>(app: &App, label: &'static str) -> Element<'a, Message> {
    action(
        app,
        "catalog-search-back",
        label,
        Action::Back,
        !app.busy(),
        false,
    )
}

fn album_card<'a>(app: &'a App, album: &'a Album) -> Element<'a, Message> {
    card(
        app,
        format!("album-{}", album.id),
        album.title.clone(),
        album_description(album),
        Action::Album(album.clone()),
    )
}

fn album_description(album: &Album) -> String {
    [
        album.artist(),
        year(album.first_release_date.as_deref()),
        album
            .disambiguation
            .clone()
            .filter(|note| !note.is_empty())
            .unwrap_or_else(|| {
                if album.secondary_types.is_empty() {
                    album.primary_type.clone().unwrap_or_default()
                } else {
                    album.secondary_types.join(", ")
                }
            }),
    ]
    .into_iter()
    .filter(|field| !field.is_empty())
    .collect::<Vec<_>>()
    .join(" · ")
}

fn artist_description(artist: &Artist) -> String {
    [
        artist.disambiguation.as_deref(),
        artist.kind.as_deref(),
        artist.location(),
    ]
    .into_iter()
    .flatten()
    .filter(|field| !field.is_empty())
    .collect::<Vec<_>>()
    .join(" · ")
}

fn song_description(song: &Song) -> String {
    let time = song.length.map_or_else(String::new, |ms| {
        let seconds = ms.saturating_add(500) / 1000;
        format!("{}:{:02}", seconds / 60, seconds % 60)
    });
    [
        song.artist(),
        song.disambiguation.clone().unwrap_or_default(),
        song.album_hint().unwrap_or("").to_owned(),
        year(song.first_release_date.as_deref()),
        time,
    ]
    .into_iter()
    .filter(|field| !field.is_empty())
    .collect::<Vec<_>>()
    .join(" · ")
}

fn year(date: Option<&str>) -> String {
    date.and_then(|date| date.get(..4)).unwrap_or("").to_owned()
}
