//! The export snapshot: effective tags, chosen artwork, and the files to be saved.
use super::components::{self, CoverContext};
use crate::app::{App, Message};
use crate::icons::{self, Icon};
use crate::style::{self, Presentation};
use audio_engine::{ArtworkMode, AudioFormat, ExportPlan, ExportPreview, Metadata, OutputKind};
use iced::widget::{column, container, row, rule, text, Column};
use iced::{Alignment, Element, Length};
use std::collections::HashSet;

const MAX_AUDIO_ROWS: usize = 200;
const MAX_EXTRA_ROWS: usize = 16;

pub fn view(app: &App, compact: bool) -> Element<'_, Message> {
    let (Some(preview), Some(plan)) = (&app.preview, app.preview_plan()) else {
        return text("Select at least one track to review the export.")
            .size(13)
            .into();
    };
    let p = app.presentation();
    let summary = Summary::new(preview, plan);
    let mut heading = column![
        row![
            icons::view(output_icon(plan.output_kind), p.colors.text_muted, 14.0),
            text(format!(
                "{} · {} {}",
                output_label(plan.output_kind),
                preview.tracks.len(),
                if preview.tracks.len() == 1 {
                    "track"
                } else {
                    "tracks"
                }
            ))
            .size(12)
            .color(p.colors.text_muted),
        ]
        .spacing(7)
        .align_y(Alignment::Center),
        text(summary.title.clone())
            .size(if compact { 22 } else { 26 })
            .font(style::semibold())
            .width(Length::Fill),
    ]
    .spacing(7)
    .width(Length::Fill);
    if let Some(artist) = &summary.artist {
        heading = heading.push(text(artist.clone()).size(14).color(p.colors.text_muted));
    }
    if let Some(year) = summary.year {
        heading = heading.push(text(year.to_string()).size(12).color(p.colors.text_muted));
    }
    let artwork_label = match plan.artwork_mode {
        ArtworkMode::Shared => "Shared cover",
        ArtworkMode::PerTrack if preview.tracks.len() == 1 => "Track artwork",
        ArtworkMode::PerTrack => "Artwork from each track",
        ArtworkMode::None => "No artwork",
    };
    let hero: Element<'_, Message> = if matches!(plan.artwork_mode, ArtworkMode::None) {
        heading.into()
    } else {
        row![
            components::cover(
                app,
                if compact { 96.0 } else { 112.0 },
                CoverContext::Export
            ),
            heading,
        ]
        .spacing(18)
        .align_y(Alignment::Center)
        .into()
    };
    let mut body = column![
        hero,
        text(artwork_label).size(12).color(p.colors.text_muted)
    ]
    .spacing(14)
    .width(Length::Fill);
    if plan.output_kind == OutputKind::File {
        if let Some(track) = preview.tracks.first() {
            let metadata = &track.metadata;
            let mut tags = Column::new().spacing(7).width(Length::Fill);
            for (label, value) in [
                ("Album", nonempty(&metadata.album).map(str::to_owned)),
                (
                    "Album artist",
                    nonempty(&metadata.album_artist).map(str::to_owned),
                ),
                (
                    "Track",
                    metadata.track_number.map(|number| number.to_string()),
                ),
                (
                    "Disc",
                    metadata.disc_number.map(|number| number.to_string()),
                ),
            ] {
                if let Some(value) = value {
                    tags = tags.push(tag(label, value, p));
                }
            }
            if !metadata.album.trim().is_empty()
                || !metadata.album_artist.trim().is_empty()
                || metadata.track_number.is_some()
                || metadata.disc_number.is_some()
            {
                body = body.push(
                    container(tags)
                        .padding(12)
                        .width(Length::Fill)
                        .style(move |_| p.panel()),
                );
            }
        }
    }
    body = body.push(rule::horizontal(1).style(p.line()));
    let audio_heading = if plan.output_kind == OutputKind::File {
        "Audio file".to_owned()
    } else {
        format!("{}/", preview.root_name)
    };
    body = body.push(
        row![
            text(audio_heading)
                .size(13)
                .font(style::semibold())
                .width(Length::Fill),
            text("Files to save").size(11).color(p.colors.text_muted)
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    );
    let mut audio = Column::new().width(Length::Fill).spacing(0);
    for track in preview.tracks.iter().take(MAX_AUDIO_ROWS) {
        let (stem, extension) = display_filename(&track.relative_path, track.extension_provisional);
        let mut filename = row![text(stem).size(13).width(Length::Fill)]
            .spacing(10)
            .align_y(Alignment::Center);
        if let Some(extension) = extension {
            filename = filename.push(
                container(text(extension).size(11).color(p.colors.text_muted))
                    .padding([3, 6])
                    .style(move |_| p.surface(p.colors.raised)),
            );
        }
        let mut labels = column![filename].spacing(4).width(Length::Fill);
        if plan.output_kind != OutputKind::File {
            let detail = summary.track_detail(&track.metadata);
            if !detail.is_empty() {
                labels = labels.push(text(detail).size(11).color(p.colors.text_muted));
            }
        }
        let line = row![icons::view(Icon::Music, p.colors.text_muted, 15.0), labels]
            .spacing(10)
            .align_y(Alignment::Center);
        audio = audio.push(container(line).padding([10, 0]).width(Length::Fill));
    }
    body = body.push(audio);
    if preview.tracks.len() > MAX_AUDIO_ROWS {
        body = body.push(
            text(format!(
                "Showing the first {MAX_AUDIO_ROWS} of {} audio files.",
                preview.tracks.len()
            ))
            .size(12)
            .color(p.colors.text_muted),
        );
    }
    let audio_paths = preview
        .tracks
        .iter()
        .map(|track| track.relative_path.as_str())
        .collect::<HashSet<_>>();
    let extras = preview
        .files
        .iter()
        .map(String::as_str)
        .filter(|file| !audio_paths.contains(file))
        .collect::<Vec<_>>();
    if !extras.is_empty() {
        body = body
            .push(rule::horizontal(1).style(p.line()))
            .push(text("Additional files").size(13).font(style::semibold()));
        // Keep portable playback and metadata receipts visible even when a
        // large playlist has thousands of per-track artwork images.
        let mut shown = Vec::new();
        for name in ["cover.jpg", "playlist.m3u", "manifest.json"] {
            if extras.contains(&name) {
                shown.push(name);
            }
        }
        for name in &extras {
            if shown.len() == MAX_EXTRA_ROWS {
                break;
            }
            if !shown.contains(name) {
                shown.push(name);
            }
        }
        for name in &shown {
            body = body.push(
                row![
                    icons::view(Icon::File, p.colors.text_muted, 14.0),
                    text(*name).size(12).color(p.colors.text_muted)
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            );
        }
        if extras.len() > shown.len() {
            body = body.push(
                text(format!(
                    "Showing {} of {} additional files.",
                    shown.len(),
                    extras.len()
                ))
                .size(11)
                .color(p.colors.text_muted),
            );
        }
    }
    let provisional = preview
        .tracks
        .iter()
        .any(|track| track.extension_provisional);
    if provisional {
        let note=match plan.format {
            AudioFormat::Compatible=>"Extensions are confirmed after download: AAC and ALAC use .m4a; other sources use .mp3.",
            AudioFormat::Preserve=>"The source codec determines the extension, confirmed after download.",
            AudioFormat::Mp3=>"Final filenames are confirmed after download.",
        };
        body = body
            .push(rule::horizontal(1).style(p.line()))
            .push(text(note).size(12).color(p.colors.text_muted));
    }
    for warning in preview
        .warnings
        .iter()
        .filter(|warning| !warning.starts_with("Audio extensions"))
    {
        body = body.push(text(warning).size(12).color(p.colors.warning));
    }
    body.into()
}

struct Summary {
    title: String,
    artist: Option<String>,
    album: Option<String>,
    year: Option<u32>,
}

impl Summary {
    fn new(preview: &ExportPreview, plan: &ExportPlan) -> Self {
        let album = shared(
            preview
                .tracks
                .iter()
                .map(|track| track.metadata.album.trim()),
        )
        .and_then(nonempty)
        .map(str::to_owned);
        let artist = if plan.output_kind == OutputKind::File {
            preview
                .tracks
                .first()
                .and_then(|track| nonempty(&track.metadata.artist))
                .map(str::to_owned)
        } else {
            shared(
                preview
                    .tracks
                    .iter()
                    .map(|track| track.metadata.album_artist.trim()),
            )
            .and_then(nonempty)
            .or_else(|| {
                shared(
                    preview
                        .tracks
                        .iter()
                        .map(|track| track.metadata.artist.trim()),
                )
                .and_then(nonempty)
            })
            .map(str::to_owned)
            .or_else(|| {
                let first = preview
                    .tracks
                    .iter()
                    .find_map(|track| nonempty(&track.metadata.artist))?;
                preview
                    .tracks
                    .iter()
                    .filter_map(|track| nonempty(&track.metadata.artist))
                    .any(|artist| artist != first)
                    .then(|| "Various artists".to_owned())
            })
        };
        let title = if plan.output_kind == OutputKind::File {
            preview
                .tracks
                .first()
                .map(|track| track.metadata.title.clone())
                .unwrap_or_else(|| plan.title.clone())
        } else {
            album.clone().unwrap_or_else(|| plan.title.clone())
        };
        let year = shared(
            preview
                .tracks
                .iter()
                .map(|track| track.metadata.release_year),
        )
        .flatten();
        Self {
            title,
            artist,
            album,
            year,
        }
    }

    fn track_detail(&self, metadata: &Metadata) -> String {
        let mut fields = Vec::new();
        if let Some(artist) =
            nonempty(&metadata.artist).filter(|artist| Some(*artist) != self.artist.as_deref())
        {
            fields.push(artist.to_owned());
        }
        if let Some(album) =
            nonempty(&metadata.album).filter(|album| Some(*album) != self.album.as_deref())
        {
            fields.push(album.to_owned());
        }
        if let Some(year) = metadata
            .release_year
            .filter(|year| Some(*year) != self.year)
        {
            fields.push(year.to_string());
        }
        fields.join(" · ")
    }
}

fn tag(label: &'static str, value: String, p: Presentation) -> Element<'static, Message> {
    row![
        text(label).size(12).color(p.colors.text_muted).width(92),
        text(value).size(12).width(Length::Fill)
    ]
    .spacing(12)
    .into()
}

fn shared<T: PartialEq>(mut values: impl Iterator<Item = T>) -> Option<T> {
    let first = values.next()?;
    values.all(|value| value == first).then_some(first)
}

fn nonempty(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

fn display_filename(path: &str, provisional: bool) -> (&str, Option<&'static str>) {
    if provisional {
        if let Some(stem) = path.strip_suffix(".mp3-or-m4a") {
            return (stem, Some(".m4a / .mp3"));
        }
        if let Some(stem) = path.strip_suffix(".audio") {
            return (stem, Some("source format"));
        }
    }
    (path, None)
}

fn output_label(kind: OutputKind) -> &'static str {
    match kind {
        OutputKind::File => "Audio file",
        OutputKind::Folder => "Music folder",
        OutputKind::Zip => "ZIP archive",
    }
}

fn output_icon(kind: OutputKind) -> Icon {
    match kind {
        OutputKind::File => Icon::File,
        OutputKind::Folder => Icon::Folder,
        OutputKind::Zip => Icon::Archive,
    }
}
