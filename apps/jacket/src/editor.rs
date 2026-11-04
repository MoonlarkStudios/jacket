use crate::app::Message;
use crate::settings::EditAction;
use audio_engine::{CollectionMode, Metadata};
use iced::widget::{column, container, pick_list, row, text, text_input, Column};
use iced::{Element, Length};
use jacket_core::{BulkEdits, Review, TitleCleanup};

#[derive(Clone, Copy, Debug)]
pub enum Kind {
    Track(usize),
    Album,
    OutputName,
    Bulk,
}

#[derive(Clone, Copy, Debug)]
pub enum Field {
    Title,
    Artist,
    Album,
    AlbumArtist,
    Year,
    Track,
    Disc,
    Prefix,
    Suffix,
}

#[derive(Clone, Copy, Debug)]
pub enum BulkField {
    Artist,
    Album,
    AlbumArtist,
    Year,
}

#[derive(Clone, Debug)]
pub enum Action {
    Input(Field, String),
    Choose(BulkField, EditAction),
    Apply,
}

#[derive(Clone, Debug)]
pub struct Editor {
    pub kind: Kind,
    title: String,
    artist: String,
    album: String,
    album_artist: String,
    year: String,
    track: String,
    disc: String,
    prefix: String,
    suffix: String,
    actions: [EditAction; 4],
    pub error: Option<String>,
}

impl Editor {
    pub fn new(kind: Kind, review: &Review) -> Self {
        let mut result = Self {
            kind,
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            album_artist: String::new(),
            year: String::new(),
            track: String::new(),
            disc: String::new(),
            prefix: String::new(),
            suffix: String::new(),
            actions: [EditAction::Keep; 4],
            error: None,
        };
        match kind {
            Kind::Track(index) => {
                if let Some(track) = review.inspection.tracks.get(index) {
                    let metadata = &track.metadata;
                    result.title.clone_from(&metadata.title);
                    result.artist.clone_from(&metadata.artist);
                    result.album = if inherited(review, Field::Album) {
                        review.preferences.album_title.trim().into()
                    } else {
                        metadata.album.clone()
                    };
                    result.album_artist = if inherited(review, Field::AlbumArtist) {
                        review.preferences.album_artist.trim().into()
                    } else {
                        metadata.album_artist.clone()
                    };
                    result.year = if inherited(review, Field::Year) {
                        review.preferences.year
                    } else {
                        metadata.release_year
                    }
                    .map(|number| number.to_string())
                    .unwrap_or_default();
                    result.track = metadata
                        .track_number
                        .map(|number| number.to_string())
                        .unwrap_or_default();
                    result.disc = metadata
                        .disc_number
                        .map(|number| number.to_string())
                        .unwrap_or_default();
                }
            }
            Kind::Album => {
                result.title.clone_from(&review.preferences.title);
                result.album.clone_from(&review.preferences.album_title);
                result
                    .album_artist
                    .clone_from(&review.preferences.album_artist);
                result.year = review
                    .preferences
                    .year
                    .map(|number| number.to_string())
                    .unwrap_or_default();
            }
            Kind::OutputName => result.title.clone_from(&review.preferences.title),
            Kind::Bulk => {}
        }
        result
    }

    pub fn input(&mut self, action: Action) {
        self.error = None;
        match action {
            Action::Input(field, value) => {
                *match field {
                    Field::Title => &mut self.title,
                    Field::Artist => &mut self.artist,
                    Field::Album => &mut self.album,
                    Field::AlbumArtist => &mut self.album_artist,
                    Field::Year => &mut self.year,
                    Field::Track => &mut self.track,
                    Field::Disc => &mut self.disc,
                    Field::Prefix => &mut self.prefix,
                    Field::Suffix => &mut self.suffix,
                } = value;
            }
            Action::Choose(field, action) => {
                self.actions[match field {
                    BulkField::Artist => 0,
                    BulkField::Album => 1,
                    BulkField::AlbumArtist => 2,
                    BulkField::Year => 3,
                }] = action;
            }
            Action::Apply => {}
        }
    }

