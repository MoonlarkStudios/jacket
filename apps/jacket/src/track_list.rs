//! Fixed-height, occurrence-indexed track rows with bounded widget creation.
use crate::app::{App, Message, Panel};
use crate::editor::Kind;
use crate::theme;
use crate::view::focus_button;
use audio_engine::Inspection;
use iced::widget::text::Wrapping;
use iced::widget::{
    button, checkbox, column, container, keyed, responsive, row, scrollable, sensor, space, text,
    tooltip,
};
use iced::{Alignment, Element, Length};
use jacket_core::Review;
use std::borrow::Cow;
use std::ops::Range;

pub const ROW_HEIGHT: f32 = 52.0;
const OVERSCAN: usize = 3;

#[derive(Clone, Copy, PartialEq, Eq)]
enum RowKey {
    Before,
    Track(usize),
    After,
}

pub fn edit_id(index: usize) -> String {
    format!("edit-track-{index}")
}

pub fn move_id(index: usize, up: bool) -> String {
    let direction = if up { "up" } else { "down" };
    format!("move-track-{index}-{direction}")
}

pub fn position_id(index: usize) -> String {
    format!("position-track-{index}")
}

pub fn view(app: &App, compact: bool) -> Element<'_, Message> {
    let Some(review) = &app.review else {
        return space().into();
    };
    table(app, &review.inspection, Some(review), compact)
}

pub fn album_preview<'a>(
    app: &'a App,
    inspection: &'a Inspection,
    compact: bool,
) -> Element<'a, Message> {
    table(app, inspection, None, compact)
}

