use crate::app::{App, Message};
use crate::editor::Kind;
use crate::icons::{self, Icon};
use crate::settings::{FormatChoice, OutputChoice};
use crate::style;
use audio_engine::{ArtworkMode, AudioFormat, OutputKind};
use iced::widget::{
    button, column, container, pick_list, row, rule, scrollable, space, text, Column,
};
use iced::{Alignment, Element, Length};

pub fn view(app: &App, docked: bool) -> Element<'_, Message> {
    let p = app.presentation();
    let Some(review) = &app.review else {
        return space().into();
    };
    if docked && review.preferences.output_kind == OutputKind::File {
        return file_inspector(app, review);
    }
    let mut fields = Column::new().spacing(18).width(Length::Fill);
    if docked {
        fields = fields.push(text("Export").size(14).font(style::semibold()));
    }
    let output = row![
        output(app, OutputChoice::File, "File", Icon::File),
        output(app, OutputChoice::Folder, "Folder", Icon::Folder),
        output(app, OutputChoice::Zip, "ZIP", Icon::Archive),
    ]
    .spacing(6);
    fields = fields
        .push(column![text("Save as").size(12).color(p.colors.text_muted), output].spacing(7));
    if review.preferences.output_kind == OutputKind::File && review.selected.len() != 1 {
        fields = fields.push(
            text("Select one track for an audio file, or choose Folder or ZIP.")
                .size(12)
                .color(p.colors.warning),
        );
    }
    fields = fields.push(
        column![
            text("Audio format").size(12).color(p.colors.text_muted),
            pick_list(
                FormatChoice::ALL,
                Some(FormatChoice::from(review.preferences.format)),
                Message::Format
            )
            .text_size(13)
            .padding(9)
            .width(Length::Fill),
            text(match review.preferences.format {
                AudioFormat::Compatible => "Keeps AAC in M4A; other sources use MP3.",
                AudioFormat::Preserve => "Retains the source audio codec.",
                AudioFormat::Mp3 => "Saves every selected track as MP3.",
            })
            .size(11)
            .color(p.colors.text_muted)
        ]
        .spacing(7),
    );
    fields = fields.push(rule::horizontal(1).style(p.line())).push(
        column![
            row![
                text(if review.preferences.output_kind == OutputKind::File {
                    "Save As filename"
                } else {
                    "Output name"
                })
                .size(12)
                .color(p.colors.text_muted),
                space::horizontal(),
                button(icons::view(Icon::Edit, p.colors.text_muted, 14.0))
                    .padding(3)
                    .on_press_maybe((!app.busy()).then_some(Message::OpenEditor(Kind::OutputName)))
                    .style(p.quiet())
            ]
            .align_y(Alignment::Center),
            text(&review.preferences.title).size(13).width(Length::Fill),
        ]
        .spacing(5),
    );
    let content: Element<'_, Message> = if docked {
        scrollable(container(fields).padding(iced::Padding {
            right: 12.0,
            ..iced::Padding::ZERO
        }))
        .style(p.scrollbar())
        .direction(crate::style::scroll_direction())
        .height(Length::Fill)
        .into()
    } else {
        fields.into()
    };
    if !docked {
        return content;
    }
    column![
        content,
        rule::horizontal(1).style(p.line()),
        preview_button(app),
        discard(app),
    ]
    .spacing(12)
    .height(Length::Fill)
    .into()
}

fn file_inspector<'a>(app: &'a App, review: &'a jacket_core::Review) -> Element<'a, Message> {
    let p = app.presentation();
    let cover = if matches!(review.preferences.artwork_mode, ArtworkMode::None) {
        "No cover"
    } else {
        "Embed cover when supported"
    };
    let options = super::focus_button::wrap(
        "file-export-options",
        button(
            row![
                icons::view(Icon::Settings, p.colors.text_muted, 15.0),
                text("Export options").size(12)
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .style(p.secondary())
        .padding([8, 10]),
        (!app.busy()).then_some(Message::OpenPanel(crate::app::Panel::Options)),
        p.colors.accent,
    );
    column![
        text("Export").size(14).font(style::semibold()),
        super::destination::save_controls(app, super::destination::Placement::Inspector),
        rule::horizontal(1).style(p.line()),
        text(format!(
            "{} · {cover}",
            FormatChoice::from(review.preferences.format)
        ))
        .size(12)
        .color(p.colors.text_muted),
        options,
        space::vertical(),
        discard(app),
    ]
    .spacing(12)
    .height(Length::Fill)
    .into()
}

fn discard(app: &App) -> Element<'_, Message> {
    button("Discard review")
        .padding(6)
        .on_press_maybe((!app.busy()).then_some(Message::Reset))
        .style(app.presentation().quiet())
        .into()
}

fn preview_button(app: &App) -> Element<'_, Message> {
    let p = app.presentation();
    button(
        row![
            text("Preview files").size(13),
            space::horizontal(),
            icons::view(
                Icon::ArrowRight,
                if app.can_preview() {
                    p.colors.accent_text
                } else {
                    p.derived.disabled
                },
                15.0
            )
        ]
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([11, 12])
    .on_press_maybe(app.can_preview().then_some(Message::Preview))
    .style(p.primary())
    .into()
}

fn output<'a>(
    app: &'a App,
    choice: OutputChoice,
    label: &'static str,
    icon: Icon,
) -> Element<'a, Message> {
    let p = app.presentation();
    let selected = app
        .review
        .as_ref()
        .is_some_and(|review| OutputChoice::from(review.preferences.output_kind) == choice);
    let enabled = !app.busy()
        && (choice != OutputChoice::File
            || app
                .review
                .as_ref()
                .is_some_and(|review| review.selected.len() == 1));
    button(
        column![
            icons::view(
                icon,
                if selected {
                    p.colors.accent
                } else {
                    p.colors.text_muted
                },
                16.0
            ),
            text(label).size(11),
        ]
        .spacing(5)
        .align_x(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([9, 4])
    .on_press_maybe(enabled.then_some(Message::Output(choice)))
    .style(p.choice(selected))
    .into()
}