    fn bulk(&self, review: &Review) -> Result<BulkEdits, String> {
        let value = |index: usize, source: &str, field: Field| {
            if inherited(review, field) {
                return None;
            }
            match self.actions[index] {
                EditAction::Keep => None,
                EditAction::Set => Some(source.trim().to_owned()),
                EditAction::Clear => Some(String::new()),
            }
        };
        let year = if inherited(review, Field::Year) {
            None
        } else {
            match self.actions[3] {
                EditAction::Keep => None,
                EditAction::Clear => Some(None),
                EditAction::Set => Some(Some(
                    year(&self.year)?.ok_or("Enter a year, or choose Clear.")?,
                )),
            }
        };
        Ok(BulkEdits {
            artist: value(0, &self.artist, Field::Artist),
            album: value(1, &self.album, Field::Album),
            album_artist: value(2, &self.album_artist, Field::AlbumArtist),
            year,
            title_cleanup: if self.prefix.is_empty() && self.suffix.is_empty() {
                None
            } else {
                Some(TitleCleanup {
                    prefix: self.prefix.clone(),
                    suffix: self.suffix.clone(),
                })
            },
        })
    }

    pub fn apply(&self, review: &mut Review) -> Result<(), String> {
        match self.kind {
            Kind::Track(index) => {
                let original = &review
                    .inspection
                    .tracks
                    .get(index)
                    .ok_or("This track is no longer in the review.")?
                    .metadata;
                let metadata = Metadata {
                    title: self.title.trim().into(),
                    artist: self.artist.trim().into(),
                    album: if inherited(review, Field::Album) {
                        original.album.clone()
                    } else {
                        self.album.trim().into()
                    },
                    album_artist: if inherited(review, Field::AlbumArtist) {
                        original.album_artist.clone()
                    } else {
                        self.album_artist.trim().into()
                    },
                    release_year: if inherited(review, Field::Year) {
                        original.release_year
                    } else {
                        year(&self.year)?
                    },
                    track_number: number(&self.track)?,
                    disc_number: number(&self.disc)?,
                };
                review.edit_track(index, metadata)?;
            }
            Kind::Album => {
                let year = year(&self.year)?;
                if self.title.trim().is_empty() {
                    return Err("Enter an export name.".into());
                }
                if [&self.title, &self.album, &self.album_artist]
                    .iter()
                    .any(|value| value.len() > 16_384)
                {
                    return Err("A metadata field is too long.".into());
                }
                let custom_export_name = (self.title.trim() != review.preferences.title.trim())
                    .then(|| self.title.trim().to_owned());
                review.update_album(
                    self.album.trim().into(),
                    self.album_artist.trim().into(),
                    year,
                )?;
                if let Some(title) = custom_export_name {
                    review.preferences.title = title;
                }
            }
            Kind::OutputName => {
                if self.title.trim().is_empty() {
                    return Err("Enter an output name.".into());
                }
                if self.title.len() > 16_384 {
                    return Err("The output name is too long.".into());
                }
                review.changed()?;
                review.preferences.title = self.title.trim().into();
            }
            Kind::Bulk => {
                review.apply_bulk(&self.bulk(review)?)?;
            }
        }
        Ok(())
    }

