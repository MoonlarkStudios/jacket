use super::{persistence, App, CatalogAction, Message, Panel};
use audio_engine::ProgressEvent;
use iced::advanced::widget::{operate, operation as widget_operation, Id, Operation};
use iced::{widget::operation, Task};
use jacket_core::CompletionIdentity;
use jacket_desktop::{Download, Event, Job};
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

pub(crate) const DOWNLOADS_PANEL_ID: &str = "downloads-panel";
pub(crate) const DOWNLOADS_LIST_ID: &str = "downloads-list";

#[derive(Clone, Debug)]
pub enum DownloadAction {
    Toggle,
    OpenPage,
    Close,
    Cancel(String),
    Retry(String),
    Remove(String),
    Reveal(String),
    Retried(String, Result<Box<Download>, String>),
    Removed(String, Result<(), String>),
    Revealed(Result<(), String>),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum DownloadSurface {
    #[default]
    Closed,
    Dropdown,
    Page,
}

impl DownloadSurface {
    fn initial_focus(self) -> &'static str {
        match self {
            Self::Closed => "open-downloads",
            Self::Dropdown => "downloads-open-page",
            Self::Page => "downloads-back",
        }
    }
}

#[derive(Default)]
pub struct Downloads {
    pub items: Vec<Download>,
    pub(crate) surface: DownloadSurface,
    pub error: Option<String>,
    job: Option<Job>,
    cancelling: bool,
    updating: BTreeSet<String>,
    notice: Option<(String, Instant)>,
}

impl Downloads {
    pub(super) fn example(&mut self, surface: DownloadSurface) {
        use audio_engine::{AudioFormat, AuthOptions};
        use jacket_desktop::recovery::Recovery;
        let mut plan = jacket_core::Review::new(crate::example::album(), AudioFormat::Mp3)
            .plan(AuthOptions::default())
            .expect("fictional album has a valid plan");
        for (id, title, stage) in [
            ("example-1", "Room to breathe", "queued"),
            ("example-2", "Along the River", "failed"),
            ("example-3", "The Quiet Between", "complete"),
        ] {
            plan.title = title.into();
            let mut recipe = Recovery::new(
                id.into(),
                plan.clone(),
                format!("Music/The Paper Trails/{title}"),
            );
            recipe.progress.stage = stage.into();
            if stage == "failed" {
                recipe.error =
                    Some("The connection dropped. Try again when you’re back online.".into());
            }
            self.insert(Download {
                recipe,
                result: None,
                can_reveal: false,
            });
        }
        self.surface = surface;
    }

    pub fn is_open(&self) -> bool {
        self.surface != DownloadSurface::Closed
    }

    pub(super) fn dismiss(&mut self) -> DownloadSurface {
        std::mem::take(&mut self.surface)
    }

    pub(super) fn initial_focus(&self) -> &'static str {
        self.surface.initial_focus()
    }

    pub fn active(&self) -> Option<&str> {
        self.job.as_ref().map(Job::id)
    }

    pub fn cancelling(&self) -> bool {
        self.cancelling
    }

    pub fn updating(&self, id: &str) -> bool {
        self.updating.contains(id)
    }

    pub fn writing(&self) -> bool {
        !self.updating.is_empty()
    }

    pub fn outstanding(&self) -> usize {
        self.items.iter().filter(|item| item.waiting()).count() + usize::from(self.job.is_some())
    }

    pub fn notice(&self) -> Option<&str> {
        self.notice.as_ref().map(|(label, _)| label.as_str())
    }

    pub fn status_for(&self, url: &str) -> Option<&'static str> {
        self.items
            .iter()
            .filter(|item| item.recipe.plan.source_url == url)
            .find_map(|item| {
                if self.active() == Some(item.recipe.job_id.as_str()) {
                    Some("Downloading")
                } else {
                    item.waiting().then_some("Queued")
                }
            })
    }

    fn insert(&mut self, item: Download) {
        match self
            .items
            .binary_search_by(|entry| entry.recipe.job_id.cmp(&item.recipe.job_id))
        {
            Ok(index) => self.items[index] = item,
            Err(index) => self.items.insert(index, item),
        }
    }
}

