use crate::{BulkEdits, Review, SourceIntent, SourceLink, TitleCleanup};
use audio_engine::{
    ArtworkMode, AudioFormat, AuthOptions, CollectionMode, Inspection, Metadata, OutputKind, Track,
};

fn inspection(mode: CollectionMode, count: usize) -> Inspection {
    Inspection {
        title: "Example album".into(),
        source_url: "https://music.youtube.com/playlist?list=PLabcdefghijk".into(),
        suggested_mode: mode,
        artwork_url: None,
        album_release_year: Some(2020),
        tracks: (0..count)
            .map(|index| {
                let metadata = Metadata {
                    title: format!("Prefix Song {} (Live)", index + 1),
                    artist: "Artist".into(),
                    album: "Example album".into(),
                    release_year: Some(2019),
                    ..Metadata::default()
                };
                Track {
                    id: "dQw4w9WgXcQ".into(),
                    source_url: "https://www.youtube.com/watch?v=dQw4w9WgXcQ".into(),
                    uploader: String::new(),
                    duration: Some(90.0),
                    artwork_url: None,
                    source_metadata: metadata.clone(),
                    metadata,
                }
            })
            .collect(),
        warnings: Vec::new(),
    }
}

#[test]
fn default_destinations_follow_song_and_collection_intent() {
    let song = Review::new(
        inspection(CollectionMode::Collection, 1),
        AudioFormat::Compatible,
    );
    assert_eq!(song.preferences.output_kind, OutputKind::File);
    assert_eq!(song.preferences.title, "Artist - Prefix Song 1 (Live)");
    for mode in [CollectionMode::Collection, CollectionMode::Album] {
        let collection = Review::new(inspection(mode, 2), AudioFormat::Mp3);
        assert_eq!(collection.preferences.output_kind, OutputKind::Folder);
    }
}

#[test]
fn repeated_entries_are_independent_occurrences_and_deselection_keeps_numbering_gaps() {
    let mut review = Review::new(inspection(CollectionMode::Album, 3), AudioFormat::Mp3);
    review.select(1, false).expect("select occurrence");
    let mut metadata = review.inspection.tracks[2].metadata.clone();
    metadata.title = "Only the third occurrence".into();
    review.edit_track(2, metadata).expect("edit occurrence");
    let plan = review.plan(AuthOptions::default()).expect("valid plan");
    assert_eq!(plan.tracks.len(), 2);
    assert_eq!(plan.tracks[0].id, plan.tracks[1].id);
    assert_eq!(plan.tracks[1].metadata.title, "Only the third occurrence");
    assert_eq!(plan.tracks[1].metadata.track_number, Some(3));
    assert_eq!(
        review.inspection.tracks[1].metadata.title,
        "Prefix Song 2 (Live)"
    );
    assert_eq!(
        review.inspection.tracks[2].source_metadata.title,
        "Prefix Song 3 (Live)"
    );
}

#[test]
fn album_overrides_preserve_per_track_unknowns_and_original_source_metadata() {
    let mut source = inspection(CollectionMode::Album, 2);
    source.tracks[1].metadata.release_year = Some(2018);
    source.tracks[1].metadata.disc_number = Some(2);
    let mut review = Review::new(source, AudioFormat::Preserve);
    assert_eq!(review.preferences.year, Some(2020));
    review
        .update_album(" Edited album ".into(), " Album Artist ".into(), None)
        .expect("edit album");
    let plan = review.plan(AuthOptions::default()).expect("valid plan");
    assert_eq!(plan.tracks[1].metadata.album, "Edited album");
    assert_eq!(plan.tracks[1].metadata.album_artist, "Album Artist");
    assert_eq!(plan.tracks[1].metadata.release_year, Some(2018));
    assert_eq!(plan.tracks[1].metadata.track_number, Some(1));
    assert_eq!(plan.tracks[1].source_metadata.album, "Example album");
    assert_eq!(review.inspection.tracks[1].metadata.album, "Example album");
}

#[test]
fn bulk_cleanup_previews_and_undo_preserve_later_manual_fields() {
    let mut review = Review::new(
        inspection(CollectionMode::Collection, 3),
        AudioFormat::Compatible,
    );
    review.select(1, false).expect("select subset");
    let edits = BulkEdits {
        artist: Some(" New Artist ".into()),
        year: Some(None),
        title_cleanup: Some(TitleCleanup {
            prefix: "Prefix ".into(),
            suffix: " (Live)".into(),
        }),
        ..BulkEdits::default()
    };
    let preview = review.preview_bulk(&edits).expect("valid preview");
    assert_eq!(preview.changed_count, 2);
    assert_eq!(preview.rows[1].index, 2);
    assert_eq!(preview.rows[1].after.title, "Song 3");
    assert_eq!(
        review.inspection.tracks[2].metadata.title,
        "Prefix Song 3 (Live)"
    );
    assert_eq!(review.apply_bulk(&edits).expect("apply edits"), 2);
    let mut later = review.inspection.tracks[2].metadata.clone();
    later.title = "Manual later title".into();
    review.edit_track(2, later).expect("manual edit");
    assert!(review.undo_bulk().expect("undo edits"));
    assert_eq!(
        review.inspection.tracks[0].metadata.title,
        "Prefix Song 1 (Live)"
    );
    assert_eq!(
        review.inspection.tracks[2].metadata.title,
        "Manual later title"
    );
    assert_eq!(review.inspection.tracks[2].metadata.artist, "Artist");
    assert_eq!(
        review.inspection.tracks[2].metadata.release_year,
        Some(2019)
    );
    assert_eq!(
        review.inspection.tracks[1].metadata.title,
        "Prefix Song 2 (Live)"
    );
    assert_eq!(review.inspection.tracks[2].source_metadata.artist, "Artist");
}