    pub fn view<'a>(
        &'a self,
        review: &'a Review,
        p: crate::style::Presentation,
    ) -> Element<'a, Message> {
        let heading = match self.kind {
            Kind::Track(_) => "Edit track tags",
            Kind::Album => "Album details",
            Kind::OutputName => "Output name",
            Kind::Bulk => "Edit selected tracks",
        };
        let field = |label: &'static str,
                     value: &'a str,
                     which: Field,
                     enabled: bool|
         -> Element<'a, Message> {
            column![
                text(label).size(12).color(p.colors.text_muted),
                text_input(
                    if matches!(which, Field::Year) {
                        "YYYY"
                    } else {
                        ""
                    },
                    value
                )
                .id(match which {
                    Field::Title => "metadata-title",
                    Field::Artist => "metadata-artist",
                    Field::Album => "metadata-album",
                    Field::AlbumArtist => "metadata-album-artist",
                    Field::Year => "metadata-year",
                    Field::Track => "metadata-track",
                    Field::Disc => "metadata-disc",
                    Field::Prefix => "metadata-prefix",
                    Field::Suffix => "metadata-suffix",
                })
                .on_input_maybe(
                    enabled.then_some(move |value| Message::Editor(Action::Input(which, value)))
                )
                .padding([8, 10])
                .size(14)
                .style(p.input())
            ]
            .spacing(5)
            .into()
        };
        let mut body = Column::new().spacing(12).push(text(heading).size(18));
        if matches!(self.kind, Kind::Track(_) | Kind::Bulk) {
            if let Some(notice) = inheritance_notice(review) {
                body = body.push(text(notice).size(12).color(p.colors.text_muted));
            }
        }
        match self.kind {
            Kind::Track(_) => {
                body = body
                    .push(field("Title", &self.title, Field::Title, true))
                    .push(field("Artist", &self.artist, Field::Artist, true))
                    .push(field(
                        if inherited(review, Field::Album) {
                            "Album · shared"
                        } else {
                            "Album"
                        },
                        &self.album,
                        Field::Album,
                        !inherited(review, Field::Album),
                    ))
                    .push(field(
                        if inherited(review, Field::AlbumArtist) {
                            "Album artist · shared"
                        } else {
                            "Album artist"
                        },
                        &self.album_artist,
                        Field::AlbumArtist,
                        !inherited(review, Field::AlbumArtist),
                    ))
                    .push(
                        row![
                            field(
                                if inherited(review, Field::Year) {
                                    "Year · shared"
                                } else {
                                    "Year"
                                },
                                &self.year,
                                Field::Year,
                                !inherited(review, Field::Year)
                            ),
                            field("Track number", &self.track, Field::Track, true),
                            field("Disc number", &self.disc, Field::Disc, true)
                        ]
                        .spacing(12),
                    );
            }
            Kind::Album => {
                body=body.push(field("Output name",&self.title,Field::Title,true))
                    .push(text("Used for the exported file, folder or ZIP.").size(12).color(p.colors.text_muted))
                    .push(field("Album title",&self.album,Field::Album,true))
                    .push(field("Album artist",&self.album_artist,Field::AlbumArtist,true))
                    .push(field("Release year",&self.year,Field::Year,true))
                    .push(text(if review.preferences.mode==CollectionMode::Album{"These album tags apply to every selected track. Leave a field empty to keep each track’s own value."}else{"Individual tags are active. Switch to shared album tags in Export options to apply these details to every track."}).size(12).color(p.colors.text_muted));
            }
            Kind::OutputName => {
                body = body
                    .push(field("Name", &self.title, Field::Title, true))
                    .push(text("Used for the exported file, folder or ZIP. Audio extensions are added automatically.").size(12).color(p.colors.text_muted));
            }
            Kind::Bulk => {
                body = body.push(
                    text(format!(
                        "{} {} selected",
                        review.selected.len(),
                        if review.selected.len() == 1 {
                            "track"
                        } else {
                            "tracks"
                        }
                    ))
                    .size(12)
                    .color(p.colors.text_muted),
                );
                for (index, label, value, which, bulk_field) in [
                    (
                        0,
                        "Artist",
                        self.artist.as_str(),
                        Field::Artist,
                        BulkField::Artist,
                    ),
                    (
                        1,
                        "Album",
                        self.album.as_str(),
                        Field::Album,
                        BulkField::Album,
                    ),
                    (
                        2,
                        "Album artist",
                        self.album_artist.as_str(),
                        Field::AlbumArtist,
                        BulkField::AlbumArtist,
                    ),
                    (3, "Year", self.year.as_str(), Field::Year, BulkField::Year),
                ] {
                    if inherited(review, which) {
                        let shared = match which {
                            Field::Album => review.preferences.album_title.clone(),
                            Field::AlbumArtist => review.preferences.album_artist.clone(),
                            Field::Year => review
                                .preferences
                                .year
                                .map(|year| year.to_string())
                                .unwrap_or_default(),
                            _ => String::new(),
                        };
                        body = body.push(
                            row![
                                text(label).size(12).color(p.colors.text_muted).width(88),
                                text(shared).size(13),
                                text("Shared").size(11).color(p.colors.text_muted)
                            ]
                            .spacing(10),
                        );
                        continue;
                    }
                    body = body.push(
                        row![
                            container(text(label).size(12).color(p.colors.text_muted)).width(88),
                            pick_list(EditAction::ALL, Some(self.actions[index]), move |action| {
                                Message::Editor(Action::Choose(bulk_field, action))
                            })
                            .width(88)
                            .padding([7, 8])
                            .text_size(13),
                            text_input("", value)
                                .on_input_maybe((self.actions[index] == EditAction::Set).then_some(
                                    move |value| Message::Editor(Action::Input(which, value))
                                ))
                                .padding([8, 10])
                                .size(14)
                                .style(p.input())
                        ]
                        .spacing(8)
                        .align_y(iced::Alignment::Center),
                    );
                }
                body = body
                    .push(text("Clean up titles").size(14))
                    .push(
                        row![
                            field("Remove from start", &self.prefix, Field::Prefix, true),
                            field("Remove from end", &self.suffix, Field::Suffix, true)
                        ]
                        .spacing(12),
                    )
                    .push(
                        text("Only matching text is removed.")
                            .size(12)
                            .color(p.colors.text_muted),
                    );
                match self
                    .bulk(review)
                    .and_then(|edits| review.preview_bulk(&edits))
                {
                    Ok(preview) => {
                        body = body.push(
                            text(if preview.changed_count == 0 {
                                "No changes yet".into()
                            } else {
                                format!(
                                    "{} {} will change",
                                    preview.changed_count,
                                    if preview.changed_count == 1 {
                                        "track"
                                    } else {
                                        "tracks"
                                    }
                                )
                            })
                            .size(12)
                            .color(p.colors.text_muted),
                        );
                        for change in preview.rows.iter().filter(|row| row.changed).take(6) {
                            let mut details = Column::new().spacing(4).push(
                                text(format!("{:02}  {}", change.index + 1, change.before.title))
                                    .size(13),
                            );
                            for (label, before, after) in [
                                (
                                    "Title",
                                    change.before.title.as_str(),
                                    change.after.title.as_str(),
                                ),
                                (
                                    "Artist",
                                    change.before.artist.as_str(),
                                    change.after.artist.as_str(),
                                ),
                                (
                                    "Album",
                                    change.before.album.as_str(),
                                    change.after.album.as_str(),
                                ),
                                (
                                    "Album artist",
                                    change.before.album_artist.as_str(),
                                    change.after.album_artist.as_str(),
                                ),
                            ] {
                                if before != after {
                                    details = details.push(
                                        text(format!(
                                            "{label}: {} → {}",
                                            display_value(before),
                                            display_value(after)
                                        ))
                                        .size(12)
                                        .color(p.colors.text_muted),
                                    );
                                }
                            }
                            if change.before.release_year != change.after.release_year {
                                details = details.push(
                                    text(format!(
                                        "Year: {} → {}",
                                        change
                                            .before
                                            .release_year
                                            .map(|year| year.to_string())
                                            .unwrap_or_else(|| "Not set".into()),
                                        change
                                            .after
                                            .release_year
                                            .map(|year| year.to_string())
                                            .unwrap_or_else(|| "Not set".into())
                                    ))
                                    .size(12)
                                    .color(p.colors.text_muted),
                                );
                            }
                            body = body.push(
                                container(details)
                                    .width(Length::Fill)
                                    .padding([8, 10])
                                    .style(move |_| container::Style {
                                        background: Some(p.colors.background.into()),
                                        border: iced::Border {
                                            width: 1.0,
                                            color: p.colors.border,
                                            radius: 4.0.into(),
                                        },
                                        ..container::Style::default()
                                    }),
                            );
                        }
                    }
                    Err(error) => body = body.push(text(error).color(p.colors.danger).size(12)),
                }
            }
        }
        if let Some(error) = &self.error {
            body = body.push(text(error).color(p.colors.danger).size(12));
        }
        body.width(Length::Fill).into()
    }
}

