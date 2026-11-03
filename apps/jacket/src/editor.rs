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
            Kind::Bulk => {
                review.apply_bulk(&self.bulk(review)?)?;
            }
        }
        Ok(())
    }

    pub fn view<'a>(&'a self, review: &'a Review) -> Element<'a, Message> {
        let heading = match self.kind {
            Kind::Track(_) => "Song details",
            Kind::Album => "Album & export details",
            Kind::Bulk => "Edit selected tracks",
        };
        let field = |label: &'static str,
                     value: &'a str,
                     which: Field,
                     enabled: bool|
         -> Element<'a, Message> {
            column![
                text(label).size(13),
                text_input(label, value)
                    .on_input_maybe(
                        enabled
                            .then_some(move |value| Message::Editor(Action::Input(which, value)))
                    )
                    .padding(10)
            ]
            .spacing(5)
            .into()
        };
        let mut body = Column::new().spacing(14).push(text(heading).size(26));
        if !matches!(self.kind, Kind::Album) {
            if let Some(notice) = inheritance_notice(review) {
                body = body.push(text(notice).size(13));
            }
        }
        match self.kind {
            Kind::Track(_) => {
                body = body
                    .push(field("Title", &self.title, Field::Title, true))
                    .push(field("Artist", &self.artist, Field::Artist, true))
                    .push(field(
                        if inherited(review, Field::Album) {
                            "Album · shared album value"
                        } else {
                            "Album"
                        },
                        &self.album,
                        Field::Album,
                        !inherited(review, Field::Album),
                    ))
                    .push(field(
                        if inherited(review, Field::AlbumArtist) {
                            "Album artist · shared album value"
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
                                    "Year · shared album value"
                                } else {
                                    "Year"
                                },
                                &self.year,
                                Field::Year,
                                !inherited(review, Field::Year)
                            ),
                            field("Track number", &self.track, Field::Track, true),
                            field("Disc number", &self.disc, Field::Disc, true),
                        ]
                        .spacing(12),
                    );
            }
            Kind::Album => {
                body = body
                    .push(field("Export name", &self.title, Field::Title, true))
                    .push(field("Album title", &self.album, Field::Album, true))
                    .push(field("Album artist", &self.album_artist, Field::AlbumArtist, true))
                    .push(field("Release year", &self.year, Field::Year, true))
                    .push(text(if review.preferences.mode == CollectionMode::Album {
                        "Album mode applies these shared tags to selected tracks. Empty shared fields preserve each track’s own tags. The export name follows the album title until you choose a custom name."
                    } else {
                        "Playlist mode preserves each track’s own tags. Choose Album mode in Download options to apply these shared album details."
                    }).size(13));
            }
            Kind::Bulk => {
                body = body.push(
                    text(format!(
                        "{} selected occurrences · source metadata stays intact",
                        review.selected.len()
                    ))
                    .size(13),
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
                        body = body
                            .push(text(format!("{label}: {shared} · shared album value")).size(13));
                        continue;
                    }
                    body = body.push(
                        row![
                            container(text(label).size(13)).width(100),
                            pick_list(EditAction::ALL, Some(self.actions[index]), move |action| {
                                Message::Editor(Action::Choose(bulk_field, action))
                            })
                            .width(100),
                            text_input("New value", value)
                                .on_input_maybe((self.actions[index] == EditAction::Set).then_some(
                                    move |value| Message::Editor(Action::Input(which, value))
                                ))
                                .padding(8),
                        ]
                        .spacing(8)
                        .align_y(iced::Alignment::Center),
                    );
                }
                body = body
                    .push(text("Title cleanup · exact matches only").size(14))
                    .push(
                        row![
                            field("Remove prefix", &self.prefix, Field::Prefix, true),
                            field("Remove suffix", &self.suffix, Field::Suffix, true)
                        ]
                        .spacing(12),
                    );
                match self
                    .bulk(review)
                    .and_then(|edits| review.preview_bulk(&edits))
                {
                    Ok(preview) => {
                        body = body.push(
                            text(format!("{} tracks will change", preview.changed_count)).size(13),
                        );
                        for change in preview
                            .rows
                            .iter()
                            .filter(|row| row.before.title != row.after.title)
                            .take(6)
                        {
                            body = body.push(
                                text(format!(
                                    "{}  →  {}",
                                    change.before.title, change.after.title
                                ))
                                .size(13),
                            );
                        }
                    }
                    Err(error) => {
                        body = body.push(text(error).color(iced::color!(0xBE4940)).size(13))
                    }
                }
            }
        }
        if let Some(error) = &self.error {
            body = body.push(text(error).color(iced::color!(0xBE4940)).size(13));
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
    (!fields.is_empty()).then(|| format!("Shared album details control {}. Change them in Album & export details, or clear the shared values there to use each track’s own tags.", fields.join(", ")))
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
