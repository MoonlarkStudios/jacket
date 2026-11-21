//! Collection editing and navigation through occurrence-indexed rows.
use super::state::TrackPositionEdit;
use super::{App, Message, Panel};
use crate::editor::Kind;
use crate::track_list;
use iced::widget::operation;
use iced::Task;

impl App {
    pub fn track_position_target(&self) -> Option<usize> {
        let track = self.track_position.as_ref()?;
        let count = self.review.as_ref()?.inspection.tracks.len();
        let position = track.position.trim().parse::<usize>().ok()?;
        (1..=count).contains(&position).then(|| position - 1)
    }

    pub(super) fn choose_track_position(&mut self, index: usize) -> Task<Message> {
        if self
            .review
            .as_ref()
            .is_some_and(|review| index < review.inspection.tracks.len())
        {
            self.track_position = Some(TrackPositionEdit {
                index,
                position: (index + 1).to_string(),
            });
            self.arranged_track = Some(index);
            operation::focus("track-position").chain(operation::select_all("track-position"))
        } else {
            Task::none()
        }
    }

    pub(super) fn cancel_track_position(&mut self) -> Task<Message> {
        self.track_position.take().map_or_else(Task::none, |edit| {
            operation::focus(track_list::position_id(edit.index))
        })
    }

    pub(super) fn editor_return_focus(&self) -> Option<String> {
        match self.panel {
            Panel::SongEditor => Some("song-edit-tags".into()),
            Panel::CollectionEditor | Panel::Editor => match self.editor.as_ref()?.kind {
                Kind::Track(index) => Some(track_list::edit_id(index)),
                Kind::Album => Some("collection-edit-album".into()),
                Kind::PlaylistName => Some("collection-rename".into()),
                Kind::Bulk => Some("collection-edit-selected".into()),
                Kind::OutputName => None,
            },
            _ => None,
        }
    }

    pub(super) fn reveal_track(&mut self, index: usize) {
        let offset = self.track_scroll;
        let height = self.track_viewport_height.max(track_list::ROW_HEIGHT);
        let top = index as f32 * track_list::ROW_HEIGHT;
        let bottom = top + track_list::ROW_HEIGHT;
        self.track_scroll = offset.clamp((bottom - height).max(0.0), top);
    }

    pub(super) fn restore_collection_anchor(&mut self) {
        if let Some(offset) = self.collection_editor_anchor.take() {
            self.track_scroll = offset;
        }
    }

    pub(super) fn move_review_track(&mut self, from: usize, to: usize) -> Task<Message> {
        let Some(review) = &mut self.review else {
            return Task::none();
        };
        if from == to {
            return Task::none();
        }
        if let Err(error) = review.move_track(from, to) {
            self.error = Some(error);
            return Task::none();
        }
        let up = to < from;
        let focus = if (up && to == 0) || (!up && to + 1 == review.inspection.tracks.len()) {
            track_list::move_id(to, !up)
        } else {
            track_list::move_id(to, up)
        };
        self.arranged_track = Some(to);
        self.reveal_track(to);
        self.autosave().chain(operation::focus(focus))
    }

    pub(super) fn undo_review_move(&mut self) -> Task<Message> {
        let Some(review) = &mut self.review else {
            return Task::none();
        };
        match review.undo_move() {
            Ok(Some(index)) => {
                self.arranged_track = Some(index);
                self.reveal_track(index);
                self.autosave()
                    .chain(operation::focus(track_list::move_id(index, index > 0)))
            }
            Ok(None) => Task::none(),
            Err(error) => {
                self.error = Some(error);
                Task::none()
            }
        }
    }
}
