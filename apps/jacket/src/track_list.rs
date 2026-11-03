//! Fixed-height, occurrence-indexed track rows with bounded widget creation.
use crate::app::{App, Message};
use crate::editor::Kind;
use crate::theme;
use iced::widget::text::Wrapping;
use iced::widget::{
    button, checkbox, column, container, keyed, responsive, row, scrollable, space, text,
};
use iced::{Alignment, Element, Length};
use std::borrow::Cow;
use std::ops::Range;

const ROW_HEIGHT: f32 = 52.0;
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
    let p = app.presentation();
    let enabled = !app.busy();
    let count = review.inspection.tracks.len();
    let all_selected = count > 0 && review.selected.len() == count;
    let mut toolbar = row![
        checkbox(all_selected)
            .size(14)
            .label("Select all")
            .text_size(12)
            .on_toggle_maybe((enabled && count > 0).then_some(Message::SelectAll)),
        text(format!("{} selected", review.selected.len()))
            .size(12)
            .color(p.colors.text_muted),
        space::horizontal(),
        button(text("Edit selected").size(12))
            .padding([5, 8])
            .on_press_maybe(
                (enabled && !review.selected.is_empty()).then_some(Message::OpenEditor(Kind::Bulk))
            )
            .style(p.quiet()),
    ]
    .spacing(12)
    .align_y(Alignment::Center);
    if review.can_undo_bulk() {
        toolbar = toolbar.push(
            button(text("Undo edit").size(12))
                .padding([5, 8])
                .on_press_maybe(enabled.then_some(Message::UndoBulk))
                .style(p.quiet()),
        );
    }
    let viewport = responsive(move |size| {
        let range = visible_rows(
            count,
            app.track_scroll,
            (size.height - 26.0).max(ROW_HEIGHT),
        );
        let artist_width = (size.width * 0.25).clamp(90.0, 170.0);
        let mut headings = row![
            space().width(18),
            text("#").size(10).width(28),
            text(if compact { "Track" } else { "Title" })
                .size(10)
                .width(Length::Fill)
        ]
        .spacing(10)
        .align_y(Alignment::Center);
        if !compact {
            headings = headings
                .push(text("Artist").size(10).width(artist_width))
                .push(text("Time").size(10).width(42));
        }
        headings = headings.push(space().width(40));
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
            let track = &review.inspection.tracks[index];
            let number = track.metadata.track_number.map_or_else(
                || format!("{:02}", index + 1),
                |number| format!("{number:02}"),
            );
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
            let mut line = row![
                checkbox(review.selected.contains(&index))
                    .size(14)
                    .on_toggle_maybe(
                        enabled.then_some(move |selected| Message::Toggle(index, selected))
                    ),
                container(
                    text(number)
                        .size(11)
                        .width(28)
                        .wrapping(Wrapping::None)
                        .color(p.colors.text_muted)
                )
                .width(28)
                .clip(true),
                container(labels).width(Length::Fill).clip(true),
            ]
            .spacing(10)
            .align_y(Alignment::Center);
            if !compact {
                let duration = track
                    .duration
                    .filter(|duration| duration.is_finite() && *duration >= 0.0)
                    .map(|duration| format!("{}:{:02}", duration as u64 / 60, duration as u64 % 60))
                    .unwrap_or_else(|| "—".into());
                line = line
                    .push(
                        container(
                            text(single_line(&track.metadata.artist))
                                .size(12)
                                .width(artist_width)
                                .wrapping(Wrapping::None)
                                .color(p.colors.text_muted),
                        )
                        .width(artist_width)
                        .clip(true),
                    )
                    .push(
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
            line = line.push(
                button(text("Edit").size(11))
                    .padding([5, 7])
                    .width(40)
                    .on_press_maybe(enabled.then_some(Message::OpenEditor(Kind::Track(index))))
                    .style(p.quiet()),
            );
            let selected = review.selected.contains(&index);
            let row_surface = container(line)
                .padding([6, 12])
                .height(ROW_HEIGHT - 1.0)
                .width(Length::Fill)
                .align_y(Alignment::Center)
                .clip(true)
                .style(move |_| container::Style {
                    background: selected
                        .then(|| theme::blend(p.colors.accent, p.colors.panel, 0.055).into()),
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
        column![header, table]
            .spacing(0)
            .height(Length::Fill)
            .into()
    })
    .height(Length::Fill);
    column![container(toolbar).padding([0, 12]), viewport]
        .spacing(8)
        .height(Length::Fill)
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
