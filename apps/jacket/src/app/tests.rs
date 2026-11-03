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
    let dark = app.appearance.dark;
    let _ = app.update(Message::Example);
    let _ = app.update(Message::Toggle(0, false));
    let _ = app.update(Message::Dark(!dark));
    assert_eq!(app.save_pending, 0);
    let _ = app.update(Message::ConfirmReset);
    assert!(!app.example);
    assert_eq!(app.appearance.dark, dark);
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
    let _ = app.update(Message::Dark(true));
    assert!(app.appearance_saving);
    assert!(!app.appearance_dirty);
    let _ = app.update(Message::Dark(false));
    assert!(app.appearance_dirty);
    let _ = app.update(Message::AppearanceSaved(Ok(())));
    assert!(app.appearance_saving);
    assert!(!app.appearance_dirty);
    assert!(!app.appearance.dark);
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
        vec![b' '; 4097],
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