impl App {
    pub(super) fn update_downloads(&mut self, action: DownloadAction) -> Task<Message> {
        use DownloadAction as Action;
        match action {
            Action::Toggle if self.downloads.is_open() => {
                return self.update_downloads(Action::Close);
            }
            Action::Toggle if self.panel == Panel::Workspace => {
                self.catalog.dismiss_song_menu();
                self.downloads.surface = DownloadSurface::Dropdown;
                return operation::focus(self.downloads.initial_focus());
            }
            Action::OpenPage if self.panel == Panel::Workspace => {
                self.catalog.dismiss_song_menu();
                self.downloads.surface = DownloadSurface::Page;
                return operation::focus(self.downloads.initial_focus());
            }
            Action::Close if self.downloads.is_open() => {
                let previous = self.downloads.dismiss();
                let focus = operation::focus(self.downloads.initial_focus());
                return if previous == DownloadSurface::Page {
                    Task::batch([self.restore_discovery_scroll(), focus])
                } else {
                    focus
                };
            }
            Action::Cancel(id) if self.downloads.active() == Some(id.as_str()) => {
                self.cancel_download();
            }
            Action::Retry(id) => return self.change_download(id, false),
            Action::Remove(id) => return self.change_download(id, true),
            Action::Reveal(id) => {
                if let Some(desktop) = self.desktop.clone() {
                    self.downloads.error = None;
                    return persistence::background(
                        move || desktop.reveal_download(&id),
                        |result| Message::Downloads(Action::Revealed(result)),
                    );
                }
            }
            Action::Retried(id, result) => {
                self.downloads.updating.remove(&id);
                match result {
                    Ok(item) => self.downloads.insert(*item),
                    Err(error) => self.downloads.error = Some(error),
                }
                if self.downloads.is_open() {
                    return operation::focus(self.downloads.initial_focus());
                }
            }
            Action::Removed(id, result) => {
                self.downloads.updating.remove(&id);
                match result {
                    Ok(()) => self.downloads.items.retain(|item| item.recipe.job_id != id),
                    Err(error) => self.downloads.error = Some(error),
                }
                if self.downloads.is_open() {
                    return operation::focus(self.downloads.initial_focus());
                }
            }
            Action::Revealed(Err(error)) => self.downloads.error = Some(error),
            _ => {}
        }
        Task::none()
    }

    fn change_download(&mut self, id: String, remove: bool) -> Task<Message> {
        use DownloadAction as Action;
        let Some(desktop) = self.desktop.clone() else {
            return Task::none();
        };
        if self.close_after_job
            || self.downloads.active() == Some(id.as_str())
            || !self
                .downloads
                .items
                .iter()
                .any(|item| item.recipe.job_id == id && (remove || item.retryable()))
            || !self.downloads.updating.insert(id.clone())
        {
            return Task::none();
        }
        self.downloads.error = None;
        let stored_id = id.clone();
        if remove {
            persistence::background(
                move || desktop.remove_download(&stored_id),
                move |result| Message::Downloads(Action::Removed(id, result)),
            )
        } else {
            persistence::background(
                move || desktop.retry_download(&stored_id).map(Box::new),
                move |result| Message::Downloads(Action::Retried(id, result)),
            )
        }
    }

    pub(super) fn download_queued(
        &mut self,
        identity: CompletionIdentity,
        result: Result<Box<Download>, String>,
    ) -> Task<Message> {
        self.dialog_open = false;
        self.notice = None;
        match result {
            Ok(item) => {
                let destination = if item.recipe.uses_music_folder() {
                    " · Music folder"
                } else {
                    ""
                };
                let label = format!("Queued {}{destination}", item.recipe.plan.title);
                self.downloads.notice = Some((label, Instant::now() + Duration::from_secs(8)));
                self.downloads.insert(*item);
                if self
                    .review
                    .as_ref()
                    .is_some_and(|review| review.identity() == identity)
                {
                    let keep_album = self.catalog.reviewed_album_on_page(&identity);
                    let scroll = self.track_scroll;
                    self.clear_review();
                    self.panel = Panel::Workspace;
                    if self.close_after_job {
                        return self.close();
                    }
                    if keep_album {
                        self.track_scroll = scroll;
                        return operation::focus("review-album");
                    }
                    return self.update_catalog(CatalogAction::Browse);
                }
            }
            Err(error) => self.error = Some(format!("Couldn’t queue your download. {error}")),
        }
        if self.close_after_job {
            self.close()
        } else {
            Task::none()
        }
    }