fn inherited(review: &Review, field: Field) -> bool {
    if review.preferences.mode != CollectionMode::Album {
        return false;
    }
    match field {
        Field::Album => !review.preferences.album_title.trim().is_empty(),
        Field::AlbumArtist => !review.preferences.album_artist.trim().is_empty(),
        Field::Year => review.preferences.year.is_some(),
        _ => false,
    }
}

fn inheritance_notice(review: &Review) -> Option<String> {
    let fields: Vec<_> = [
        (Field::Album, "album title"),
        (Field::AlbumArtist, "album artist"),
        (Field::Year, "year"),
    ]
    .into_iter()
    .filter_map(|(field, label)| inherited(review, field).then_some(label))
    .collect();
    (!fields.is_empty()).then(|| format!("Album details set {}. Edit them there, or clear the shared values to keep each track’s own tags.", fields.join(", ")))
}

fn display_value(value: &str) -> &str {
    if value.is_empty() {
        "Not set"
    } else {
        value
    }
}

fn year(value: &str) -> Result<Option<u32>, String> {
    let result = number(value)?;
    if result.is_some_and(|number| !(1000..=9999).contains(&number)) {
        return Err("Enter a year from 1000 to 9999, or leave it empty.".into());
    }
    Ok(result)
}

fn number(value: &str) -> Result<Option<u32>, String> {
    if value.trim().is_empty() {
        return Ok(None);
    }
    value
        .trim()
        .parse::<u32>()
        .ok()
        .filter(|number| *number > 0)
        .map(Some)
        .ok_or_else(|| "Enter a positive whole number, or leave it empty.".into())
}

