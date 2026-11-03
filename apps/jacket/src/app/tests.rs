use super::persistence::{persist_review, read_appearance};
use super::{App, Message, Panel};
use crate::example;
use audio_engine::AudioFormat;
use jacket_core::Review;
use jacket_desktop::Desktop;

fn desktop() -> (tempfile::TempDir, Desktop) {
    let root = tempfile::tempdir().expect("temporary desktop");
    let desktop = Desktop::with_directories(
        root.path().join("data"),
        root.path().join("cache"),
        root.path().join("resources"),
        root.path().join("home"),
    )
    .expect("isolated desktop");
    (root, desktop)
}

fn app(desktop: Desktop) -> App {
    let (mut app, _) = App::new();
    app.loading = false;
    app.example = false;
    app.desktop = Some(desktop);
    app.review = Some(Review::new(example::album(), AudioFormat::Compatible));
    app
}

#[test]
fn export_barrier_accepts_identical_saved_revision_and_rejects_a_tombstone() {
    let (_root, desktop) = desktop();
    let review = Review::new(example::album(), AudioFormat::Mp3);
    assert!(persist_review(&desktop, review.draft()).expect("first save"));
    assert!(persist_review(&desktop, review.draft()).expect("already saved"));
    let identity = review.identity();
    desktop
        .discard_review(&identity.session_id, identity.revision + 1)
        .expect("discard review");
    assert!(persist_review(&desktop, review.draft()).is_err());
    assert!(desktop.load_review().expect("saved draft").is_none());
}

#[test]
fn pending_autosaves_block_session_replacement_and_example_entry() {
    let (_root, desktop) = desktop();
    let mut app = app(desktop);
    let identity = app.review.as_ref().expect("review").identity();
    app.save_pending = 1;
    let _ = app.update(Message::Example);
    let _ = app.update(Message::ConfirmReset);
    assert!(!app.example);
    assert_eq!(
        app.review.as_ref().expect("review kept").identity(),
        identity
    );
    assert_eq!(app.panel, Panel::Workspace);
}

#[test]
fn example_edits_and_appearance_restore_the_previous_review_without_saving() {
    let (_root, desktop) = desktop();
    let mut app = app(desktop.clone());
    let original = app.review.as_ref().expect("review").draft();
    persist_review(&desktop, original.clone()).expect("save real review");
    let theme = app.appearance.theme.clone();
    let _ = app.update(Message::Example);
    let _ = app.update(Message::Toggle(0, false));
    let _ = app.update(Message::ThemePreset(crate::theme::Preset::Light));
    assert_eq!(app.save_pending, 0);
    let _ = app.update(Message::ConfirmReset);
    assert!(!app.example);
    assert_eq!(app.appearance.theme, theme);
    assert_eq!(
        app.review
            .as_ref()
            .expect("restored review")
            .identity()
            .session_id,
        original.session_id
    );
    assert!(app.review.as_ref().expect("review").selected.contains(&0));
    assert_eq!(
        desktop
            .load_review()
            .expect("saved review")
            .expect("real draft")
            .revision,
        original.revision
    );
}

#[test]
fn resumed_export_preserves_an_unrelated_or_newer_review() {
    let (_root, desktop) = desktop();
    let mut app = app(desktop);
    app.review
        .as_mut()
        .expect("review")
        .changed()
        .expect("newer revision");
    let original = app.review.as_ref().expect("review").identity();
    let unrelated = Review::new(example::album(), AudioFormat::Mp3).identity();
    let previous = app.suspend_review();
    app.resumed_previous = Some((previous, Some(unrelated)));
    app.review = Some(Review::new(example::album(), AudioFormat::Preserve));
    app.finish_resumed_review(true);
    assert_eq!(
        app.review
            .as_ref()
            .expect("unrelated review kept")
            .identity(),
        original
    );

    let mut older_export = original.clone();
    older_export.revision -= 1;
    let previous = app.suspend_review();
    app.resumed_previous = Some((previous, Some(older_export)));
    app.finish_resumed_review(true);
    assert_eq!(
        app.review.as_ref().expect("newer edits kept").identity(),
        original
    );
}