fn table<'a>(
    app: &'a App,
    inspection: &'a Inspection,
    review: Option<&'a Review>,
    compact: bool,
) -> Element<'a, Message> {
    let p = app.presentation();
    let arranging = review.is_some() && app.panel == Panel::TrackOrder;
    let enabled = review.is_some() && !app.busy() && app.panel == Panel::Workspace;
    let show_time = !compact || (app.catalog.visible && app.catalog.album().is_some());
    let count = inspection.tracks.len();
    let mut toolbar = row![text("Track list").size(14).font(crate::style::semibold())];
    if let Some(review) = review {
        let all_selected = count > 0 && review.selected.len() == count;
        toolbar = row![
            focus_button::wrap_control(
                "review-select-all",
                checkbox(all_selected)
                    .size(14)
                    .label("Select all")
                    .text_size(12)
                    .on_toggle_maybe((enabled && count > 0).then_some(Message::SelectAll))
                    .into(),
                (enabled && count > 0).then_some(Message::SelectAll(!all_selected)),
                p.colors.accent,
            ),
            text(format!("{} selected", review.selected.len()))
                .size(12)
                .color(p.colors.text_muted),
            space::horizontal(),
            focus_button::wrap(
                "collection-edit-selected",
                button(text("Edit selected").size(12))
                    .padding([5, 8])
                    .style(p.quiet()),
                (enabled && !review.selected.is_empty()).then_some(Message::OpenEditor(Kind::Bulk)),
                p.colors.accent,
            ),
            focus_button::wrap(
                "open-track-order",
                button(text("Track order…").size(12))
                    .padding([5, 8])
                    .style(p.secondary()),
                (enabled && count > 1).then_some(Message::OpenPanel(Panel::TrackOrder)),
                p.colors.accent,
            ),
        ]
        .spacing(12)
        .align_y(Alignment::Center);
        if review.can_undo_bulk() && !arranging {
            toolbar = toolbar.push(
                button(text("Undo edit").size(12))
                    .padding([5, 8])
                    .on_press_maybe(enabled.then_some(Message::UndoBulk))
                    .style(p.quiet()),
            );
        }
    }
    if arranging {
        toolbar = row![
            text("Track order").size(14).font(crate::style::semibold()),
            text(app.arranged_track.map_or_else(
                || "Use the arrows or choose Move…".into(),
                |index| format!("Position {} of {count}", index + 1)
            ))
            .size(12)
            .color(p.colors.text_muted),
        ]
        .spacing(12)
        .align_y(Alignment::Center);
    }
    let editor_track = app.editor.as_ref().and_then(|editor| match editor.kind {
        Kind::Track(index) => Some(index),
        _ => None,
    });
    let viewport = responsive(move |size| {
        let range = visible_rows(
            count,
            app.track_scroll,
            (size.height - 26.0).max(ROW_HEIGHT),
        );
        let artist_width = (size.width * 0.25).clamp(90.0, 170.0);
        let number_width = if arranging && count >= 10_000 { 36 } else { 28 };
        let mut headings = row![
            space().width(18),
            text("#").size(10).width(number_width),
            text(if compact { "Track" } else { "Title" })
                .size(10)
                .width(Length::Fill)
        ]
        .spacing(10)
        .align_y(Alignment::Center);
        if !compact {
            headings = headings.push(text("Artist").size(10).width(artist_width));
        }
        if show_time {
            headings = headings.push(text("Time").size(10).width(42));
        }
        headings = headings.push(space().width(if arranging { 124 } else { 40 }));
        let header = container(headings)
            .padding([5, 12])
            .width(Length::Fill)
            .height(26)
            .style(move |_| container::Style {
                text_color: Some(p.colors.text_muted),
                background: Some(p.colors.panel.into()),
                ..container::Style::default()
            });
        let mut list = keyed::Column::new().width(Length::Fill).spacing(0);
        if range.start > 0 {
            list = list.push(
                RowKey::Before,
                space().height(range.start as f32 * ROW_HEIGHT),
            );
        }
        for index in range.clone() {
            // A source ID may repeat; each ordered occurrence owns its controls.
            let track = &inspection.tracks[index];
            let number = if arranging {
                format!("{:02}", index + 1)
            } else {
                track.metadata.track_number.map_or_else(
                    || format!("{:02}", index + 1),
                    |number| format!("{number:02}"),
                )
            };
            let title = text(single_line(&track.metadata.title))
                .size(13)
                .width(Length::Fill)
                .height(18)
                .wrapping(Wrapping::None);
            let labels: Element<'_, Message> = if compact {
                column![
                    title,
                    text(single_line(&track.metadata.artist))
                        .size(11)
                        .height(15)
                        .width(Length::Fill)
                        .wrapping(Wrapping::None)
                        .color(p.colors.text_muted)
                ]
                .spacing(2)
                .width(Length::Fill)
                .into()
            } else {
                title.into()
            };
            let selected = review.is_some_and(|review| review.selected.contains(&index));
            let selection: Element<'_, Message> = if review.is_some() {
                focus_button::wrap_control(
                    format!("review-select-track-{index}"),
                    checkbox(selected)
                        .size(14)
                        .on_toggle_maybe(
                            enabled.then_some(move |selected| Message::Toggle(index, selected)),
                        )
                        .into(),
                    enabled.then_some(Message::Toggle(index, !selected)),
                    p.colors.accent,
                )
            } else {
                space().width(14).into()
            };
            let mut line = row![
                selection,
                container(
                    text(number)
                        .size(11)
                        .width(number_width)
                        .wrapping(Wrapping::None)
                        .color(p.colors.text_muted)
                )
                .width(number_width)
                .clip(true),
                container(labels).width(Length::Fill).clip(true),
            ]
            .spacing(10)
            .align_y(Alignment::Center);
            if !compact {
                line = line.push(
                    container(
                        text(single_line(&track.metadata.artist))
                            .size(12)
                            .width(artist_width)
                            .wrapping(Wrapping::None)
                            .color(p.colors.text_muted),
                    )
                    .width(artist_width)
                    .clip(true),
                );
            }
            if show_time {
                let duration = track
                    .duration
                    .filter(|duration| duration.is_finite() && *duration >= 0.0)
                    .map(|duration| format!("{}:{:02}", duration as u64 / 60, duration as u64 % 60))
                    .unwrap_or_else(|| "—".into());
                line = line.push(
                    container(
                        text(duration)
                            .size(11)
                            .width(42)
                            .wrapping(Wrapping::None)
                            .color(p.colors.text_muted),
                    )
                    .width(42)
                    .clip(true),
                );
            }
            line = if arranging {
                line.push(
                    row![
                        focus_button::wrap(
                            position_id(index),
                            button(text("Move…").size(11))
                                .width(52)
                                .padding([5, 6])
                                .style(p.quiet()),
                            (!app.busy() && app.track_position.is_none())
                                .then_some(Message::ChooseTrackPosition(index)),
                            p.colors.accent,
                        ),
                        move_button(app, index, count, true),
                        move_button(app, index, count, false)
                    ]
                    .spacing(6),
                )
            } else if review.is_some() {
                line.push(focus_button::wrap(
                    edit_id(index),
                    button(
                        text(if editor_track == Some(index) {
                            "Open"
                        } else {
                            "Edit"
                        })
                        .size(11),
                    )
                    .padding([5, 7])
                    .width(40)
                    .style(p.quiet()),
                    enabled.then_some(Message::OpenEditor(Kind::Track(index))),
                    p.colors.accent,
                ))
            } else {
                line.push(space().width(40))
            };
            let active =
                editor_track == Some(index) || (arranging && app.arranged_track == Some(index));
            let row_surface = container(line)
                .padding([6, 12])
                .height(ROW_HEIGHT - 1.0)
                .width(Length::Fill)
                .align_y(Alignment::Center)
                .clip(true)
                .style(move |_| container::Style {
                    background: (selected || active).then(|| {
                        theme::blend(
                            p.colors.accent,
                            p.colors.panel,
                            if active { 0.19 } else { 0.055 },
                        )
                        .into()
                    }),
                    text_color: Some(p.colors.text),
                    ..container::Style::default()
                });
            let separator = container(space())
                .height(1)
                .width(Length::Fill)
                .style(move |_| container::Style {
                    background: Some(p.colors.border.into()),
                    ..container::Style::default()
                });
            list = list.push(
                RowKey::Track(index),
                column![row_surface, separator]
                    .spacing(0)
                    .height(ROW_HEIGHT),
            );
        }
        if range.end < count {
            list = list.push(
                RowKey::After,
                space().height((count - range.end) as f32 * ROW_HEIGHT),
            );
        }
        let table = scrollable(list)
            .style(p.scrollbar())
            .direction(crate::style::scroll_direction())
            .id("track-list")
            .height(Length::Fill)
            .width(Length::Fill)
            .on_scroll(|viewport| Message::TrackScrolled(viewport.absolute_offset().y));
        let measured = sensor(table)
            .key(app.panel)
            .on_show(|size| Message::TrackListSized(size.height))
            .on_resize(|size| Message::TrackListSized(size.height));
        column![header, measured]
            .spacing(0)
            .height(Length::Fill)
            .into()
    })
    .height(Length::Fill);
    column![
        container(toolbar)
            .padding([0, 12])
            .height(28)
            .align_y(Alignment::Center),
        viewport
    ]
    .spacing(8)
    .height(Length::Fill)
    .into()
}