#[cfg(test)]
mod tests {
    use super::{inherited, number, year, Action, BulkField, Editor, Field, Kind};
    use crate::example;
    use crate::settings::EditAction;
    use audio_engine::{AudioFormat, AuthOptions, CollectionMode};
    use jacket_core::Review;

    fn review() -> Review {
        Review::new(example::album(), AudioFormat::Compatible)
    }

    #[test]
    fn inherited_track_fields_show_export_values_and_preserve_underlying_tags() {
        let mut review = review();
        review.preferences.album_title = "Shared album".into();
        review.preferences.album_artist = "Shared artist".into();
        review.preferences.year = Some(2024);
        let original = review.inspection.tracks[0].metadata.clone();
        let mut editor = Editor::new(Kind::Track(0), &review);
        assert_eq!(editor.album, "Shared album");
        assert_eq!(editor.album_artist, "Shared artist");
        assert_eq!(editor.year, "2024");
        editor.input(Action::Input(Field::Title, "New title".into()));
        // Disabled controls cannot change hidden tags even if an old input is delivered.
        editor.input(Action::Input(Field::Album, "Ignored album".into()));
        editor.input(Action::Input(Field::AlbumArtist, "Ignored artist".into()));
        editor.input(Action::Input(Field::Year, "not a year".into()));
        editor.apply(&mut review).expect("edit title");
        let metadata = &review.inspection.tracks[0].metadata;
        assert_eq!(metadata.title, "New title");
        assert_eq!(metadata.album, original.album);
        assert_eq!(metadata.album_artist, original.album_artist);
        assert_eq!(metadata.release_year, original.release_year);
        let plan = review.plan(AuthOptions::default()).expect("export plan");
        assert_eq!(plan.tracks[0].metadata.album, "Shared album");
        assert_eq!(plan.tracks[0].metadata.release_year, Some(2024));
    }

    #[test]
    fn blank_shared_values_make_individual_track_fields_editable() {
        let mut review = review();
        review.preferences.album_title.clear();
        review.preferences.album_artist.clear();
        review.preferences.year = None;
        assert!(!inherited(&review, Field::Album));
        assert!(!inherited(&review, Field::AlbumArtist));
        assert!(!inherited(&review, Field::Year));
        let mut editor = Editor::new(Kind::Track(0), &review);
        editor.input(Action::Input(Field::Album, "Track album".into()));
        editor.input(Action::Input(
            Field::AlbumArtist,
            "Track album artist".into(),
        ));
        editor.input(Action::Input(Field::Year, "2001".into()));
        editor.apply(&mut review).expect("individual tags");
        let metadata = &review.plan(AuthOptions::default()).expect("plan").tracks[0].metadata;
        assert_eq!(metadata.album, "Track album");
        assert_eq!(metadata.album_artist, "Track album artist");
        assert_eq!(metadata.release_year, Some(2001));
    }

    #[test]
    fn bulk_edits_omit_inherited_fields_and_allow_independent_artist_edits() {
        let mut review = review();
        let original = review.inspection.tracks[0].metadata.clone();
        let mut editor = Editor::new(Kind::Bulk, &review);
        for field in [
            BulkField::Artist,
            BulkField::Album,
            BulkField::AlbumArtist,
            BulkField::Year,
        ] {
            editor.input(Action::Choose(field, EditAction::Set));
        }
        editor.input(Action::Input(Field::Artist, "Edited artist".into()));
        editor.input(Action::Input(Field::Album, "Ineffective album".into()));
        editor.input(Action::Input(Field::Year, "invalid inherited year".into()));
        let edits = editor.bulk(&review).expect("inherited year is omitted");
        assert!(edits.album.is_none());
        assert!(edits.album_artist.is_none());
        assert!(edits.year.is_none());
        editor.apply(&mut review).expect("bulk edit");
        assert_eq!(review.inspection.tracks[0].metadata.artist, "Edited artist");
        assert_eq!(review.inspection.tracks[0].metadata.album, original.album);
        assert_eq!(
            review.inspection.tracks[0].metadata.release_year,
            original.release_year
        );
    }