#[test]
fn invalid_bulk_edit_is_atomic_and_cleanup_does_not_trim_unmatched_titles() {
    let mut review = Review::new(inspection(CollectionMode::Collection, 1), AudioFormat::Mp3);
    let original = serde_json::to_string(&review.draft()).expect("serialize review");
    let edits = BulkEdits {
        title_cleanup: Some(TitleCleanup {
            prefix: "Prefix Song 1 (Live)".into(),
            suffix: String::new(),
        }),
        ..BulkEdits::default()
    };
    assert!(review.apply_bulk(&edits).is_err());
    assert_eq!(
        serde_json::to_string(&review.draft()).expect("serialize review"),
        original
    );
    review.inspection.tracks[0].metadata.title = "  Unmatched  ".into();
    let edits = BulkEdits {
        title_cleanup: Some(TitleCleanup {
            prefix: "Not matching".into(),
            suffix: String::new(),
        }),
        ..BulkEdits::default()
    };
    assert_eq!(
        review.preview_bulk(&edits).expect("preview").rows[0]
            .after
            .title,
        "  Unmatched  "
    );
}

#[test]
fn song_export_name_follows_edits_until_user_chooses_custom_name() {
    let mut review = Review::new(
        inspection(CollectionMode::Collection, 1),
        AudioFormat::Compatible,
    );
    let mut metadata = review.inspection.tracks[0].metadata.clone();
    metadata.title = "Edited song".into();
    review.edit_track(0, metadata.clone()).expect("edit song");
    assert_eq!(review.preferences.title, "Artist - Edited song");
    review.preferences.title = "My recording".into();
    metadata.title = "Another title".into();
    review.edit_track(0, metadata).expect("edit song");
    assert_eq!(review.preferences.title, "My recording");
}

#[test]
fn renumber_undo_keeps_later_track_number_and_restarts_for_each_disc() {
    let mut source = inspection(CollectionMode::Album, 3);
    source.tracks[0].metadata.track_number = Some(8);
    source.tracks[1].metadata.track_number = Some(9);
    source.tracks[2].metadata.disc_number = Some(2);
    source.tracks[2].metadata.track_number = Some(7);
    let mut review = Review::new(source, AudioFormat::Mp3);
    review.number_album_tracks().expect("renumber");
    assert_eq!(review.inspection.tracks[2].metadata.track_number, Some(1));
    let mut metadata = review.inspection.tracks[1].metadata.clone();
    metadata.track_number = Some(4);
    review.edit_track(1, metadata).expect("later edit");
    review.undo_numbering().expect("undo numbering");
    assert_eq!(review.inspection.tracks[0].metadata.track_number, Some(8));
    assert_eq!(review.inspection.tracks[1].metadata.track_number, Some(4));
    assert_eq!(review.inspection.tracks[2].metadata.track_number, Some(7));
}

