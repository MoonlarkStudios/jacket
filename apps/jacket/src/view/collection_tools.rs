//! Editors and ordering controls that keep the collection in view.
use super::focus_button;
use crate::app::{App, Message};
use crate::editor::{Action, Editor, Kind};
use crate::style;
use iced::widget::{button, column, container, row, rule, scrollable, space, text, text_input};
use iced::{Alignment, Element, Length};

pub fn editor(app: &App, compact: bool) -> Element<'_, Message> {
    let (Some(review), Some(editor)) = (&app.review, &app.editor) else {
        return space().into();
    };
    let p = app.presentation();
    let count = review.inspection.tracks.len();
    let (heading, height) = match editor.kind {
        Kind::Track(index) => (
            format!("Track {} of {count}", index + 1),
            if Editor::has_individual_album_fields(review) {
                300
            } else {
                248
            },
        ),
        Kind::Album => ("Shared album tags".into(), 236),
        Kind::PlaylistName => ("Rename playlist".into(), 166),
        _ => ("Edit tags".into(), 294),
    };
    let body = column![
        text(heading).font(style::semibold()).size(15),
        editor.quick_fields(review, p),
    ]
    .spacing(10)
    .width(Length::Fill);
    let fields = scrollable(container(body).padding([12, 16]).width(Length::Fill))
        .style(p.scrollbar())
        .direction(style::scroll_direction())
        .height(Length::Fill);
    let mut buttons = row![tool(
        "cancel-collection-edit",
        "Cancel",
        Message::ClosePanel,
        app,
        true
    )];
    if let Kind::Track(index) = editor.kind {
        if compact {
            buttons = buttons.push(tool(
                "restore-track-tags",
                "Restore source tags",
                Message::RestoreSourceTags,
                app,
                !app.busy(),
            ));
        }
        buttons = buttons.push(space::horizontal());
        if index + 1 < count {
            buttons = buttons.push(tool(
                "apply-next-track",
                "Apply & next",
                Message::Editor(Action::ApplyNext),
                app,
                !app.busy(),
            ));
        }
    } else {
        buttons = buttons.push(space::horizontal());
    }
    let mut actions = column![buttons
        .push(focus_button::wrap(
            "apply-collection-tags",
            button("Apply").padding([8, 10]).style(p.primary()),
            (!app.busy()).then_some(Message::Editor(Action::Apply)),
            p.colors.accent,
        ))
        .spacing(7)
        .align_y(Alignment::Center)];
    if !compact && matches!(editor.kind, Kind::Track(_)) {
        actions = actions.push(tool(
            "restore-track-tags",
            "Restore source tags",
            Message::RestoreSourceTags,
            app,
            !app.busy(),
        ));
    }
    if let Some(error) = &editor.error {
        actions = actions.push(text(error).size(12).color(p.colors.danger));
    }
    let content = column![
        fields,
        rule::horizontal(1).style(p.line()),
        container(actions.spacing(8))
            .padding([10, 14])
            .width(Length::Fill),
    ]
    .width(Length::Fill);
    container(content)
        .width(Length::Fill)
        .height(if compact {
            Length::Fixed(height as f32 + if editor.error.is_some() { 22.0 } else { 0.0 })
        } else {
            Length::Fill
        })
        .style(move |_| p.surface(p.colors.panel))
        .into()
}

pub fn order(app: &App, compact: bool) -> Element<'_, Message> {
    let p = app.presentation();
    let Some(review) = &app.review else {
        return space().into();
    };
    if let Some(edit) = &app.track_position {
        let count = review.inspection.tracks.len();
        let valid = app.track_position_target().is_some();
        let input = text_input("Position", &edit.position)
            .id("track-position")
            .on_input(Message::TrackPosition)
            .on_submit_maybe(valid.then_some(Message::ApplyTrackPosition))
            .width(76)
            .padding([7, 8])
            .size(13)
            .style(p.input());
        let buttons = row![
            tool(
                "cancel-track-position",
                "Cancel",
                Message::CancelTrackPosition,
                app,
                true
            ),
            focus_button::wrap(
                "apply-track-position",
                button("Move").padding([8, 10]).style(p.primary()),
                valid.then_some(Message::ApplyTrackPosition),
                p.colors.accent,
            )
        ]
        .spacing(7)
        .align_y(Alignment::Center);
        let fields = row![
            text(format!("Position (1–{count})"))
                .size(12)
                .color(p.colors.text_muted),
            input,
        ]
        .spacing(8)
        .align_y(Alignment::Center);
        let heading = text(format!("Move track {}", edit.index + 1))
            .font(style::semibold())
            .size(15);
        return if compact {
            row![heading, fields, space::horizontal(), buttons]
                .spacing(10)
                .align_y(Alignment::Center)
                .into()
        } else {
            column![heading, fields, buttons].spacing(12).into()
        };
    }
    let idle = !app.busy();
    let mut actions = row![tool(
        "undo-track-move",
        "Undo move",
        Message::UndoTrackMove,
        app,
        idle && review.can_undo_move()
    )]
    .spacing(7);
    actions = actions.push(tool(
        "number-track-tags",
        "Number tags in order",
        Message::NumberTracks,
        app,
        idle,
    ));
    if review.can_undo_numbering() {
        actions = actions.push(tool(
            "undo-track-numbering",
            "Undo numbering",
            Message::UndoNumbering,
            app,
            idle,
        ));
    }
    let done = focus_button::wrap(
        "finish-track-order",
        button("Done").padding([8, 12]).style(p.primary()),
        Some(Message::ClosePanel),
        p.colors.accent,
    );
    let note = text(
        if review.preferences.mode == audio_engine::CollectionMode::Album {
            "Number tags to match this order within each disc. Album filenames use those numbers."
        } else {
            "Number tags to match this order within each disc."
        },
    )
    .size(12)
    .color(p.colors.text_muted);
    if compact {
        column![
            actions
                .push(space::horizontal())
                .push(done)
                .align_y(Alignment::Center),
            note
        ]
        .spacing(5)
        .into()
    } else {
        column![
            text("Track order").font(style::semibold()).size(15),
            note,
            actions.wrap().vertical_spacing(6),
            space::vertical(),
            done
        ]
        .spacing(12)
        .height(Length::Fill)
        .into()
    }
}

fn tool<'a>(
    id: &'static str,
    label: &'static str,
    message: Message,
    app: &App,
    enabled: bool,
) -> Element<'a, Message> {
    let p = app.presentation();
    focus_button::wrap(
        id,
        button(text(label).size(12))
            .padding([7, 8])
            .style(p.secondary()),
        enabled.then_some(message),
        p.colors.accent,
    )
}
