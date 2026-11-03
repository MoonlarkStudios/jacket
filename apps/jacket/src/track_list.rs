//! Fixed-height, occurrence-indexed track rows with bounded widget creation.
use crate::app::{App, Message};
use crate::editor::Kind;
use crate::style;
use iced::widget::text::Wrapping;
use iced::widget::{
    button, checkbox, column, container, keyed, responsive, row, rule, scrollable, space, text,
};
use iced::{Alignment, Element, Length};
use std::borrow::Cow;
use std::ops::Range;

const ROW_HEIGHT: f32 = 64.0;
const COMPACT_HEIGHT: f32 = 300.0;
const OVERSCAN: usize = 3;

#[derive(Clone, Copy, PartialEq, Eq)]
enum RowKey {
    Before,
    Track(usize),
    After,
}

pub fn view(app: &App, compact: bool) -> Element<'_, Message> {
    let Some(review) = &app.review else {
        return space().into();
    };
    let enabled = !app.busy();
    let count = review.inspection.tracks.len();
    let all_selected = count > 0 && review.selected.len() == count;
    let mut toolbar = row![
        checkbox(all_selected)
            .label("Select all")
            .on_toggle_maybe((enabled && count > 0).then_some(Message::SelectAll)),
        space::horizontal(),
        button("Edit selected")
            .on_press_maybe(
                (enabled && !review.selected.is_empty()).then_some(Message::OpenEditor(Kind::Bulk))
            )
            .style(button::text),
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    if review.can_undo_bulk() {
        toolbar = toolbar.push(
            button("Undo edit")
                .on_press_maybe(enabled.then_some(Message::UndoBulk))
                .style(button::text),
        );
    }
    let viewport = responsive(move |size| {
        let range = visible_rows(count, app.track_scroll, size.height);
        let mut list = keyed::Column::new().width(Length::Fill).spacing(0);
        if range.start > 0 {
            list = list.push(
                RowKey::Before,
                space().height(range.start as f32 * ROW_HEIGHT),
            );
        }
        for index in range.clone() {
            // Index identifies this playlist occurrence. Repeated source IDs
            // must remain independently selectable and editable.
            let track = &review.inspection.tracks[index];
            let number = track.metadata.track_number.map_or_else(
                || format!("{:02}", index + 1),
                |number| format!("{number:02}"),
            );
            let labels = container(
                column![
                    text(single_line(&track.metadata.title))
                        .size(15)
                        .height(20)
                        .width(Length::Fill)
                        .wrapping(Wrapping::None),
                    text(single_line(&track.metadata.artist))
                        .size(12)
                        .height(16)
                        .width(Length::Fill)
                        .wrapping(Wrapping::None)
                        .color(style::muted(app.appearance.dark)),
                ]
                .spacing(2)
                .width(Length::Fill),
            )
            .width(Length::Fill)
            .clip(true);
            let mut line = row![
                checkbox(review.selected.contains(&index)).on_toggle_maybe(
                    enabled.then_some(move |selected| Message::Toggle(index, selected))
                ),
                container(
                    text(number)
                        .size(12)
                        .width(40)
                        .wrapping(Wrapping::None)
                        .color(style::muted(app.appearance.dark))
                )
                .width(40)
                .clip(true),
                labels,
            ]
            .spacing(10)
            .align_y(Alignment::Center);
            if !compact {
                let duration = track
                    .duration
                    .filter(|duration| duration.is_finite() && *duration >= 0.0)
                    .map(|duration| format!("{}:{:02}", duration as u64 / 60, duration as u64 % 60))
                    .unwrap_or_else(|| "—".into());
                line = line.push(
                    container(
                        text(duration)
                            .size(12)
                            .width(46)
                            .wrapping(Wrapping::None)
                            .color(style::muted(app.appearance.dark)),
                    )
                    .width(46)
                    .clip(true),
                );
            }
            line = line.push(
                button("Edit")
                    .padding([6, 10])
                    .on_press_maybe(enabled.then_some(Message::OpenEditor(Kind::Track(index))))
                    .style(button::text),
            );
            list = list.push(
                RowKey::Track(index),
                container(line)
                    .padding([10, 10])
                    .height(ROW_HEIGHT)
                    .width(Length::Fill)
                    .align_y(Alignment::Center)
                    .clip(true)
                    .style(move |theme| {
                        if index % 2 == 0 {
                            style::toolbar(theme)
                        } else {
                            container::Style::default()
                        }
                    }),
            );
        }
        if range.end < count {
            list = list.push(
                RowKey::After,
                space().height((count - range.end) as f32 * ROW_HEIGHT),
            );
        }
        scrollable(list)
            .id("track-list")
            .height(Length::Fill)
            .width(Length::Fill)
            .on_scroll(|viewport| Message::TrackScrolled(viewport.absolute_offset().y))
            .into()
    })
    .height(if compact {
        Length::Fixed(COMPACT_HEIGHT)
    } else {
        Length::Fill
    });
    column![toolbar, rule::horizontal(1), viewport]
        .spacing(10)
        .height(if compact {
            Length::Shrink
        } else {
            Length::Fill
        })
        .into()
}

// Explicit line breaks must not change the fixed row geometry. Keep ordinary
// titles borrowed and preserve the original metadata for editing/export.
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
        assert!(last.start <= 9_995 && last.start >= 9_992);
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
