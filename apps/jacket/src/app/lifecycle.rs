use super::persistence;
use super::state::SuspendedReview;
use super::{App, Message, Panel};
use iced::Task;
use jacket_core::Review;

impl App {
    pub(super) fn change_review(
        &mut self,
        change: impl FnOnce(&mut Review) -> Result<(), String>,
    ) -> Task<Message> {
        if let Some(review) = &mut self.review {
            if let Err(error) = change(review) {
                self.error = Some(error);
                return Task::none();
            }
        }
        self.autosave()
    }

    pub(super) fn autosave(&mut self) -> Task<Message> {
        if self.example {
            return self.restore_scroll();
        }
        if let (Some(desktop), Some(review)) = (self.desktop.clone(), &self.review) {
            let draft = review.draft();
            self.save_pending += 1;
            let identity = review.identity();
            return Task::batch([
                persistence::background(
                    move || persistence::persist_review(&desktop, draft),
                    move |result| Message::Saved(identity, result),
                ),
                self.restore_scroll(),
            ]);
        }
        Task::none()
    }

    pub(super) fn prepare_preview(&mut self) -> Result<(), String> {
        let review = self
            .review
            .as_ref()
            .ok_or("Find a song or collection first.")?;
        let plan = review.plan(self.auth.clone())?;
        self.preview = Some(audio_engine::preview_export(&plan).map_err(|e| e.to_string())?);
        self.pending_plan = Some(plan);
        self.panel = Panel::Preview;
        Ok(())
    }

    pub(super) fn close(&mut self) -> Task<Message> {
        self.close_after_job = true;
        if self.job.is_some()
            || self.dialog_open
            || self.loading
            || self.save_pending > 0
            || self.appearance_saving
        {
            return Task::none();
        }
        self.dialog_open = true;
        if !self.example {
            if let Some(desktop) = self.desktop.clone() {
                let draft = self.review.as_ref().map(Review::draft);
                let appearance = self.appearance.clone();
                return persistence::background(
                    move || {
                        if let Some(draft) = draft {
                            persistence::persist_review(&desktop, draft)?;
                        }
                        persistence::write_appearance(&desktop, &appearance)
                    },
                    Message::CloseReady,
                );
            }
        }
        Task::done(Message::CloseReady(Ok(())))
    }

    pub(super) fn suspend_review(&mut self) -> SuspendedReview {
        self.preview = None;
        self.pending_plan = None;
        SuspendedReview {
            review: self.review.take(),
            url: self.url.clone(),
            cover: self.cover.take(),
            source_cover: self.source_cover.take(),
            track_scroll: self.track_scroll,
            result: self.result.take(),
            appearance: self.appearance.clone(),
        }
    }

    pub(super) fn restore_review(&mut self, previous: SuspendedReview) {
        self.review = previous.review;
        self.url = previous.url;
        self.cover = previous.cover;
        self.source_cover = previous.source_cover;
        self.track_scroll = previous.track_scroll;
        self.result = previous.result;
        self.appearance = previous.appearance;
        self.preview = None;
        self.pending_plan = None;
    }

    pub(super) fn clear_review(&mut self) {
        self.review = None;
        self.url.clear();
        self.cover = None;
        self.source_cover = None;
        self.track_scroll = 0.0;
        self.result = None;
        self.preview = None;
        self.pending_plan = None;
        self.save_failed = false;
    }

    pub(super) fn finish_resumed_review(&mut self, success: bool) {
        if let Some((mut previous, identity)) = self.resumed_previous.take() {
            if success
                && previous.review.as_ref().is_some_and(|review| {
                    identity.as_ref().is_some_and(|exported| {
                        review.identity().session_id == exported.session_id
                            && review.identity().revision <= exported.revision
                    })
                })
            {
                previous.review = None;
                previous.cover = None;
                previous.source_cover = None;
            }
            self.restore_review(previous);
        } else if success {
            self.clear_review();
        }
    }

    pub(super) fn save_appearance(&mut self) -> Task<Message> {
        if self.example {
            return Task::none();
        }
        self.appearance_dirty = true;
        self.flush_appearance()
    }

    pub(super) fn flush_appearance(&mut self) -> Task<Message> {
        if self.appearance_saving || !self.appearance_dirty || self.example {
            return Task::none();
        }
        if let Some(desktop) = self.desktop.clone() {
            self.appearance_saving = true;
            self.appearance_dirty = false;
            let prefs = self.appearance.clone();
            return persistence::background(
                move || persistence::write_appearance(&desktop, &prefs),
                Message::AppearanceSaved,
            );
        }
        Task::none()
    }
}