fn move_button(app: &App, index: usize, count: usize, up: bool) -> Element<'_, Message> {
    let p = app.presentation();
    let to = if up {
        index.checked_sub(1)
    } else {
        Some(index + 1).filter(|&next| next < count)
    };
    tooltip(
        focus_button::wrap(
            move_id(index, up),
            button(text(if up { "↑" } else { "↓" }).size(15))
                .width(30)
                .padding([4, 7])
                .style(p.secondary()),
            to.filter(|_| !app.busy() && app.track_position.is_none())
                .map(|to| Message::MoveTrack { from: index, to }),
            p.colors.accent,
        ),
        container(text(if up { "Move up" } else { "Move down" }).size(12))
            .padding(6)
            .style(move |_| p.panel()),
        tooltip::Position::Left,
    )
    .into()
}

// Preserve original tags while keeping explicit line breaks out of row geometry.
fn single_line(value: &str) -> Cow<'_, str> {
    let is_break =
        |character: char| character.is_control() || matches!(character, '\u{2028}' | '\u{2029}');
    if value.chars().any(is_break) {
        Cow::Owned(
            value
                .chars()
                .map(|character| if is_break(character) { ' ' } else { character })
                .collect(),
        )
    } else {
        Cow::Borrowed(value)
    }
}

fn visible_rows(count: usize, offset: f32, viewport_height: f32) -> Range<usize> {
    if count == 0 {
        return 0..0;
    }
    let height = if viewport_height.is_finite() {
        viewport_height.max(ROW_HEIGHT)
    } else {
        ROW_HEIGHT
    };
    let maximum = (count as f32 * ROW_HEIGHT - height).max(0.0);
    let offset = if offset.is_finite() {
        offset.clamp(0.0, maximum)
    } else {
        0.0
    };
    let first = ((offset / ROW_HEIGHT).floor() as usize).min(count);
    let after = (((offset + height) / ROW_HEIGHT).ceil() as usize).min(count);
    first.saturating_sub(OVERSCAN)..after.saturating_add(OVERSCAN).min(count)
}