    #[test]
    fn playlist_mode_uses_individual_values_despite_stored_album_preferences() {
        let mut review = review();
        review
            .set_mode(CollectionMode::Collection)
            .expect("playlist mode");
        let mut editor = Editor::new(Kind::Bulk, &review);
        editor.input(Action::Choose(BulkField::Album, EditAction::Set));
        editor.input(Action::Input(Field::Album, "Playlist track album".into()));
        editor.apply(&mut review).expect("bulk album edit");
        assert_eq!(
            review.plan(AuthOptions::default()).expect("plan").tracks[0]
                .metadata
                .album,
            "Playlist track album"
        );
    }

    #[test]
    fn album_title_updates_followed_export_name_but_keeps_explicit_custom_names() {
        let mut review = review();
        let mut editor = Editor::new(Kind::Album, &review);
        editor.input(Action::Input(Field::Album, "New album".into()));
        editor.apply(&mut review).expect("rename album");
        assert_eq!(review.preferences.title, "New album");
        let mut editor = Editor::new(Kind::Album, &review);
        editor.input(Action::Input(Field::Album, "Another album".into()));
        editor.input(Action::Input(Field::Title, "Custom folder".into()));
        editor.apply(&mut review).expect("custom export name");
        assert_eq!(review.preferences.title, "Custom folder");
        let mut editor = Editor::new(Kind::Album, &review);
        editor.input(Action::Input(Field::Album, "Third album".into()));
        editor.apply(&mut review).expect("keep custom name");
        assert_eq!(review.preferences.title, "Custom folder");
    }

    #[test]
    fn years_and_track_numbers_reject_fractional_negative_and_out_of_range_values() {
        for value in ["0", "-1", "1.5", "1e3", "NaN", "4294967296"] {
            assert!(number(value).is_err(), "accepted number {value}");
        }
        for value in ["999", "10000", "0", "2026.5", "2026AD"] {
            assert!(year(value).is_err(), "accepted year {value}");
        }
        assert_eq!(number(" 12 ").expect("number"), Some(12));
        assert_eq!(number(" ").expect("blank number"), None);
        assert_eq!(year("1000").expect("minimum year"), Some(1000));
        assert_eq!(year("9999").expect("maximum year"), Some(9999));
        assert_eq!(year("").expect("blank year"), None);
    }

    #[test]
    fn bulk_year_set_requires_a_value_and_clear_is_explicit() {
        let mut review = review();
        review
            .set_mode(CollectionMode::Collection)
            .expect("playlist mode");
        let mut editor = Editor::new(Kind::Bulk, &review);
        editor.input(Action::Choose(BulkField::Year, EditAction::Set));
        assert!(editor.apply(&mut review).is_err());
        editor.input(Action::Choose(BulkField::Year, EditAction::Clear));
        editor.apply(&mut review).expect("clear year");
        assert!(review
            .inspection
            .tracks
            .iter()
            .all(|track| track.metadata.release_year.is_none()));
    }

    #[test]
    fn invalid_track_and_album_forms_preserve_the_review_atomically() {
        let mut review = review();
        let original = serde_json::to_string(&review.draft()).expect("original snapshot");
        let mut editor = Editor::new(Kind::Track(0), &review);
        editor.input(Action::Input(Field::Track, "1.5".into()));
        assert!(editor.apply(&mut review).is_err());
        let mut editor = Editor::new(Kind::Album, &review);
        editor.input(Action::Input(Field::Album, "Unsaved album".into()));
        editor.input(Action::Input(Field::Year, "10000".into()));
        assert!(editor.apply(&mut review).is_err());
        assert_eq!(
            serde_json::to_string(&review.draft()).expect("unchanged snapshot"),
            original
        );
    }
}