#[test]
fn resumed_success_retires_only_its_matching_review_and_failure_restores_it() {
    let (_root, desktop) = desktop();
    let mut app = app(desktop);
    let identity = app.review.as_ref().expect("review").identity();
    let previous = app.suspend_review();
    app.resumed_previous = Some((previous, Some(identity.clone())));
    app.review = Some(Review::new(example::album(), AudioFormat::Mp3));
    app.finish_resumed_review(false);
    assert_eq!(
        app.review
            .as_ref()
            .expect("failed resume restores review")
            .identity(),
        identity
    );
    let previous = app.suspend_review();
    app.resumed_previous = Some((previous, Some(identity)));
    app.finish_resumed_review(true);
    assert!(app.review.is_none());
    assert_eq!(app.save_pending, 0);
}

#[test]
fn failed_discard_keeps_the_committed_review_visible() {
    let (_root, desktop) = desktop();
    let mut app = app(desktop);
    let identity = app.review.as_ref().expect("review").identity();
    app.dialog_open = true;
    let _ = app.update(Message::ReviewDiscarded(Err("Disk unavailable".into())));
    assert_eq!(
        app.review.as_ref().expect("review preserved").identity(),
        identity
    );
    assert!(!app.dialog_open);
    assert!(app
        .error
        .as_deref()
        .is_some_and(|error| error.contains("Disk unavailable")));
}

#[test]
fn appearance_writes_are_serialized_and_coalesce_to_the_latest_choice() {
    let (_root, desktop) = desktop();
    let mut app = app(desktop);
    let _ = app.update(Message::ThemePreset(crate::theme::Preset::Dark));
    assert!(app.appearance_saving);
    assert!(!app.appearance_dirty);
    let _ = app.update(Message::ThemePreset(crate::theme::Preset::Light));
    assert!(app.appearance_dirty);
    let _ = app.update(Message::AppearanceSaved(Ok(())));
    assert!(app.appearance_saving);
    assert!(!app.appearance_dirty);
    assert_eq!(
        app.appearance.theme,
        crate::theme::Selection::Preset {
            preset: crate::theme::Preset::Light
        }
    );
    let _ = app.update(Message::AppearanceSaved(Ok(())));
    assert!(!app.appearance_saving);
}

#[test]
fn a_new_saved_review_fences_all_later_writes_from_its_predecessor() {
    let (_root, desktop) = desktop();
    let mut old = Review::new(example::album(), AudioFormat::Compatible);
    persist_review(&desktop, old.draft()).expect("initial review");
    let new = Review::new(example::album(), AudioFormat::Mp3);
    persist_review(&desktop, new.draft()).expect("replacement review");
    old.changed().expect("delayed revision");
    assert!(persist_review(&desktop, old.draft()).is_err());
    assert_eq!(
        desktop
            .load_review()
            .expect("load")
            .expect("replacement")
            .session_id,
        new.identity().session_id
    );
}

#[test]
fn appearance_reader_rejects_oversized_records() {
    let (_root, desktop) = desktop();
    std::fs::create_dir_all(desktop.data_root()).expect("data directory");
    std::fs::write(
        desktop.data_root().join("native-preferences.json"),
        vec![b' '; 524289],
    )
    .expect("oversized preferences");
    assert!(read_appearance(&desktop).is_err());
}
#[test]
fn stale_save_results_do_not_replace_the_current_review_save_status() {
    let (_root, desktop) = desktop();
    let mut app = app(desktop);
    let old = app.review.as_ref().expect("review").identity();
    app.review
        .as_mut()
        .expect("review")
        .changed()
        .expect("new revision");
    let current = app.review.as_ref().expect("review").identity();
    let _ = app.update(Message::Saved(old.clone(), Err("old write failed".into())));
    assert!(!app.save_failed);
    let _ = app.update(Message::Saved(
        current.clone(),
        Err("current write failed".into()),
    ));
    assert!(app.save_failed);
    let _ = app.update(Message::Saved(old, Ok(true)));
    assert!(app.save_failed);
    let _ = app.update(Message::Saved(current, Ok(true)));
    assert!(!app.save_failed);
}

