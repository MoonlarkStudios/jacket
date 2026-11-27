use super::focus_button;
use crate::app::{
    App, DownloadAction as Action, Message, Panel, DOWNLOADS_LIST_ID, DOWNLOADS_PANEL_ID,
};
use crate::icons::{self, Icon};
use crate::style;
use iced::widget::{
    button, column, container, progress_bar, row, rule, scrollable, space, text, tooltip,
};
use iced::{Alignment, Element, Length};
use jacket_desktop::Download;
use std::time::Duration;

pub(super) fn trigger(app: &App) -> Element<'_, Message> {
    let count = app.downloads.outstanding();
    let p = app.presentation();
    let active = app.downloads.is_open();
    let enabled = app.panel == Panel::Workspace;
    let color = if !enabled {
        p.derived.disabled
    } else if active {
        p.colors.accent
    } else {
        p.colors.text_muted
    };
    let mut content = row![icons::view(Icon::Download, color, 17.0)]
        .spacing(5)
        .align_y(Alignment::Center);
    if count > 0 {
        content = content.push(
            container(text(count.to_string()).size(10).font(style::semibold()))
                .padding([1, 5])
                .style(move |_| container::Style {
                    background: Some(p.derived.selection_fill.into()),
                    text_color: Some(if enabled {
                        p.colors.accent
                    } else {
                        p.derived.disabled
                    }),
                    border: iced::Border {
                        radius: 8.0.into(),
                        ..iced::Border::default()
                    },
                    ..container::Style::default()
                }),
        );
    }
    let button = focus_button::wrap(
        "open-downloads",
        button(content).padding([6, 9]).style(move |theme, status| {
            if active {
                p.choice(true)(theme, status)
            } else {
                p.quiet()(theme, status)
            }
        }),
        enabled.then_some(Message::Downloads(Action::Toggle)),
        p.colors.accent,
    );
    if active {
        return button;
    }
    tooltip(
        button,
        text("Downloads").size(12),
        tooltip::Position::Bottom,
    )
    .delay(Duration::from_millis(500))
    .gap(5)
    .padding(6)
    .style(move |_| p.panel())
    .into()
}

