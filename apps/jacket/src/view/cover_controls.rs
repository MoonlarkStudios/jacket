//! Inspect and manage artwork beside its preview.
use super::focus_button;
use crate::app::{App, Message, Panel};
use crate::settings::ArtworkChoice;
use audio_engine::ArtworkMode;
use iced::widget::{button, column, pick_list, row, text};
use iced::{Alignment, Element};
use jacket_core::Review;

pub fn view(app: &App, review: &Review) -> Element<'static, Message> {
    let p = app.presentation();
    let custom = review.preferences.shared_artwork_asset.is_some();
    let source = review.source_artwork_url().is_some();
    let included = !matches!(review.preferences.artwork_mode, ArtworkMode::None);
    let idle = !app.busy() && app.panel == Panel::Workspace;
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
    let mut body = column![].spacing(6);
    if review.is_song() || (included && custom && app.cover.is_none()) {
        body = body.push(text(status).size(12).color(p.colors.text_muted));
    }
    body.push(controls.wrap().vertical_spacing(6)).into()
}