#[test]
fn old_drafts_restore_selection_and_revision_without_credentials_or_runtime_paths() {
    let mut review = Review::new(inspection(CollectionMode::Collection, 3), AudioFormat::Mp3);
    review.select(0, false).expect("deselect");
    let identity = review.identity();
    let serialized = serde_json::to_string(&review.draft()).expect("serialize");
    assert!(!serialized.contains("browser"));
    assert!(!serialized.contains("cookie"));
    let restored = Review::from_draft(serde_json::from_str(&serialized).expect("deserialize"))
        .expect("restore");
    assert_eq!(restored.identity(), identity);
    assert_eq!(
        restored.selected.iter().copied().collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert!(restored.plan(AuthOptions::default()).is_ok());
    let mut broken = restored.draft();
    broken.selected = vec![1, 1];
    assert!(Review::from_draft(broken).is_err());
    let mut old = serde_json::to_value(restored.draft()).expect("serialize");
    old["preferences"]
        .as_object_mut()
        .expect("preferences")
        .remove("outputKind");
    let restored =
        Review::from_draft(serde_json::from_value(old).expect("old draft")).expect("restore old");
    assert_eq!(restored.preferences.output_kind, OutputKind::Zip);
}

#[test]
fn recovery_plan_keeps_already_committed_song_album_tags_and_output_choice() {
    let mut review = Review::new(
        inspection(CollectionMode::Collection, 1),
        AudioFormat::Preserve,
    );
    review.preferences.title = "Custom output".into();
    let plan = review.plan(AuthOptions::default()).expect("valid plan");
    let restored = Review::from_plan(&plan);
    let recovered_plan = restored
        .plan(AuthOptions::default())
        .expect("recovered plan");
    assert_eq!(recovered_plan.output_kind, OutputKind::File);
    assert_eq!(recovered_plan.title, "Custom output");
    assert_eq!(recovered_plan.tracks[0].metadata.album, "Example album");
}

#[test]
fn source_intent_uses_same_validated_link_as_review() {
    let source = "https://youtu.be/dQw4w9WgXcQ?list=PLabcdefghijk";
    let link = SourceLink::parse(source).expect("source");
    assert_eq!(
        link.target(SourceIntent::Playlist),
        "https://www.youtube.com/playlist?list=PLabcdefghijk"
    );
}

#[test]
fn a_song_has_one_cover_and_prefers_its_track_artwork() {
    let mut source = inspection(CollectionMode::Collection, 1);
    source.artwork_url = Some("https://i.ytimg.com/source.jpg".into());
    source.tracks[0].artwork_url = Some("https://lh3.googleusercontent.com/album-cover".into());
    let review = Review::new(source, AudioFormat::Compatible);
    assert!(review.is_song());
    assert!(matches!(
        review.preferences.artwork_mode,
        ArtworkMode::Shared
    ));
    assert_eq!(
        review.preferences.shared_artwork.as_deref(),
        Some("https://lh3.googleusercontent.com/album-cover")
    );
    assert!(matches!(
        review
            .plan(AuthOptions::default())
            .expect("song plan")
            .artwork_mode,
        ArtworkMode::Shared
    ));
    assert!(!Review::new(inspection(CollectionMode::Album, 2), AudioFormat::Mp3).is_song());
}

#[test]
fn legacy_song_cover_settings_resolve_without_mutating_the_saved_review() {
    let mut source = inspection(CollectionMode::Collection, 1);
    source.tracks[0].artwork_url = Some("https://i.ytimg.com/song.jpg".into());
    let mut review = Review::new(source, AudioFormat::Mp3);
    review.preferences.artwork_mode = ArtworkMode::PerTrack;
    review.preferences.shared_artwork = None;
    let before = serde_json::to_string(&review.draft()).expect("saved review");
    let plan = review
        .plan(AuthOptions::default())
        .expect("legacy song plan");
    assert!(matches!(plan.artwork_mode, ArtworkMode::Shared));
    assert_eq!(
        plan.shared_artwork_url.as_deref(),
        Some("https://i.ytimg.com/song.jpg")
    );
    assert_eq!(
        serde_json::to_string(&review.draft()).expect("saved review"),
        before
    );
    review.preferences.artwork_mode = ArtworkMode::None;
    assert!(matches!(
        review
            .plan(AuthOptions::default())
            .expect("without cover")
            .artwork_mode,
        ArtworkMode::None
    ));
    review.preferences.artwork_mode = ArtworkMode::Shared;
    review.preferences.shared_artwork_asset = Some("chosen-local-cover".into());
    assert_eq!(
        review
            .plan(AuthOptions::default())
            .expect("local cover")
            .shared_artwork_asset
            .as_deref(),
        Some("chosen-local-cover")
    );
}

#[test]
fn effective_metadata_matches_export_and_never_changes_source_tags() {
    let mut review = Review::new(inspection(CollectionMode::Album, 2), AudioFormat::Mp3);
    review
        .update_album(
            " Shared album ".into(),
            " Shared artist ".into(),
            Some(2025),
        )
        .expect("album tags");
    let tags = review.effective_metadata(0).expect("effective metadata");
    assert_eq!(tags.album, "Shared album");
    assert_eq!(tags.album_artist, "Shared artist");
    assert_eq!(tags.release_year, Some(2025));
    let plan = review.plan(AuthOptions::default()).expect("export");
    assert_eq!(
        serde_json::to_value(&tags).expect("tags"),
        serde_json::to_value(&plan.tracks[0].metadata).expect("planned tags")
    );
    assert_eq!(review.inspection.tracks[0].metadata.album, "Example album");
    assert_eq!(
        review.inspection.tracks[0].source_metadata.release_year,
        Some(2019)
    );
    assert!(review.effective_metadata(99).is_none());
    review
        .set_mode(CollectionMode::Collection)
        .expect("individual tags");
    let tags = review.effective_metadata(0).expect("individual metadata");
    assert_eq!(tags.album, "Example album");
    assert_eq!(tags.release_year, Some(2019));
}