#[test]
fn close_waits_for_pending_review_and_appearance_writes_before_final_flush() {
    let (_root, desktop) = desktop();
    let mut app = app(desktop);
    app.save_pending = 1;
    app.appearance_saving = true;
    let _ = app.close();
    assert!(app.close_after_job);
    assert!(!app.dialog_open);
    app.save_pending = 0;
    let _ = app.tick();
    assert!(!app.dialog_open);
    app.appearance_saving = false;
    let _ = app.tick();
    assert!(app.dialog_open);
}

#[test]
fn legacy_appearance_migrates_explicit_light_and_dark_choices() {
    use crate::theme::{Preset, Selection};
    let (_root, desktop) = desktop();
    let path = desktop.data_root().join("native-preferences.json");
    for (source, preset) in [
        (r#"{"dark":false,"default_format":"mp3"}"#, Preset::Light),
        (r#"{"dark":true,"default_format":"mp3"}"#, Preset::Dark),
        (r#"{"default_format":"mp3"}"#, Preset::Dark),
    ] {
        std::fs::write(&path, source).expect("legacy preferences");
        let appearance = read_appearance(&desktop).expect("migrate preferences");
        assert_eq!(appearance.theme, Selection::Preset { preset });
        assert!(matches!(
            appearance.default_format,
            crate::settings::FormatPreference::Mp3
        ));
        assert_eq!(
            std::fs::read_to_string(&path).expect("original legacy record"),
            source
        );
    }
}

#[test]
fn custom_theme_changes_leave_review_selection_and_worker_ownership_unchanged() {
    use crate::theme::Selection;
    let (_root, desktop) = desktop();
    let mut app = app(desktop.clone());
    app.review
        .as_mut()
        .expect("review")
        .select(1, false)
        .expect("selection");
    let original =
        serde_json::to_vec(&app.review.as_ref().expect("review").draft()).expect("snapshot");
    // No browser is selected, so this worker rejects access before extraction.
    app.job = Some(
        desktop
            .test_access(
                "https://www.youtube.com/watch?v=abcdefghijk".into(),
                audio_engine::AuthOptions::default(),
            )
            .expect("admitted worker"),
    );
    let worker = app.job.as_ref().expect("worker").id().to_owned();
    let selection = Selection::Custom {
        name: "Studio".into(),
        source: "version=1\nbase='dark'\n[colors]\naccent='#FFBB88'".into(),
    };
    let _ = app.update(Message::ThemeImported(Ok(selection.clone())));
    assert_eq!(app.appearance.theme, selection);
    assert_eq!(app.job.as_ref().expect("worker retained").id(), worker);
    assert_eq!(
        serde_json::to_vec(&app.review.as_ref().expect("review retained").draft())
            .expect("unchanged snapshot"),
        original
    );
    assert_eq!(app.save_pending, 0);
    assert!(app.appearance_saving);
}

#[test]
fn failed_theme_import_preserves_the_current_theme_and_saved_preference() {
    use crate::theme::{Preset, Selection};
    let (_root, desktop) = desktop();
    let mut app = app(desktop.clone());
    app.appearance.theme = Selection::Preset {
        preset: Preset::Light,
    };
    app.theme = app.appearance.theme.resolve(4).expect("light theme");
    super::persistence::write_appearance(&desktop, &app.appearance).expect("saved preference");
    let path = desktop.data_root().join("native-preferences.json");
    let original = std::fs::read(&path).expect("saved bytes");
    let colors = app.theme.colors;
    for result in [
        Err("Could not read selected file".into()),
        Ok(Selection::Custom {
            name: "Broken".into(),
            source: "version=99\nbase='dark'".into(),
        }),
    ] {
        app.dialog_open = true;
        let _ = app.update(Message::ThemeImported(result));
        assert_eq!(
            app.appearance.theme,
            Selection::Preset {
                preset: Preset::Light
            }
        );
        assert_eq!(app.theme.colors, colors);
        assert_eq!(std::fs::read(&path).expect("preserved bytes"), original);
        assert!(!app.dialog_open);
        assert!(!app.appearance_saving);
    }
}

#[test]
fn closing_preserves_corrupt_appearance_until_an_explicit_settings_change() {
    use crate::theme::{Preset, Selection};
    let (_root, desktop) = desktop();
    let path = desktop.data_root().join("native-preferences.json");
    let original = b"{corrupt but recoverable appearance file";
    std::fs::write(&path, original).expect("damaged record");
    assert!(read_appearance(&desktop).is_err());
    let mut app = app(desktop.clone());
    app.appearance_valid = false;
    let draft = app.review.as_ref().expect("review").draft();
    let _ = app.close();
    super::persistence::persist_before_close(
        &desktop,
        Some(draft.clone()),
        app.appearance_valid.then(|| app.appearance.clone()),
    )
    .expect("close persists review");
    assert_eq!(
        std::fs::read(&path).expect("damaged preference retained"),
        original
    );
    assert_eq!(
        desktop
            .load_review()
            .expect("saved review")
            .expect("review")
            .session_id,
        draft.session_id
    );
    app.close_after_job = false;
    app.dialog_open = false;
    let _ = app.update(Message::Example);
    let _ = app.update(Message::ThemePreset(Preset::CatppuccinMocha));
    assert!(
        !app.appearance_valid,
        "example theme changes must not authorize overwriting the real preference"
    );
    let _ = app.update(Message::ConfirmReset);
    let _ = app.update(Message::ThemePreset(Preset::Light));
    assert!(app.appearance_valid);
    super::persistence::persist_before_close(
        &desktop,
        None,
        app.appearance_valid.then(|| app.appearance.clone()),
    )
    .expect("explicit choice replaces damaged preferences");
    assert_eq!(
        read_appearance(&desktop)
            .expect("new valid preference")
            .theme,
        Selection::Preset {
            preset: Preset::Light
        }
    );
}

#[test]
fn embedded_custom_themes_larger_than_the_legacy_envelope_restore() {
    use crate::theme::Selection;
    let (_root, desktop) = desktop();
    let appearance = crate::settings::Appearance {
        theme: Selection::Custom {
            name: "Long authoring notes".into(),
            source: format!("version=1\nbase='dark'\n#{}", "theme notes ".repeat(600)),
        },
        ..crate::settings::Appearance::default()
    };
    super::persistence::write_appearance(&desktop, &appearance).expect("custom preference");
    let restored = read_appearance(&desktop).expect("restore full embedded custom definition");
    assert_eq!(restored.theme, appearance.theme);
}

#[test]
fn the_example_preview_owns_its_cover_without_changing_the_saved_review() {
    let (_root, desktop) = desktop();
    let mut app = app(desktop.clone());
    let original = app.review.as_ref().expect("real review").draft();
    persist_review(&desktop, original.clone()).expect("save real review");
    let _ = app.update(Message::Example);
    assert!(app.example);
    assert!(!app.can_export(), "sample audio must remain disabled");
    let (asset, path) = app.cover.as_ref().expect("sample cover");
    assert!(path.is_file(), "sample cover owns a real local image");
    assert_eq!(
        asset.id,
        audio_engine::export::hash_file(path).expect("image digest")
    );
    assert_eq!(
        app.review
            .as_ref()
            .expect("sample review")
            .preferences
            .shared_artwork_asset
            .as_deref(),
        Some(asset.id.as_str())
    );
    app.prepare_preview().expect("sample export plan");
    assert!(app
        .preview
        .as_ref()
        .expect("file preview")
        .files
        .iter()
        .any(|name| name == "cover.jpg"));
    let _ = app.update(Message::ConfirmReset);
    assert!(!app.example);
    assert_eq!(
        serde_json::to_value(app.review.as_ref().expect("restored review").draft())
            .expect("restored snapshot"),
        serde_json::to_value(&original).expect("original snapshot")
    );
    assert_eq!(
        serde_json::to_value(
            desktop
                .load_review()
                .expect("read saved review")
                .expect("saved draft")
        )
        .expect("saved snapshot"),
        serde_json::to_value(&original).expect("original snapshot")
    );
}

#[test]
fn save_shortcut_applies_the_open_metadata_form_without_previewing_stale_tags() {
    use crate::editor::{Action, Field, Kind};
    let (_root, desktop) = desktop();
    let mut app = app(desktop);
    let _ = app.update(Message::OpenEditor(Kind::Track(0)));
    let _ = app.update(Message::Editor(Action::Input(
        Field::Title,
        "Corrected title".into(),
    )));
    let _ = app.update(Message::Preview);
    assert_eq!(app.panel, Panel::Workspace);
    assert!(app.editor.is_none());
    assert!(
        app.preview.is_none(),
        "the shortcut applies the form before any export preview"
    );
    assert_eq!(
        app.review.as_ref().expect("review").inspection.tracks[0]
            .metadata
            .title,
        "Corrected title"
    );
    assert_eq!(
        app.save_pending, 1,
        "the edited review is queued for persistence"
    );
}

#[test]
fn invalid_metadata_stays_open_and_preserves_the_review_when_the_save_shortcut_is_used() {
    use crate::editor::{Action, Field, Kind};
    let (_root, desktop) = desktop();
    let mut app = app(desktop);
    let original =
        serde_json::to_value(app.review.as_ref().expect("review").draft()).expect("snapshot");
    let _ = app.update(Message::OpenEditor(Kind::Track(0)));
    let _ = app.update(Message::Editor(Action::Input(Field::Track, "1.5".into())));
    let _ = app.update(Message::Preview);
    assert_eq!(app.panel, Panel::Editor);
    assert!(app.editor.as_ref().expect("form preserved").error.is_some());
    assert_eq!(
        serde_json::to_value(app.review.as_ref().expect("review preserved").draft())
            .expect("snapshot"),
        original
    );
    assert!(app.preview.is_none());
    assert_eq!(app.save_pending, 0);
}

#[test]
fn navigation_shortcuts_keep_an_unsaved_editor_and_theme_changes_keep_its_values() {
    use crate::editor::{Action, Field, Kind};
    let (_root, desktop) = desktop();
    let mut app = app(desktop);
    let _ = app.update(Message::OpenEditor(Kind::Track(0)));
    let _ = app.update(Message::Editor(Action::Input(
        Field::Title,
        "Keep this edit".into(),
    )));
    let _ = app.update(Message::OpenPanel(Panel::Settings));
    assert_eq!(app.panel, Panel::Editor);
    assert_eq!(
        app.update(Message::FocusSource).units(),
        0,
        "a hidden source field must not receive focus"
    );
    let _ = app.update(Message::ThemePreset(crate::theme::Preset::CatppuccinMocha));
    assert_eq!(app.panel, Panel::Editor);
    let _ = app.update(Message::Preview);
    assert_eq!(
        app.review.as_ref().expect("review").inspection.tracks[0]
            .metadata
            .title,
        "Keep this edit"
    );
}

#[test]
fn preview_and_source_shortcuts_do_not_leave_other_modal_contexts() {
    let (_root, desktop) = desktop();
    let mut app = app(desktop);
    for panel in [
        Panel::Settings,
        Panel::Appearance,
        Panel::Crop,
        Panel::Discard,
        Panel::Close,
    ] {
        app.panel = panel;
        assert_eq!(app.update(Message::Preview).units(), 0);
        assert_eq!(app.panel, panel);
        assert!(app.preview.is_none());
        assert_eq!(app.update(Message::FocusSource).units(), 0);
    }
    app.panel = Panel::Crop;
    let _ = app.update(Message::OpenPanel(Panel::Settings));
    assert_eq!(app.panel, Panel::Crop);
    app.panel = Panel::Options;
    let _ = app.update(Message::Preview);
    assert_eq!(
        app.panel,
        Panel::Preview,
        "export options may open their file preview"
    );
}

#[test]
fn cancelling_a_destination_keeps_the_captured_preview_ready_to_retry() {
    use audio_engine::OutputKind;
    for kind in [OutputKind::File, OutputKind::Folder, OutputKind::Zip] {
        let (_root, desktop) = desktop();
        let mut app = app(desktop);
        let review = app.review.as_mut().expect("review");
        review.selected = [0].into_iter().collect();
        review.preferences.output_kind = kind;
        app.prepare_preview().expect("valid captured preview");
        let plan = serde_json::to_value(app.pending_plan.as_ref()).expect("plan snapshot");
        let preview = serde_json::to_value(app.preview.as_ref()).expect("preview snapshot");
        let identity = app.review.as_ref().expect("review").identity();
        app.dialog_open = true;

        assert_eq!(app.update(Message::Destination(None)).units(), 0);

        assert!(
            !app.dialog_open,
            "cancelled destination releases the dialog"
        );
        assert_eq!(app.panel, Panel::Preview);
        assert_eq!(
            serde_json::to_value(app.pending_plan.as_ref()).expect("retained plan"),
            plan
        );
        assert_eq!(
            serde_json::to_value(app.preview.as_ref()).expect("retained preview"),
            preview
        );
        assert_eq!(app.review.as_ref().expect("review").identity(), identity);
        assert_eq!(app.save_pending, 0);
        assert!(
            app.update(Message::Save).units() > 0,
            "Save can reopen the destination dialog"
        );
        assert!(app.dialog_open);
    }
}

#[test]
fn editing_the_output_name_keeps_tags_selection_and_other_preferences_unchanged() {
    use crate::editor::{Action, Field, Kind};
    let (_root, desktop) = desktop();
    let mut app = app(desktop);
    let review = app.review.as_ref().expect("review");
    let inspection = serde_json::to_value(&review.inspection).expect("original tags");
    let selected = review.selected.clone();
    let identity = review.identity();
    let mut preferences = serde_json::to_value(&review.preferences).expect("original preferences");
    preferences["title"] = serde_json::json!("Library copy");
    let _ = app.update(Message::OpenEditor(Kind::OutputName));
    let _ = app.update(Message::Editor(Action::Input(
        Field::Title,
        "  Library copy  ".into(),
    )));
    let _ = app.update(Message::Editor(Action::Apply));

    let review = app.review.as_ref().expect("edited review");
    assert_eq!(
        serde_json::to_value(&review.inspection).expect("retained tags"),
        inspection
    );
    assert_eq!(
        serde_json::to_value(&review.preferences).expect("edited preferences"),
        preferences
    );
    assert_eq!(review.selected, selected);
    assert_eq!(review.identity().session_id, identity.session_id);
    assert_eq!(review.identity().revision, identity.revision + 1);
    assert_eq!(app.panel, Panel::Workspace);
    assert!(app.editor.is_none());
    assert_eq!(app.save_pending, 1);
}

#[test]
fn an_empty_output_name_stays_open_without_changing_the_review() {
    use crate::editor::{Action, Field, Kind};
    let (_root, desktop) = desktop();
    let mut app = app(desktop);
    let original =
        serde_json::to_value(app.review.as_ref().expect("review").draft()).expect("snapshot");
    let _ = app.update(Message::OpenEditor(Kind::OutputName));
    let _ = app.update(Message::Editor(Action::Input(
        Field::Title,
        "  \n  ".into(),
    )));
    let _ = app.update(Message::Editor(Action::Apply));

    assert_eq!(app.panel, Panel::Editor);
    assert!(app.editor.as_ref().expect("form retained").error.is_some());
    assert_eq!(
        serde_json::to_value(app.review.as_ref().expect("review").draft())
            .expect("unchanged snapshot"),
        original
    );
    assert_eq!(app.save_pending, 0);
}

#[test]
fn preparing_a_remote_cover_for_crop_does_not_commit_artwork_or_metadata() {
    let (_root, desktop) = desktop();
    let mut app = app(desktop);
    let original =
        serde_json::to_value(app.review.as_ref().expect("review").draft()).expect("snapshot");
    let identity = app.review.as_ref().expect("review").identity();
    let (asset, file) = example::cover().expect("isolated cover");
    let path = file.path().to_path_buf();
    app.dialog_open = true;

    let _ = app.update(Message::RemoteCoverImported(
        identity,
        Ok((asset.clone(), path.clone())),
    ));

    assert!(!app.dialog_open);
    assert_eq!(app.panel, Panel::Crop);
    let (staged, staged_path) = app.crop_source.as_ref().expect("staged remote cover");
    assert_eq!(staged.id, asset.id);
    assert_eq!(*staged_path, path);
    assert_eq!(app.crop_size, asset.width.min(asset.height).to_string());
    assert!(
        app.cover.is_none(),
        "staging must not choose an export cover"
    );
    assert_eq!(
        serde_json::to_value(app.review.as_ref().expect("review").draft())
            .expect("unchanged snapshot"),
        original
    );
    assert_eq!(app.save_pending, 0);
}

#[test]
fn closing_a_staged_crop_preserves_the_chosen_cover_and_review_without_saving() {
    let (_root, desktop) = desktop();
    let mut app = app(desktop);
    let (mut original_asset, original_file) = example::cover().expect("original cover");
    original_asset.id = "original-cover".into();
    let original_path = original_file.path().to_path_buf();
    app.review
        .as_mut()
        .expect("review")
        .preferences
        .shared_artwork_asset = Some(original_asset.id.clone());
    app.cover = Some((original_asset.clone(), original_path.clone()));
    let original =
        serde_json::to_value(app.review.as_ref().expect("review").draft()).expect("snapshot");
    let (staged, staged_file) = example::cover().expect("staged cover");
    app.crop_source = Some((staged, staged_file.path().to_path_buf()));
    app.crop_dragging = true;
    app.panel = Panel::Crop;

    let _ = app.update(Message::ClosePanel);

    assert_eq!(app.panel, Panel::Workspace);
    assert!(app.crop_source.is_none());
    assert!(!app.crop_dragging);
    let (chosen, path) = app.cover.as_ref().expect("original cover retained");
    assert_eq!(chosen.id, original_asset.id);
    assert_eq!(*path, original_path);
    assert_eq!(
        serde_json::to_value(app.review.as_ref().expect("review").draft())
            .expect("unchanged snapshot"),
        original
    );
    assert_eq!(app.save_pending, 0);
}

#[test]
fn a_remote_cover_completion_cannot_stage_artwork_for_a_newer_review() {
    let (_root, desktop) = desktop();
    let mut app = app(desktop);
    let identity = app.review.as_ref().expect("review").identity();
    app.review
        .as_mut()
        .expect("review")
        .changed()
        .expect("newer edit");
    let original =
        serde_json::to_value(app.review.as_ref().expect("review").draft()).expect("newer snapshot");
    let (asset, file) = example::cover().expect("isolated cover");
    app.dialog_open = true;

    let _ = app.update(Message::RemoteCoverImported(
        identity,
        Ok((asset, file.path().to_path_buf())),
    ));

    assert!(!app.dialog_open);
    assert_eq!(app.panel, Panel::Workspace);
    assert!(app.crop_source.is_none());
    assert!(app.cover.is_none());
    assert_eq!(
        serde_json::to_value(app.review.as_ref().expect("review").draft())
            .expect("retained snapshot"),
        original
    );
    assert_eq!(app.save_pending, 0);
}

#[test]
fn source_cover_failures_are_fenced_and_a_successful_retry_clears_the_error() {
    let (_root, desktop) = desktop();
    let mut app = app(desktop);
    let current = "https://i.ytimg.com/current.jpg";
    app.review
        .as_mut()
        .expect("review")
        .preferences
        .shared_artwork = Some(current.into());
    let stale = "https://i.ytimg.com/previous.jpg";
    let _ = app.update(Message::SourceCover(
        stale.into(),
        Err("stale failure".into()),
    ));
    assert!(app.source_cover_error.is_none());
    let _ = app.update(Message::SourceCover(
        current.into(),
        Err("network unavailable".into()),
    ));
    assert_eq!(
        app.source_cover_error.as_deref(),
        Some("network unavailable")
    );
    let handle = crate::artwork::example_cover();
    let _ = app.update(Message::SourceCover(stale.into(), Ok(handle.clone())));
    assert!(app.source_cover.is_none());
    assert_eq!(
        app.source_cover_error.as_deref(),
        Some("network unavailable")
    );
    let _ = app.update(Message::SourceCover(current.into(), Ok(handle.clone())));
    assert_eq!(app.source_cover, Some(handle));
    assert!(app.source_cover_error.is_none());
    let _ = app.update(Message::SourceCover(
        stale.into(),
        Err("late stale failure".into()),
    ));
    assert!(app.source_cover_error.is_none());
}

#[test]
fn preview_cover_callbacks_follow_the_captured_plan_and_retry_clears_the_error() {
    let (_root, desktop) = desktop();
    let mut app = app(desktop);
    let current = "https://i.ytimg.com/preview.jpg";
    app.review
        .as_mut()
        .expect("review")
        .preferences
        .shared_artwork = Some(current.into());
    app.prepare_preview().expect("captured plan");
    // Editing the ordinary review after capture must not retarget its preview.
    app.review
        .as_mut()
        .expect("review")
        .preferences
        .shared_artwork = Some("https://i.ytimg.com/new-review.jpg".into());
    let stale = "https://i.ytimg.com/previous.jpg";
    let _ = app.update(Message::PreviewCover(
        stale.into(),
        Err("stale failure".into()),
    ));
    assert!(app.preview_cover_error.is_none());
    let _ = app.update(Message::PreviewCover(
        current.into(),
        Err("network unavailable".into()),
    ));
    assert_eq!(
        app.preview_cover_error.as_deref(),
        Some("network unavailable")
    );
    let handle = crate::artwork::example_cover();
    let _ = app.update(Message::PreviewCover(stale.into(), Ok(handle.clone())));
    assert!(app.preview_cover.is_none());
    let _ = app.update(Message::PreviewCover(current.into(), Ok(handle.clone())));
    assert_eq!(app.preview_cover, Some(handle));
    assert!(app.preview_cover_error.is_none());
    let _ = app.update(Message::PreviewCover(
        stale.into(),
        Err("late stale failure".into()),
    ));
    assert!(app.preview_cover_error.is_none());
}

#[test]
fn a_missing_owned_cover_blocks_export_until_artwork_is_changed() {
    let (_root, desktop) = desktop();
    let mut app = app(desktop);
    app.review
        .as_mut()
        .expect("review")
        .preferences
        .shared_artwork_asset = Some("missing-cover".into());
    assert!(
        !app.can_preview(),
        "missing selected cover needs recovery before export"
    );
    app.review
        .as_mut()
        .expect("review")
        .preferences
        .artwork_mode = audio_engine::ArtworkMode::None;
    assert!(
        app.can_preview(),
        "explicit no-artwork mode does not require the unavailable asset"
    );
    app.review
        .as_mut()
        .expect("review")
        .preferences
        .artwork_mode = audio_engine::ArtworkMode::PerTrack;
    assert!(
        app.can_preview(),
        "playlist track covers do not use the shared asset"
    );
}
