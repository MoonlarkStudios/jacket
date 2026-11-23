use crate::app::{App, Message};
use crate::editor::Kind;
use crate::icons::{self, Icon};
use crate::settings::{FormatChoice, OutputChoice};
use audio_engine::{AudioFormat, OutputKind};
use iced::widget::{button, column, pick_list, row, rule, space, text};
use iced::{Alignment, Element, Length};

pub fn view(app: &App) -> Element<'_, Message> {
    let p = app.presentation();
    let Some(review) = &app.review else {
        return space().into();
    };
    let output = row![
        output(app, OutputChoice::File, "File", Icon::File),
        output(app, OutputChoice::Folder, "Folder", Icon::Folder),
        output(app, OutputChoice::Zip, "ZIP", Icon::Archive),
    ]
    .spacing(6);
    let mut fields = column![].spacing(18).width(Length::Fill).push(
        column![
            text("Output type").size(12).color(p.colors.text_muted),
            output
        ]
        .spacing(7),
    );
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
                AudioFormat::Compatible =>
                    "New files use M4A for AAC and ALAC, and MP3 for other codecs.",
                AudioFormat::Preserve => "New files retain the source audio codec.",
                AudioFormat::Mp3 => "New files use MP3.",
            })
            .size(11)
            .color(p.colors.text_muted)
        ]
        .spacing(7),
    );
    fields = fields.push(rule::horizontal(1).style(p.line())).push(
        column![
            row![
                text(match review.preferences.output_kind {
                    OutputKind::File => "Save As filename",
                    OutputKind::Folder if app.is_album_folder() => "Save As folder name",
                    OutputKind::Folder => "Folder name",
                    OutputKind::Zip => "ZIP filename",
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
    let mut actions = row![
        super::focus_button::wrap(
            "discard-review",
            button("Discard review").padding([7, 0]).style(p.quiet()),
            (!app.busy()).then_some(Message::Reset),
            p.colors.accent,
        ),
        space::horizontal()
    ]
    .align_y(Alignment::Center);
    if app.uses_music_folder() {
        actions = actions.push(super::focus_button::wrap(
            "export-music-folder",
            button("Music folder settings…")
                .padding([7, 9])
                .style(p.secondary()),
            (!app.busy()).then_some(Message::OpenMusicFolder),
            p.colors.accent,
        ));
    }
    fields
        .push(rule::horizontal(1).style(p.line()))
        .push(actions)
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
