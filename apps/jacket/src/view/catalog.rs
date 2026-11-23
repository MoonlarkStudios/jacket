use crate::app::{App, CatalogAction as Action, CatalogPending, Message};
use crate::icons::{self, Icon};
use crate::style;
use iced::widget::{
    button, column, container, image, row, rule, scrollable, space, text, text_input,
};
use iced::{Alignment, ContentFit, Element, Length};
use jacket_core::TrackMatch;
use jacket_desktop::musicbrainz::Release;

pub(super) fn view(app: &App, compact: bool) -> Element<'_, Message> {
    let p = app.presentation();
    let catalog = &app.catalog;
    let enabled = !app.busy();
    let can_search = enabled && app.desktop.is_some() && !catalog.query.trim().is_empty();
    let search = text_input("Album or artist", &catalog.query)
        .id("album-search")
        .on_input_maybe(
            catalog
                .pending
                .is_none()
                .then_some(|query| Message::Catalog(Action::Query(query))),
        )
        .on_submit_maybe(can_search.then_some(Message::Catalog(Action::Search)))
        .size(16)
        .padding([11, 13])
        .style(p.input());
    let top = column![
        row![
            text("Find an album")
                .size(27)
                .font(style::semibold())
                .width(Length::Fill),
            if app.review.is_some() || app.result.is_some() {
                action(
                    app,
                    "return-to-review",
                    "Back to review",
                    Action::ReturnToReview,
                    enabled,
                    false,
                )
            } else {
                action(
                    app,
                    "open-link",
                    "Paste a link",
                    Action::PasteLink,
                    enabled,
                    false,
                )
            },
        ]
        .spacing(10)
        .align_y(Alignment::Center),
        row![
            search,
            action(
                app,
                "search-albums",
                "Search",
                Action::Search,
                can_search,
                true
            )
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    ]
    .spacing(14);
    let body = if catalog.choosing_edition {
        editions(app)
    } else if let Some(release) = &catalog.release {
        release_detail(app, release, compact)
    } else if let Some(group) = &catalog.group {
        let detail = match &catalog.editions {
            Some(editions) if editions.is_empty() => {
                "MusicBrainz has no official editions for this album."
            }
            Some(_) if catalog.pending.is_none() => "Choose an edition to see its tracks.",
            _ => "Looking up editions…",
        };
        let mut content = column![
            toolbar(app, "Back to results", false),
            text(&group.title).size(25).font(style::semibold()),
            text(detail).size(14).color(p.colors.text_muted),
        ]
        .spacing(12);
        if catalog.pending.is_none() {
            if catalog
                .editions
                .as_ref()
                .is_some_and(|editions| !editions.is_empty())
            {
                content = content.push(action(
                    app,
                    "choose-edition",
                    "Choose an edition",
                    Action::Editions,
                    enabled,
                    true,
                ));
            } else if catalog.editions.is_none() {
                content = content.push(action(
                    app,
                    "retry-editions",
                    "Retry",
                    Action::Album(group.clone()),
                    enabled,
                    true,
                ));
            }
        }
        content.into()
    } else {
        results(app)
    };
    let edge = if compact { 18 } else { 32 };
    let mut footer = column![].spacing(9);
    if let Some(error) = &catalog.error {
        footer = footer.push(
            text(error)
                .size(13)
                .color(p.colors.danger)
                .width(Length::Fill),
        );
    }
    if let Some(pending) = catalog.pending {
        let label = if catalog.cancelling {
            "Cancelling…"
        } else {
            match pending {
                CatalogPending::Search { .. } => "Searching MusicBrainz…",
                CatalogPending::Editions => "Looking up editions…",
                CatalogPending::Release => "Loading the track list…",
                CatalogPending::Match => "Checking the source track by track…",
            }
        };
        footer = footer.push(
            row![
                text(label)
                    .size(13)
                    .color(p.colors.text_muted)
                    .width(Length::Fill),
                super::focus_button::wrap(
                    "cancel-catalog",
                    button("Cancel").padding([7, 12]).style(p.secondary()),
                    (!catalog.cancelling).then_some(Message::Cancel),
                    p.colors.accent,
                )
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        );
    }
    if catalog.confirm_replace {
        footer = footer.push(
            text("Replace the current review?")
                .size(14)
                .font(style::semibold()),
        );
        footer = footer.push(
            row![
                action(
                    app,
                    "keep-current-review",
                    "Keep current review",
                    Action::ReturnToReview,
                    enabled,
                    false
                ),
                action(
                    app,
                    "confirm-album-review",
                    "Replace and review",
                    Action::ConfirmReview,
                    enabled,
                    true
                ),
            ]
            .spacing(8),
        );
    } else if catalog.pending.is_none() && !catalog.choosing_edition {
        if let Some(matched) = catalog
            .matched
            .as_ref()
            .filter(|matched| !matched.inspection.tracks.is_empty())
        {
            let count = matched.inspection.tracks.len();
            let label = if matched.tracks.len() == count {
                "Review album".to_owned()
            } else {
                format!("Review {count} matched tracks")
            };
            footer = footer.push(
                row![
                    text(format!(
                        "{count} of {} tracks matched",
                        matched.tracks.len()
                    ))
                    .size(13)
                    .color(p.colors.text_muted),
                    space::horizontal(),
                    action(
                        app,
                        "review-matched-album",
                        label,
                        Action::Review,
                        enabled,
                        true
                    ),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            );
        }
    }
    let show_footer = catalog.error.is_some()
        || catalog.pending.is_some()
        || catalog.confirm_replace
        || (!catalog.choosing_edition
            && catalog
                .matched
                .as_ref()
                .is_some_and(|matched| !matched.inspection.tracks.is_empty()));
    let mut layout = column![].height(Length::Fill);
    if catalog.group.is_none() {
        layout = layout
            .push(
                container(
                    container(top)
                        .padding([20, edge])
                        .max_width(910)
                        .width(Length::Fill),
                )
                .center_x(Length::Fill),
            )
            .push(rule::horizontal(1).style(p.line()));
    }
    layout = layout.push(
        scrollable(
            container(
                container(body)
                    .padding([18, edge])
                    .width(Length::Fill)
                    .max_width(910),
            )
            .center_x(Length::Fill),
        )
        .id("album-browser")
        .height(Length::Fill)
        .width(Length::Fill)
        .style(p.scrollbar())
        .direction(style::scroll_direction()),
    );
    if show_footer {
        layout = layout.push(rule::horizontal(1).style(p.line())).push(
            container(container(footer).width(Length::Fill).max_width(850))
                .padding([10, edge])
                .center_x(Length::Fill)
                .style(move |_| p.surface(p.colors.panel)),
        );
    }
    layout.into()
}

fn results(app: &App) -> Element<'_, Message> {
    let p = app.presentation();
    let enabled = !app.busy();
    let catalog = &app.catalog;
    let Some(albums) = &catalog.results else {
        let description = if catalog.pending.is_some() {
            "Looking for albums…"
        } else {
            "Search by album title or artist name to find editions and available audio."
        };
        let mut start = column![text(description).size(14).color(p.colors.text_muted)].spacing(12);
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
    };
    if albums.is_empty() {
        return column![
            text("No albums found").size(18).font(style::semibold()),
            text("Try a shorter album title or the artist’s name.")
                .size(14)
                .color(p.colors.text_muted),
        ]
        .spacing(6)
        .into();
    }
    let mut body = column![].spacing(7);
    for album in albums {
        let description = [
            album.artist(),
            album
                .first_release_date
                .as_deref()
                .and_then(|date| date.get(..4))
                .unwrap_or("")
                .to_owned(),
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
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
        body = body.push(card(
            app,
            format!("album-{}", album.id),
            album.title.clone(),
            description,
            Action::Album(album.clone()),
        ));
    }
    if catalog.can_search_more() {
        body = body.push(action(
            app,
            "more-albums",
            "Show more albums",
            Action::More,
            enabled,
            false,
        ));
    }
    body.into()
}

fn editions(app: &App) -> Element<'_, Message> {
    let p = app.presentation();
    let mut body = column![
        toolbar(
            app,
            if app.catalog.release.is_some() {
                "Back to album"
            } else {
                "Back to results"
            },
            false
        ),
        text("Choose an edition").size(24).font(style::semibold()),
    ]
    .spacing(9);
    if let Some(editions) = &app.catalog.editions {
        for edition in editions {
            let chosen = app
                .catalog
                .release
                .as_ref()
                .is_some_and(|release| release.edition.id == edition.id);
            let description = if chosen {
                format!("Selected · {}", edition.description())
            } else {
                edition.description()
            };
            body = body.push(card(
                app,
                format!("edition-{}", edition.id),
                edition.display_title(),
                description,
                Action::Edition(edition.id.clone()),
            ));
        }
    }
    body.push(
        text("Editions and track listings from MusicBrainz")
            .size(12)
            .color(p.colors.text_muted),
    )
    .into()
}

fn release_detail<'a>(app: &'a App, release: &'a Release, compact: bool) -> Element<'a, Message> {
    let p = app.presentation();
    let catalog = &app.catalog;
    let enabled = !app.busy();
    let art_size = if compact { 82 } else { 112 };
    let art: Element<'_, Message> = if let Some(cover) = &catalog.cover {
        image(cover.handle(art_size))
            .width(art_size)
            .height(art_size)
            .content_fit(ContentFit::Contain)
            .into()
    } else {
        container(icons::view(Icon::Music, p.colors.text_muted, 28.0))
            .center(art_size)
            .style(move |_| p.panel())
            .into()
    };
    let mut content = column![
        toolbar(app, "Back to results", true),
        row![
            art,
            column![
                text(&release.album.title)
                    .size(if compact { 23 } else { 28 })
                    .font(style::semibold()),
                text(&release.album.artist).size(15),
                text(release.edition.description())
                    .size(13)
                    .color(p.colors.text_muted),
            ]
            .spacing(5)
            .width(Length::Fill)
        ]
        .spacing(14)
        .align_y(Alignment::Center),
        rule::horizontal(1).style(p.line()),
    ]
    .spacing(if compact { 9 } else { 12 });
    let matched = catalog.matched.as_deref();
    let detail = match matched {
        Some(matched) if matched.inspection.tracks.is_empty() => {
            "No tracks matched. Try a different album or playlist.".to_owned()
        }
        Some(matched) if matched.inspection.tracks.len() < matched.tracks.len() => {
            let unresolved: Vec<&str> = release
                .album
                .tracks
                .iter()
                .zip(&matched.tracks)
                .enumerate()
                .filter(|(index, (_, status))| {
                    **status != TrackMatch::Matched
                        && matched.suggested_source_title(*index).is_none()
                })
                .map(|(_, (track, _))| track.title.as_str())
                .take(3)
                .collect();
            match unresolved.as_slice() {
                [] => "Review the suggested tracks below to finish matching the album.".to_owned(),
                [title] => format!("Try another source for {title}."),
                [first, second] => format!("Try another source for {first} and {second}."),
                _ => "Some tracks need another source. See the track list for details.".to_owned(),
            }
        }
        Some(_) => "All tracks matched to the source.".to_owned(),
        None if release.sources.is_empty() => {
            "No YouTube Music album is linked to this edition.".to_owned()
        }
        None => "Check the linked YouTube Music album against this edition.".to_owned(),
    };
    content = content.push(text(detail).size(13).color(p.colors.text_muted));
    if matched.is_none() {
        if let Some(source) = release.sources.first() {
            content = content.push(action(
                app,
                "check-linked-audio",
                "Check YouTube Music album",
                Action::CheckAudio(Some(source.clone())),
                enabled,
                true,
            ));
        }
    }
    let mut other = row![].spacing(8);
    let show_playlist = catalog.use_playlist || release.sources.is_empty();
    if !show_playlist {
        other = other.push(action(
            app,
            "use-album-playlist",
            "Use a playlist link",
            Action::UsePlaylist,
            enabled,
            false,
        ));
    }
    for store in &release.stores {
        other = other.push(action(
            app,
            format!("buy-from-{}", store.provider),
            format!("Buy on {}", store.provider),
            Action::OpenStore(store.url.clone()),
            enabled,
            false,
        ));
    }
    if !show_playlist || !release.stores.is_empty() {
        content = content.push(other);
    }
    if show_playlist {
        let can_check = enabled && !catalog.playlist.trim().is_empty();
        content = content.push(
            row![
                text_input("YouTube album or playlist link", &catalog.playlist)
                    .id("album-playlist")
                    .on_input_maybe(
                        enabled.then_some(|link| Message::Catalog(Action::Playlist(link)))
                    )
                    .on_submit_maybe(
                        can_check.then_some(Message::Catalog(Action::CheckAudio(None)))
                    )
                    .padding([9, 10])
                    .style(p.input()),
                action(
                    app,
                    "check-pasted-playlist",
                    "Check link",
                    Action::CheckAudio(None),
                    can_check,
                    true
                ),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        );
    }
    let multi_disc = release
        .album
        .tracks
        .last()
        .is_some_and(|track| track.disc > 1);
    let mut disc = None;
    for (index, track) in release.album.tracks.iter().enumerate() {
        if multi_disc && disc != Some(track.disc) {
            disc = Some(track.disc);
            content = content.push(
                text(format!("Disc {}", track.disc))
                    .size(14)
                    .font(style::semibold()),
            );
        }
        let time = track.duration_ms.map_or_else(String::new, |ms| {
            let seconds = ms.saturating_add(500) / 1000;
            format!("{}:{:02}", seconds / 60, seconds % 60)
        });
        let mut title = column![text(&track.title).size(14)]
            .spacing(3)
            .width(Length::Fill);
        if !track.artist.is_empty() && track.artist != release.album.artist {
            title = title.push(text(&track.artist).size(12).color(p.colors.text_muted));
        }
        let suggested = matched.and_then(|matched| matched.suggested_source_title(index));
        if let Some(suggested) = suggested {
            title = title.push(
                text(format!("Source title · {suggested}"))
                    .size(12)
                    .color(p.colors.text_muted),
            );
        }
        let mut entry = row![
            text(track.number.to_string())
                .size(12)
                .color(p.colors.text_muted)
                .width(32),
            title,
        ]
        .spacing(8)
        .align_y(Alignment::Center);
        if suggested.is_some() {
            entry = entry.push(action(
                app,
                format!("accept-source-{index}"),
                "Use this track",
                Action::AcceptTrack(index),
                enabled,
                false,
            ));
        } else if let Some(status) = matched.and_then(|matched| matched.tracks.get(index)) {
            let (label, color) = match status {
                TrackMatch::Matched => ("Matched", p.colors.text_muted),
                TrackMatch::Missing => ("Missing", p.colors.danger),
                TrackMatch::Ambiguous => ("Uncertain", p.colors.danger),
            };
            entry = entry.push(text(label).size(12).color(color));
        }
        entry = entry.push(
            text(time)
                .size(12)
                .color(p.colors.text_muted)
                .width(45)
                .align_x(Alignment::End),
        );
        content = content.push(container(entry).padding([5, 0]));
    }
    content.into()
}

fn toolbar<'a>(app: &'a App, back: &'static str, show_editions: bool) -> Element<'a, Message> {
    let enabled = !app.busy();
    let mut bar = row![
        action(app, "album-back", back, Action::Back, enabled, false),
        space::horizontal()
    ]
    .spacing(8);
    if show_editions
        && app
            .catalog
            .editions
            .as_ref()
            .is_some_and(|editions| editions.len() > 1)
    {
        bar = bar.push(action(
            app,
            "change-edition",
            "Change edition",
            Action::Editions,
            enabled,
            false,
        ));
    }
    bar.into()
}

fn card<'a>(
    app: &'a App,
    id: String,
    title: String,
    subtitle: String,
    select: Action,
) -> Element<'a, Message> {
    let p = app.presentation();
    let labels = column![
        text(title).size(15).font(style::semibold()),
        text(subtitle).size(12).color(p.colors.text_muted),
    ]
    .spacing(4)
    .width(Length::Fill);
    super::focus_button::wrap(
        id,
        button(
            row![
                labels,
                icons::view(Icon::ArrowRight, p.colors.text_muted, 16.0)
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .padding([9, 12])
        .style(p.secondary()),
        (!app.busy()).then_some(Message::Catalog(select)),
        p.colors.accent,
    )
}

fn action<'a>(
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
