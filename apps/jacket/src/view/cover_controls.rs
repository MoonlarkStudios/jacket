//! Inspect and manage artwork beside its preview.
use super::focus_button;
use crate::app::{App, Message, Panel};
use crate::settings::ArtworkChoice;
use audio_engine::ArtworkMode;
use iced::widget::{button, column, pick_list, row, space, text};
use iced::{Alignment, Element};
use jacket_core::Review;

pub fn edit_button(app: &App) -> Element<'static, Message> {
    let p = app.presentation();
    focus_button::wrap(
        "edit-cover",
        button(text("Edit cover…").size(12))
            .padding([5, 7])
            .style(p.quiet()),
        (!app.busy()).then_some(Message::OpenPanel(Panel::Cover)),
        p.colors.accent,
    )
}

pub fn view(app: &App) -> Element<'_, Message> {
    let Some(review) = &app.review else {
        return space().into();
    };
    column![
        super::components::cover(app, 156.0, super::components::CoverContext::Source),
        controls(app, review),
    ]
    .spacing(14)
    .into()
}

fn controls(app: &App, review: &Review) -> Element<'static, Message> {
    let p = app.presentation();
    let custom = review.preferences.shared_artwork_asset.is_some();
    let source = review.source_artwork_url().is_some();
    let included = !matches!(review.preferences.artwork_mode, ArtworkMode::None);
    let idle = !app.busy();
    let status = if !included {
        if review.is_song() {
            "Cover omitted from export"
        } else {
            "Cover preview"
        }
    } else if custom && app.cover.is_none() {
        "Replacement unavailable"
    } else if custom {
        "Custom cover"
    } else if source {
        "Source cover"
    } else {
        "No cover"
    };
    let action = |id, label, message, enabled: bool| {
        focus_button::wrap(
            id,
            button(text(label).size(12))
                .padding([6, 8])
                .style(p.secondary()),
            enabled.then_some(message),
            p.colors.accent,
        )
    };
    let mut controls = row![].spacing(7).align_y(Alignment::Center);
    if !review.is_song() {
        controls = controls.push(
            pick_list(
                ArtworkChoice::ALL,
                Some(ArtworkChoice::from(review.preferences.artwork_mode)),
                Message::Artwork,
            )
            .text_size(12)
            .padding([5, 7]),
        );
    }
    controls = controls.push(action(
        "review-choose-cover",
        if source || custom {
            "Replace cover…"
        } else {
            "Choose cover…"
        },
        Message::ImportCover,
        idle && !app.example,
    ));
    if (included || !review.is_song()) && app.can_crop_cover() {
        controls = controls.push(action(
            "review-crop-cover",
            "Crop…",
            Message::CropCover,
            idle && !app.example,
        ));
    }
    if review.is_song() && (source || custom) {
        controls = controls.push(action(
            "review-toggle-cover",
            if included {
                "Omit cover"
            } else {
                "Include cover"
            },
            Message::Artwork(if included {
                ArtworkChoice::None
            } else {
                ArtworkChoice::Shared
            }),
            idle,
        ));
    }
    if custom {
        controls = controls.push(action(
            "review-source-cover",
            if source {
                "Use source cover"
            } else {
                "Remove custom cover"
            },
            Message::UseSourceCover,
            idle && !app.example,
        ));
    }
    if app.source_cover_error.is_some() {
        controls = controls.push(action(
            "review-retry-cover",
            "Retry cover",
            Message::RetryArtwork,
            idle,
        ));
    }
    column![
        text(status).size(12).color(p.colors.text_muted),
        controls.wrap().vertical_spacing(6),
    ]
    .spacing(8)
    .into()
}