    pub(super) fn cancel_download(&mut self) {
        if let Some(job) = &self.downloads.job {
            self.downloads.cancelling = true;
            job.cancel();
        }
    }

    pub(super) fn tick_downloads(&mut self) {
        if self
            .downloads
            .notice
            .as_ref()
            .is_some_and(|(_, expires)| Instant::now() >= *expires)
        {
            self.downloads.notice = None;
        }
        if let Some(job) = &mut self.downloads.job {
            let entry = self
                .downloads
                .items
                .iter_mut()
                .find(|item| item.recipe.job_id == job.id());
            if let Some(item) = entry {
                for _ in 0..64 {
                    match job.poll() {
                        Some(Event::Progress(progress)) => item.recipe.progress = progress,
                        Some(Event::DownloadExported { result, can_reveal }) => {
                            item.recipe.progress.stage = "complete".into();
                            item.recipe.progress.completed = item.recipe.progress.total;
                            item.recipe.output_path.clone_from(&result.output_path);
                            item.can_reveal = can_reveal;
                            item.result = Some(result);
                        }
                        Some(Event::Failed(error)) => {
                            item.recipe.progress.stage = if self.downloads.cancelling {
                                "cancelled"
                            } else {
                                "failed"
                            }
                            .into();
                            item.recipe.error = Some(error);
                        }
                        _ => break,
                    }
                }
            }
            if !job.is_finished() {
                return;
            }
            self.downloads.job = None;
            self.downloads.cancelling = false;
        }
        if self.close_after_job || self.loading || self.job.is_some() || self.example {
            return;
        }
        let Some(desktop) = &self.desktop else {
            return;
        };
        let Some(item) =
            self.downloads.items.iter_mut().find(|item| {
                item.waiting() && !self.downloads.updating.contains(&item.recipe.job_id)
            })
        else {
            return;
        };
        match desktop.start_download(&item.recipe.job_id) {
            Ok(job) => {
                item.recipe.progress = ProgressEvent {
                    stage: "starting".into(),
                    message: "Preparing…".into(),
                    completed: 0,
                    total: item.recipe.plan.tracks.len(),
                };
                self.downloads.job = Some(job);
            }
            Err(error) => {
                item.recipe.progress.stage = "failed".into();
                item.recipe.error = Some(error);
            }
        }
    }
}

pub(super) fn focus_next(forward: bool) -> Task<Message> {
    operate(DownloadFocus {
        forward,
        ..DownloadFocus::default()
    })
    .discard()
    .chain(super::catalog_focus::reveal_in(DOWNLOADS_LIST_ID))
}

#[derive(Default)]
struct DownloadFocus {
    forward: bool,
    inside: bool,
    entering: bool,
    controls: Vec<Id>,
    focused: Option<usize>,
}

impl Operation for DownloadFocus {
    fn container(&mut self, id: Option<&Id>, _bounds: iced::Rectangle) {
        self.entering = id == Some(&Id::new(DOWNLOADS_PANEL_ID));
    }

    fn traverse(&mut self, visit: &mut dyn FnMut(&mut dyn Operation)) {
        let parent = self.inside;
        self.inside |= std::mem::take(&mut self.entering);
        visit(self);
        self.inside = parent;
    }

    fn focusable(
        &mut self,
        id: Option<&Id>,
        _bounds: iced::Rectangle,
        state: &mut dyn widget_operation::Focusable,
    ) {
        if let Some(id) = id.filter(|_| self.inside) {
            if state.is_focused() {
                self.focused = Some(self.controls.len());
            }
            self.controls.push(id.clone());
        }
    }

    fn finish(&self) -> widget_operation::Outcome<()> {
        let count = self.controls.len();
        if count == 0 {
            return widget_operation::Outcome::None;
        }
        let next = match (self.focused, self.forward) {
            (Some(current), true) => (current + 1) % count,
            (Some(current), false) => (current + count - 1) % count,
            (None, true) => 0,
            (None, false) => count - 1,
        };
        widget_operation::Outcome::Chain(Box::new(widget_operation::focusable::focus(
            self.controls[next].clone(),
        )))
    }
}