pub(super) fn notice(app: &App, label: &str) -> Element<'static, Message> {
    let p = app.presentation();
    container(
        row![
            text(label.to_owned()).size(13).width(Length::Fill),
            control(
                app,
                "view-downloads",
                "View Downloads".into(),
                Action::OpenPage,
                true
            ),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .padding([8, 12])
    .width(Length::Fill)
    .max_width(520)
    .style(move |_| p.panel())
    .into()
}

pub(super) fn dropdown(app: &App, width: f32) -> Element<'_, Message> {
    const RECENT_LIMIT: usize = 3;
    let p = app.presentation();
    let active = app.downloads.active();
    let items = &app.downloads.items;
    let recent = items
        .iter()
        .filter(|item| active == Some(item.recipe.job_id.as_str()))
        .chain(
            items
                .iter()
                .rev()
                .filter(|item| active != Some(item.recipe.job_id.as_str())),
        )
        .take(RECENT_LIMIT);
    let mut list = column![].width(Length::Fill);
    if let Some(error) = &app.downloads.error {
        list = list.push(text(error).size(12).color(p.colors.danger));
    }
    if items.is_empty() {
        list = list.push(
            text("No downloads yet.")
                .size(13)
                .color(p.colors.text_muted),
        );
    }
    for (index, item) in recent.enumerate() {
        if index > 0 {
            list = list.push(rule::horizontal(1).style(p.line()));
        }
        list = list.push(download_row(app, item, true));
    }
    let footer = focus_button::wrap(
        "downloads-open-page",
        button(
            row![
                text("Open Downloads").size(13).width(Length::Fill),
                icons::view(Icon::ArrowRight, p.colors.text_muted, 15.0),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .padding([8, 10])
        .style(p.quiet()),
        Some(Message::Downloads(Action::OpenPage)),
        p.colors.accent,
    );
    container(
        column![
            text("Recent downloads").size(15).font(style::semibold()),
            scrollable(list)
                .id(DOWNLOADS_LIST_ID)
                .height(Length::Shrink)
                .width(Length::Fill)
                .direction(style::scroll_direction())
                .style(p.scrollbar()),
            rule::horizontal(1).style(p.line()),
            footer,
        ]
        .spacing(8),
    )
    .id(DOWNLOADS_PANEL_ID)
    .padding(12)
    .width(width.min(360.0))
    .style(move |_| container::Style {
        shadow: iced::Shadow {
            color: iced::Color::BLACK.scale_alpha(0.28),
            offset: iced::Vector::new(0.0, 5.0),
            blur_radius: 16.0,
        },
        ..p.panel()
    })
    .into()
}

pub(super) fn page(app: &App, compact: bool) -> Element<'_, Message> {
    let p = app.presentation();
    let active = app.downloads.active();
    let items = &app.downloads.items;
    let mut list = column![].spacing(10).width(Length::Fill);
    if items.is_empty() {
        list = list.push(
            text("Downloads you queue will appear here.")
                .size(14)
                .color(p.colors.text_muted),
        );
    }
    let ordered = items
        .iter()
        .filter(|item| active == Some(item.recipe.job_id.as_str()))
        .chain(items.iter().filter(|item| item.waiting()))
        .chain(
            items
                .iter()
                .rev()
                .filter(|item| !item.waiting() && active != Some(item.recipe.job_id.as_str())),
        );
    for item in ordered {
        list = list.push(download_row(app, item, false));
    }

    let mut body = column![
        control(app, "downloads-back", "Back".into(), Action::Close, true),
        text("Downloads").font(style::semibold()).size(27),
    ]
    .spacing(14);
    if let Some(error) = &app.downloads.error {
        body = body.push(text(error).size(13).color(p.colors.danger));
    }
    body = body.push(
        scrollable(list)
            .id(DOWNLOADS_LIST_ID)
            .width(Length::Fill)
            .height(Length::Fill)
            .direction(style::scroll_direction())
            .style(p.scrollbar()),
    );
    container(
        container(body)
            .id(DOWNLOADS_PANEL_ID)
            .padding(if compact { [14, 18] } else { [20, 24] })
            .max_width(840)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .center_x(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn download_row<'a>(app: &'a App, item: &'a Download, compact: bool) -> Element<'a, Message> {
    let p = app.presentation();
    let recipe = &item.recipe;
    let id = &recipe.job_id;
    let active = app.downloads.active() == Some(id);
    let mut actions = Vec::new();
    let enabled = !app.downloads.updating(id);
    let mut action = |label: &str, key: &str, message, enabled| {
        actions.push(control(
            app,
            format!("download-{key}-{id}"),
            label.to_owned(),
            message,
            enabled,
        ));
    };
    let stage = if active {
        action(
            "Cancel",
            "cancel",
            Action::Cancel(id.clone()),
            !app.downloads.cancelling(),
        );
        if app.downloads.cancelling() {
            "Cancelling…".into()
        } else if recipe.progress.stage == "starting" {
            "Preparing…".into()
        } else {
            recipe.progress.message.clone()
        }
    } else {
        let status = if item.saved() {
            if item.can_reveal {
                action("Open files", "open", Action::Reveal(id.clone()), enabled);
            }
            "Saved"
        } else if item.waiting() {
            "Waiting"
        } else {
            if item.retryable() {
                action("Retry", "retry", Action::Retry(id.clone()), enabled);
            }
            match recipe.progress.stage.as_str() {
                "failed" => "Needs attention",
                "cancelled" => "Cancelled",
                _ => "Interrupted",
            }
        };
        if !compact {
            action("Remove", "remove", Action::Remove(id.clone()), enabled);
        }
        status.into()
    };
    if compact {
        let details = column![
            text(&recipe.plan.title)
                .size(13)
                .font(style::semibold())
                .wrapping(text::Wrapping::None),
            text(stage)
                .size(11)
                .color(if !active && recipe.error.is_some() {
                    p.colors.warning
                } else {
                    p.colors.text_muted
                })
                .wrapping(text::Wrapping::None),
        ]
        .spacing(3);
        let mut item = column![row![
            container(details).clip(true).width(Length::Fill),
            row(actions).spacing(5)
        ]
        .spacing(8)
        .align_y(Alignment::Center),]
        .spacing(5);
        if active && recipe.progress.total > 0 && recipe.progress.completed > 0 {
            item = item.push(
                progress_bar(
                    0.0..=recipe.progress.total as f32,
                    recipe.progress.completed as f32,
                )
                .girth(3),
            );
        }
        return container(item).width(Length::Fill).padding([7, 0]).into();
    }
    let mut body = column![
        text(&recipe.plan.title).size(15).font(style::semibold()),
        text(&recipe.output_path)
            .size(11)
            .color(p.colors.text_muted),
        text(stage)
            .size(12)
            .color(if !active && recipe.error.is_some() {
                p.colors.warning
            } else {
                p.colors.text
            }),
    ]
    .spacing(5);
    if active && recipe.progress.total > 0 && recipe.progress.completed > 0 {
        body = body.push(
            progress_bar(
                0.0..=recipe.progress.total as f32,
                recipe.progress.completed as f32,
            )
            .girth(3),
        );
    }
    if active && recipe.progress.total > 1 {
        body = body.push(
            text(format!(
                "{} of {} tracks ready",
                recipe.progress.completed, recipe.progress.total
            ))
            .size(11)
            .color(p.colors.text_muted),
        );
    }
    if let Some(error) = recipe
        .error
        .as_deref()
        .filter(|_| !active && !item.waiting())
    {
        body = body.push(text(error).size(12).color(p.colors.text_muted));
    }
    if let Some(result) = &item.result {
        for warning in &result.warnings {
            body = body.push(text(warning).size(12).color(p.colors.warning));
        }
    }
    if !active && !item.waiting() && !item.saved() && recipe.error.is_none() {
        body = body.push(
            text("Check the destination before retrying an interrupted download.")
                .size(12)
                .color(p.colors.text_muted),
        );
    }
    container(body.push(row![space::horizontal(), row(actions).spacing(6)]))
        .padding(11)
        .width(Length::Fill)
        .style(move |_| p.panel())
        .into()
}

fn control<'a>(
    app: &App,
    id: impl Into<iced::advanced::widget::Id>,
    label: String,
    action: Action,
    enabled: bool,
) -> Element<'a, Message> {
    let p = app.presentation();
    focus_button::wrap(
        id,
        button(text(label).size(12))
            .padding([5, 9])
            .style(p.secondary()),
        enabled.then_some(Message::Downloads(action)),
        p.colors.accent,
    )
}