#[cfg(test)]
mod tests {
    use super::{single_line, visible_rows, OVERSCAN, ROW_HEIGHT};

    #[test]
    fn empty_lists_have_no_rows_or_spacers() {
        assert_eq!(visible_rows(0, 1000.0, 300.0), 0..0);
    }

    #[test]
    fn a_large_playlist_creates_only_the_visible_rows_and_overscan() {
        let count = 1_000_000;
        let offset = ROW_HEIGHT * 123_456.0 + 32.0;
        let range = visible_rows(count, offset, 300.0);
        assert_eq!(range.start, 123_456 - OVERSCAN);
        assert!(range.end >= 123_462);
        assert!(range.len() <= (300.0 / ROW_HEIGHT).ceil() as usize + 1 + 2 * OVERSCAN);
        assert_eq!(
            range.start as f32 * ROW_HEIGHT
                + range.len() as f32 * ROW_HEIGHT
                + (count - range.end) as f32 * ROW_HEIGHT,
            count as f32 * ROW_HEIGHT
        );
    }

    #[test]
    fn stale_offsets_are_clamped_after_a_playlist_or_viewport_change() {
        let last = visible_rows(10_000, 1_000_000.0, 300.0);
        assert_eq!(last.end, 10_000);
        assert!((9_990..=9_994).contains(&last.start));
        assert_eq!(visible_rows(2, 1_000_000.0, 300.0), 0..2);
    }

    #[test]
    fn fractional_rows_remain_covered_and_invalid_offsets_start_at_the_top() {
        let range = visible_rows(100, ROW_HEIGHT * 20.0 + 0.5, ROW_HEIGHT);
        assert!(range.contains(&20) && range.contains(&21));
        for offset in [-1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(visible_rows(100, offset, 300.0).start, 0);
        }
        for height in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert!(visible_rows(100, 0.0, height).len() <= 1 + 2 * OVERSCAN);
        }
    }
    #[test]
    fn explicit_line_breaks_cannot_expand_a_fixed_height_row() {
        let original = "Song\nwith\rline\u{2028}breaks";
        assert_eq!(single_line(original), "Song with line breaks");
        assert_eq!(original, "Song\nwith\rline\u{2028}breaks");
        assert!(matches!(
            single_line("Ordinary song"),
            std::borrow::Cow::Borrowed(_)
        ));
    }
}
